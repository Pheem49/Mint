//! Dependency-free, loopback-only static HTML preview, tracked as a background job.
use crate::{Capability, MintConfig, assert_path_capability};
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

static SERVERS: LazyLock<Mutex<HashMap<(PathBuf, Option<String>), (String, String)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn start(
    root: &Path,
    relative: &str,
    owner: Option<String>,
    config: &MintConfig,
) -> Result<serde_json::Value, String> {
    let root = assert_path_capability(root, Capability::Read, config).map_err(|e| e.to_string())?;
    let file = super::workspace::resolve(&root, relative)?;
    assert_path_capability(&file, Capability::Read, config).map_err(|e| e.to_string())?;
    if !file.is_file()
        || !file
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
    {
        return Err("Select an existing HTML file inside the workspace".into());
    }
    if Path::new(relative)
        .components()
        .any(|p| p.as_os_str().to_string_lossy().starts_with('.'))
    {
        return Err("Hidden files cannot be served by HTML preview".into());
    }
    let mut servers = SERVERS.lock().map_err(|e| e.to_string())?;
    servers.retain(|_, (id, _)| {
        crate::bg_shell::job_json(id, false).is_ok_and(|j| j["status"] == "running")
    });
    let key = (root.clone(), owner.clone());
    let existing = servers
        .get(&key)
        .filter(|(id, _)| {
            crate::bg_shell::job_json(id, false).is_ok_and(|j| j["status"] == "running")
        })
        .cloned();
    let (id, base) = if let Some(value) = existing {
        value
    } else {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let base = format!(
            "http://{}",
            listener.local_addr().map_err(|e| e.to_string())?
        );
        let served_root = root.clone();
        let served_config = config.clone();
        let id = crate::bg_shell::start_local_service(root, owner, base.clone(), move |stop| {
            while !stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = serve(stream, &served_root, &served_config, &stop);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(20))
                    }
                    Err(e) => return Err(e.to_string()),
                }
            }
            Ok(())
        })?;
        servers.insert(key, (id.clone(), base.clone()));
        (id, base)
    };
    let encoded = relative
        .replace('\\', "/")
        .split('/')
        .map(|segment| {
            segment
                .bytes()
                .map(|b| {
                    if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                        (b as char).to_string()
                    } else {
                        format!("%{b:02X}")
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("/");
    Ok(serde_json::json!({"url": format!("{base}/{encoded}"), "jobId": id}))
}

fn reply(stream: &mut TcpStream, code: &str, message: &str) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {code}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{message}",
        message.len()
    )
}
fn serve(
    mut stream: TcpStream,
    root: &Path,
    config: &MintConfig,
    stop: &Arc<AtomicBool>,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    let mut request = Vec::new();
    let mut chunk = [0; 1024];
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
        if stop.load(Ordering::Acquire) || std::time::Instant::now() > deadline {
            return Ok(());
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 || request.len() + n > 16384 {
            return reply(&mut stream, "400 Bad Request", "Invalid request");
        }
        request.extend_from_slice(&chunk[..n]);
    }
    let request = String::from_utf8_lossy(&request);
    let headers: HashMap<_, _> = request
        .lines()
        .skip(1)
        .filter_map(|line| {
            line.split_once(':')
                .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_string()))
        })
        .collect();
    let mut words = request.lines().next().unwrap_or("").split_whitespace();
    let method = words.next().unwrap_or("");
    let target = words.next().unwrap_or("");
    if !matches!(method, "GET" | "HEAD") {
        return reply(&mut stream, "405 Method Not Allowed", "Read-only preview");
    }
    let address = stream.local_addr()?.to_string();
    if headers.get("host").is_none_or(|host| host != &address)
        || (headers
            .get("sec-fetch-site")
            .is_some_and(|s| s == "cross-site")
            && headers
                .get("sec-fetch-mode")
                .is_none_or(|m| m != "navigate"))
    {
        return reply(&mut stream, "403 Forbidden", "Preview access denied");
    }
    let raw = target
        .split('?')
        .next()
        .unwrap_or("")
        .strip_prefix('/')
        .unwrap_or("");
    let mut decoded = Vec::new();
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return reply(&mut stream, "400 Bad Request", "Invalid path");
            }
            let Some(value) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok())
            else {
                return reply(&mut stream, "400 Bad Request", "Invalid path");
            };
            decoded.push(value);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    let Ok(relative) = String::from_utf8(decoded) else {
        return reply(&mut stream, "400 Bad Request", "Invalid path");
    };
    if relative.contains('\\')
        || relative.contains('\0')
        || relative.split('/').any(|s| s.starts_with('.'))
    {
        return reply(&mut stream, "403 Forbidden", "Path denied");
    }
    let relative = if relative.is_empty() {
        "index.html".to_string()
    } else if relative.ends_with('/') {
        format!("{relative}index.html")
    } else {
        relative
    };
    let Ok(path) = super::workspace::resolve(root, &relative).and_then(|p| {
        assert_path_capability(&p, Capability::Read, config).map_err(|e| e.to_string())
    }) else {
        return reply(&mut stream, "403 Forbidden", "Path denied");
    };
    if !path.is_file() {
        return reply(&mut stream, "404 Not Found", "File not found");
    }
    let mut file = File::open(&path)?;
    let mime = match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    };
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        file.metadata()?.len()
    )?;
    if method == "HEAD" {
        return Ok(());
    }
    let mut buffer = [0; 65536];
    while !stop.load(Ordering::Acquire) {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        stream.write_all(&buffer[..n])?;
    }
    Ok(())
}

/// Text interface for CLI/TUI and authenticated slash-command hosts.
pub fn command(
    root: &Path,
    path: &str,
    owner: Option<String>,
    config: &MintConfig,
) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Ok("Usage: /preview <workspace-relative HTML file>".into());
    }
    let path = if path.starts_with('"') && path.ends_with('"') && path.len() > 1 {
        &path[1..path.len() - 1]
    } else {
        path
    };
    let preview = start(root, path, owner, config)?;
    Ok(format!(
        "HTML preview: {}\n\nBackground terminal: `{}` · `/shells stop {}`",
        preview["url"].as_str().unwrap_or(""),
        preview["jobId"].as_str().unwrap_or(""),
        preview["jobId"].as_str().unwrap_or("")
    ))
}
