use crate::native::capture::toggle_live_translate_interaction;
use crate::native::windows::open_desktop_window;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub(crate) fn install_shortcuts(app: &AppHandle) -> tauri::Result<()> {
    let main_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);
    let spotlight_shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Space);
    let live_translate_shortcut =
        Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyL);
    let main_handler = main_shortcut;
    let spotlight_handler = spotlight_shortcut;
    let live_translate_handler = live_translate_shortcut;
    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut, event| {
                if event.state() != ShortcutState::Pressed {
                    return;
                }
                if shortcut == &main_handler {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = if window.is_visible().unwrap_or(false) {
                            window.hide()
                        } else {
                            window.show()
                        };
                    }
                } else if shortcut == &spotlight_handler {
                    let _ = open_desktop_window(app, "spotlight");
                } else if shortcut == &live_translate_handler {
                    toggle_live_translate_interaction(app);
                }
            })
            .build(),
    )?;
    let _ = app.global_shortcut().register(main_shortcut);
    let _ = app.global_shortcut().register(spotlight_shortcut);
    let _ = app.global_shortcut().register(live_translate_shortcut);
    Ok(())
}
