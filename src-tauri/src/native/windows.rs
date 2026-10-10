use crate::native::capture::reset_live_translate_interaction;
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};
pub fn open_desktop_window(app: &AppHandle, kind: &str) -> Result<(), String> {
    let (label, route, width, height, always_on_top, skip_taskbar) = match kind {
        "spotlight" => ("spotlight", "spotlight", 600.0, 80.0, true, true),
        "widget" => ("widget", "widget", 150.0, 150.0, true, true),
        "screen-picker" => ("screen-picker", "screen-picker", 1280.0, 800.0, true, true),
        "live-translate-controls" => (
            "live-translate-controls",
            "live-translate-controls",
            880.0,
            76.0,
            true,
            true,
        ),
        other => return Err(format!("unsupported desktop window '{other}'")),
    };

    if let Some(window) = app.get_webview_window(label) {
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }

    let url = WebviewUrl::App(format!("index.html#/{route}").into());
    let mut builder = WebviewWindowBuilder::new(app, label, url)
        .title(format!("Mint {kind}"))
        .inner_size(width, height)
        .decorations(false)
        .transparent(true)
        .always_on_top(always_on_top)
        .skip_taskbar(skip_taskbar);
    if kind == "screen-picker" {
        builder = builder.fullscreen(true);
    }
    builder.build().map_err(|error| error.to_string())?;
    Ok(())
}

pub fn hide_window(app: &AppHandle, label: &str) -> Result<(), String> {
    app.get_webview_window(label)
        .ok_or_else(|| format!("window '{label}' is not open"))?
        .hide()
        .map_err(|error| error.to_string())
}

pub fn close_window(app: &AppHandle, label: &str) -> Result<(), String> {
    app.get_webview_window(label)
        .ok_or_else(|| format!("window '{label}' is not open"))?
        .close()
        .map_err(|error| error.to_string())
}

pub fn resize_window(app: &AppHandle, label: &str, width: u32, height: u32) -> Result<(), String> {
    app.get_webview_window(label)
        .ok_or_else(|| format!("window '{label}' is not open"))?
        .set_size(PhysicalSize::new(width, height))
        .map_err(|error| error.to_string())
}

pub fn position_widget(app: &AppHandle) {
    let Some(widget) = app.get_webview_window("widget") else {
        return;
    };
    let Ok(Some(monitor)) = widget.primary_monitor() else {
        return;
    };
    let size = monitor.size();
    let x = size.width.saturating_sub(190) as i32;
    let _ = widget.set_position(PhysicalPosition::new(x, 40));
}

pub fn emit_to_main(app: &AppHandle, event: &str, payload: impl Serialize + Clone) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.emit(event, payload);
        let _ = main.show();
        let _ = main.set_focus();
    }
}

#[tauri::command]
pub(crate) fn open_window(app: AppHandle, kind: String) -> Result<(), String> {
    if kind == "main" {
        let window = app
            .get_webview_window("main")
            .ok_or_else(|| "main window is unavailable".to_string())?;
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }
    open_desktop_window(&app, &kind)?;
    if kind == "widget" {
        position_widget(&app);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn hide_desktop_window(app: AppHandle, label: String) -> Result<(), String> {
    hide_window(&app, &label)
}

#[tauri::command]
pub(crate) fn close_desktop_window(app: AppHandle, label: String) -> Result<(), String> {
    close_window(&app, &label)?;
    if label == "screen-picker" {
        if let Some(controls) = app.get_webview_window("live-translate-controls") {
            let _ = controls.close();
        }
        reset_live_translate_interaction();
        if let Some(main) = app.get_webview_window("main") {
            main.show().map_err(|error| error.to_string())?;
            main.set_focus().map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn resize_desktop_window(
    app: AppHandle,
    label: String,
    width: u32,
    height: u32,
) -> Result<(), String> {
    resize_window(&app, &label, width, height)
}

#[tauri::command]
pub(crate) fn submit_spotlight(app: AppHandle, query: String) {
    emit_to_main(&app, "spotlight-to-chat", query);
    let _ = hide_window(&app, "spotlight");
}

#[tauri::command]
pub(crate) fn set_ai_state(app: AppHandle, state: String) {
    if let Some(widget) = app.get_webview_window("widget") {
        let _ = widget.emit("widget-state", state);
    }
}
