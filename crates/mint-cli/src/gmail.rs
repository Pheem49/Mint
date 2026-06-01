use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Stdio},
    thread,
    time::Duration,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use mint_core::{MintConfig, load_config, save_config};
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const SCOPES: &str =
    "https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.compose";

pub async fn auth(no_open: bool, port: u16) -> Result<()> {
    let mut config = load_config()?;
    let client_id = config_string(&config, "gmailClientId")?;
    let client_secret = config_string(&config, "gmailClientSecret")?;
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let actual_port = listener.local_addr()?.port();
    let redirect = format!("http://127.0.0.1:{actual_port}/oauth2callback");
    let state = state_token();
    let url = format!(
        "{AUTH_URL}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&state={}",
        encode(&client_id),
        encode(&redirect),
        encode(SCOPES),
        encode(&state)
    );
    println!("Open this Google OAuth consent link for Gmail:\n{url}\n");
    if !no_open {
        open_browser(&url)?;
    }
    println!("Waiting for Gmail OAuth callback on {redirect} ...");
    let code = wait_for_code(listener, &state)?;
    let token: Value = Client::new()
        .post(TOKEN_URL)
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", code.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let refresh = token["refresh_token"]
        .as_str()
        .ok_or_else(|| anyhow!("Google did not return a refresh token; run Gmail auth again"))?;
    config
        .extra
        .insert("gmailRefreshToken".into(), Value::String(refresh.into()));
    config
        .extra
        .insert("pluginGmailEnabled".into(), Value::Bool(true));
    save_config(&config)?;
    println!("Gmail OAuth refresh token saved.");
    Ok(())
}

fn wait_for_code(listener: TcpListener, expected_state: &str) -> Result<String> {
    listener.set_nonblocking(true)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(180);
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() >= deadline {
                    bail!("timed out waiting for Gmail authorization callback");
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(error.into()),
        }
    };
    let mut bytes = [0_u8; 8192];
    let read = stream.read(&mut bytes)?;
    let request = String::from_utf8_lossy(&bytes[..read]);
    let target = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| anyhow!("invalid OAuth callback request"))?;
    let (_, query) = target
        .split_once('?')
        .ok_or_else(|| anyhow!("OAuth callback did not contain query parameters"))?;
    let values = query
        .split('&')
        .filter_map(|part| part.split_once('='))
        .map(|(key, value)| (key.to_owned(), decode(value)))
        .collect::<BTreeMap<_, _>>();
    let response = if values
        .get("state")
        .is_some_and(|state| state == expected_state)
        && values.contains_key("code")
    {
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<h1>Gmail connected</h1><p>You can close this window and return to Mint.</p>"
    } else {
        "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\n\r\nInvalid Gmail authorization response."
    };
    stream.write_all(response.as_bytes())?;
    if let Some(error) = values.get("error") {
        bail!("Gmail authorization failed: {error}");
    }
    if values
        .get("state")
        .is_none_or(|state| state != expected_state)
    {
        bail!("invalid Gmail OAuth state");
    }
    values
        .get("code")
        .cloned()
        .ok_or_else(|| anyhow!("Gmail OAuth callback did not include a code"))
}

fn config_string(config: &MintConfig, key: &str) -> Result<String> {
    config
        .extra
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("missing config value '{key}'"))
}

fn state_token() -> String {
    let seed = format!(
        "{}:{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    format!("{:x}", Sha256::digest(seed.as_bytes()))
}

fn open_browser(url: &str) -> Result<()> {
    let mut command = if cfg!(target_os = "windows") {
        let mut command = Command::new("cmd");
        command.args(["/c", "start", "", url]);
        command
    } else if cfg!(target_os = "macos") {
        let mut command = Command::new("open");
        command.arg(url);
        command
    } else {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("unable to open browser")?;
    Ok(())
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&value[index + 1..index + 3], 16)
        {
            output.push(byte);
            index += 3;
            continue;
        }
        output.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes_oauth_values() {
        let value = "http://127.0.0.1/callback?a=b c";
        assert_eq!(decode(&encode(value)), value);
    }
}
