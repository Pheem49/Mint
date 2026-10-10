use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

#[derive(Default)]
pub(crate) struct TerminalSessions {
    sessions: std::sync::Arc<Mutex<HashMap<String, std::sync::Arc<TerminalSession>>>>,
}

pub(crate) struct TerminalSession {
    writer: Mutex<Box<dyn Write + Send>>,
    pty: Mutex<Box<dyn MasterPty + Send>>,
    child: Mutex<Box<dyn Child + Send + Sync>>,
}

impl Drop for TerminalSessions {
    fn drop(&mut self) {
        if let Ok(sessions) = self.sessions.lock() {
            for session in sessions.values() {
                if let Ok(mut child) = session.child.lock() {
                    let _ = child.kill();
                }
            }
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalOutput {
    session_id: String,
    data: String,
}

fn terminal_shell() -> String {
    #[cfg(windows)]
    {
        std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".into())
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
    }
}

#[tauri::command]
pub(crate) fn start_interactive_terminal(
    app: AppHandle,
    state: tauri::State<'_, TerminalSessions>,
    cwd: Option<String>,
) -> Result<String, String> {
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 28,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("Could not create terminal: {error}"))?;
    let mut command = CommandBuilder::new(terminal_shell());
    command.env("TERM", "xterm-256color");
    if let Some(path) = cwd.filter(|path| std::path::Path::new(path).is_dir()) {
        command.cwd(path);
    }
    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| format!("Could not start shell: {error}"))?;
    drop(pair.slave);

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| format!("Could not read terminal output: {error}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| format!("Could not write to terminal: {error}"))?;
    let session_id = uuid::Uuid::new_v4().to_string();
    let session = std::sync::Arc::new(TerminalSession {
        writer: Mutex::new(writer),
        pty: Mutex::new(pair.master),
        child: Mutex::new(child),
    });
    state
        .sessions
        .lock()
        .map_err(|_| "Terminal state is unavailable".to_string())?
        .insert(session_id.clone(), session.clone());

    let output_app = app.clone();
    let output_id = session_id.clone();
    let sessions = state.sessions.clone();
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(length) => {
                    let _ = output_app.emit(
                        "terminal-output",
                        TerminalOutput {
                            session_id: output_id.clone(),
                            data: BASE64.encode(&buffer[..length]),
                        },
                    );
                }
            }
        }
        let exit_code = session
            .child
            .lock()
            .ok()
            .and_then(|mut child| child.wait().ok())
            .map(|status| status.exit_code());
        if let Ok(mut sessions) = sessions.lock() {
            sessions.remove(&output_id);
        }
        let _ = output_app.emit(
            "terminal-exit",
            serde_json::json!({
                "sessionId": output_id,
                "exitCode": exit_code,
            }),
        );
    });

    Ok(session_id)
}

#[tauri::command]
pub(crate) fn write_interactive_terminal(
    state: tauri::State<'_, TerminalSessions>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    let sessions = state
        .sessions
        .lock()
        .map_err(|_| "Terminal state is unavailable".to_string())?;
    let session = sessions
        .get(&session_id)
        .ok_or("Terminal session was not found")?;
    session
        .writer
        .lock()
        .map_err(|_| "Terminal input is unavailable".to_string())?
        .write_all(data.as_bytes())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn resize_interactive_terminal(
    state: tauri::State<'_, TerminalSessions>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let sessions = state
        .sessions
        .lock()
        .map_err(|_| "Terminal state is unavailable".to_string())?;
    let session = sessions
        .get(&session_id)
        .ok_or("Terminal session was not found")?;
    session
        .pty
        .lock()
        .map_err(|_| "Terminal is unavailable".to_string())?
        .resize(PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn stop_interactive_terminal(
    state: tauri::State<'_, TerminalSessions>,
    session_id: String,
) -> Result<(), String> {
    let session = state
        .sessions
        .lock()
        .map_err(|_| "Terminal state is unavailable".to_string())?
        .remove(&session_id)
        .ok_or("Terminal session was not found")?;
    session
        .child
        .lock()
        .map_err(|_| "Terminal process is unavailable".to_string())?
        .kill()
        .map_err(|error| error.to_string())
}
