//! Collapse the actual window so the invisible character area cannot intercept clicks.
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{Emitter, Manager};

struct SavedWindow {
    size: tauri::PhysicalSize<u32>,
    decorated: bool,
    resizable: bool,
    maximized: bool,
}

#[derive(Default)]
pub struct CompactWindow {
    saved: Mutex<Option<SavedWindow>>,
    pub active: AtomicBool,
}

fn restore(window: &tauri::WebviewWindow, saved: &SavedWindow) -> tauri::Result<()> {
    window.set_size(saved.size)?;
    window.set_min_size(Some(tauri::LogicalSize::new(340.0, 560.0)))?;
    window.set_decorations(saved.decorated)?;
    window.set_resizable(saved.resizable)?;
    if saved.maximized {
        window.maximize()?;
    }
    Ok(())
}

#[tauri::command]
pub fn set_compact(window: tauri::WebviewWindow, compact: bool) -> Result<(), String> {
    let state = window.state::<CompactWindow>();
    let mut saved = state.saved.lock().unwrap();
    if compact == saved.is_some() {
        return Ok(());
    }
    if compact {
        let previous = SavedWindow {
            size: window.inner_size().map_err(|e| e.to_string())?,
            decorated: window.is_decorated().map_err(|e| e.to_string())?,
            resizable: window.is_resizable().map_err(|e| e.to_string())?,
            maximized: window.is_maximized().map_err(|e| e.to_string())?,
        };
        state.active.store(true, Ordering::Release);
        let result = (|| -> tauri::Result<()> {
            if previous.maximized {
                window.unmaximize()?;
            }
            window.set_decorations(false)?;
            window.set_resizable(false)?;
            window.set_min_size(Some(tauri::LogicalSize::new(64.0, 64.0)))?;
            window.set_size(tauri::LogicalSize::new(64.0, 64.0))
        })();
        if let Err(error) = result {
            let _ = restore(&window, &previous);
            state.active.store(false, Ordering::Release);
            return Err(error.to_string());
        }
        *saved = Some(previous);
    } else if let Some(previous) = saved.as_ref() {
        restore(&window, previous).map_err(|e| e.to_string())?;
        *saved = None;
        state.active.store(false, Ordering::Release);
    }
    let _ = window.emit("companion-compact", compact);
    Ok(())
}
