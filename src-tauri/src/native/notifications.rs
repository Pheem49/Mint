use tauri::{AppHandle, Emitter, Manager};

/// Keep delivery errors awaitable, while waiting for an OS action off the UI thread.
#[tauri::command]
pub async fn send_mint_notification(
    app: AppHandle,
    body: String,
    chat_id: Option<String>,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        notify_rust::set_application(if tauri::is_dev() {
            "com.apple.Terminal"
        } else {
            &app.config().identifier
        })
        .map_err(|error| error.to_string())?;
        let mut notification = notify_rust::Notification::new();
        notification
            .summary("Mint Agent")
            .body(&body)
            .appname("Mint Agent")
            .action("default", "Open Mint");
        #[cfg(target_os = "windows")]
        if !tauri::is_dev() {
            notification.app_id(&app.config().identifier);
        }
        let handle = notification.show().map_err(|error| error.to_string())?;
        std::thread::spawn(move || {
            handle.wait_for_action(|action| {
                if action != "default" {
                    return;
                }
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.unminimize();
                    let _ = main.set_focus();
                    if let Some(chat_id) = &chat_id {
                        let _ = main.emit(
                            "notification-clicked",
                            serde_json::json!({ "chatId": chat_id }),
                        );
                    }
                }
            });
        });
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}
