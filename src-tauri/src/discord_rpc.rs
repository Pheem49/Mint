use std::io::{Read, Write};

use mint_core::MintConfig;
use serde_json::{Value, json};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

pub fn set_activity(config: &MintConfig, instruction: &str) -> Result<String, String> {
    let application_id = config
        .extra
        .get("discordApplicationId")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or("missing config value 'discordApplicationId'")?;
    let input: Value = serde_json::from_str(instruction).unwrap_or_else(|_| {
        json!({ "details": instruction.trim().is_empty().then_some("Using Mint Assistant").unwrap_or(instruction) })
    });
    let activity = json!({
        "details": input["details"].as_str().unwrap_or("Using Mint Assistant"),
        "state": input["state"].as_str().unwrap_or("Native Tauri desktop"),
        "instance": false
    });
    send(application_id, activity)?;
    Ok("Discord Rich Presence activity updated.".into())
}

#[cfg(unix)]
fn send(application_id: &str, activity: Value) -> Result<(), String> {
    let mut socket = connect()?;
    write_frame(
        &mut socket,
        0,
        &json!({ "v": 1, "client_id": application_id }),
    )?;
    let _ = read_frame(&mut socket)?;
    write_frame(
        &mut socket,
        1,
        &json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": std::process::id(), "activity": activity },
            "nonce": format!("mint-{}", std::process::id())
        }),
    )?;
    let _ = read_frame(&mut socket)?;
    Ok(())
}

#[cfg(not(unix))]
fn send(_application_id: &str, _activity: Value) -> Result<(), String> {
    Err("Discord Rich Presence is currently implemented for Unix desktop IPC only".into())
}

#[cfg(unix)]
fn connect() -> Result<UnixStream, String> {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    (0..10)
        .find_map(|index| UnixStream::connect(format!("{runtime}/discord-ipc-{index}")).ok())
        .ok_or_else(|| "Discord IPC socket was not found; start Discord desktop first".into())
}

fn write_frame(stream: &mut impl Write, opcode: u32, payload: &Value) -> Result<(), String> {
    let payload = serde_json::to_vec(payload).map_err(|error| error.to_string())?;
    stream
        .write_all(&opcode.to_le_bytes())
        .and_then(|_| stream.write_all(&(payload.len() as u32).to_le_bytes()))
        .and_then(|_| stream.write_all(&payload))
        .map_err(|error| format!("unable to write Discord IPC frame: {error}"))
}

fn read_frame(stream: &mut impl Read) -> Result<Value, String> {
    let mut header = [0_u8; 8];
    stream
        .read_exact(&mut header)
        .map_err(|error| format!("unable to read Discord IPC header: {error}"))?;
    let length = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
    let mut payload = vec![0; length];
    stream
        .read_exact(&mut payload)
        .map_err(|error| format!("unable to read Discord IPC payload: {error}"))?;
    serde_json::from_slice(&payload).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_discord_rpc_frame() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, 1, &json!({ "ok": true })).unwrap();
        assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 1);
        assert_eq!(
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
            bytes.len() - 8
        );
    }
}
