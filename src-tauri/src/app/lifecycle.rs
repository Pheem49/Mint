use crate::app::shortcuts::install_shortcuts;
use crate::app::tray::install_tray;
use crate::background::events::start_system_events;
use crate::background::headless::start_headless_queue;
use crate::background::proactive::start_loop as start_proactive_loop;
use crate::commands::approvals::ApprovalsState;
use crate::commands::voice::{GeminiLiveState, MicRecordingState};
use crate::native::terminal::TerminalSessions;
use crate::native::windows::{open_desktop_window, position_widget};
use crate::{app, background, commands, native};
use mint_core::{load_config, start_channels, start_cron_scheduler};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub(crate) fn exit_app(app: AppHandle) {
    app.exit(0);
}

/// WebKitGTK denies every `permission-request` (microphone, camera, geolocation, ...) by
/// default unless something handles the signal — Tauri/wry doesn't wire this up on Linux,
/// which is why `getUserMedia()` rejects with `NotAllowedError` even though the user never
/// saw (or could act on) a prompt. Auto-allow only mic/camera requests here; everything
/// else falls through to WebKit's default (deny).
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
fn allow_media_permission_requests(window: &tauri::WebviewWindow) {
    use webkit2gtk::{PermissionRequestExt, WebViewExt, glib::prelude::ObjectExt};
    let _ = window.with_webview(|platform_webview| {
        let webview = platform_webview.inner();
        webview.connect_permission_request(|_webview, request| {
            if request.is::<webkit2gtk::UserMediaPermissionRequest>() {
                request.allow();
                true
            } else {
                false
            }
        });
    });
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
)))]
fn allow_media_permission_requests(_window: &tauri::WebviewWindow) {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if std::env::args().nth(1).as_deref() == Some("--mint-mod-host") {
        if let Err(error) = mint_core::mods::worker_main() {
            eprintln!("{error}");
        }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(ApprovalsState::default())
        .manage(GeminiLiveState::default())
        .manage(MicRecordingState::default())
        .manage(TerminalSessions::default())
        .manage(mint_core::avatar_bridge::AvatarBridge::new(
            load_config()
                .map(|c| mint_core::avatar_bridge::AvatarBridgeConfig::from_mint_config(&c))
                .unwrap_or_else(|_| mint_core::avatar_bridge::AvatarBridgeConfig::from_env()),
        ))
        .setup(|app| {
            // The embedded API also observes these signals, but its completion
            // does not stop Tauri's event loop. Keep the host watcher independent
            // so Ctrl+C still exits when the API port could not be bound.
            let shutdown_app = app.handle().clone();
            tauri::async_runtime::spawn(mint_core::api_server::shutdown_on_signal(move || {
                shutdown_app.exit(0);
            }));
            if let Some(main_window) = app.get_webview_window("main") {
                allow_media_permission_requests(&main_window);
            }
            std::thread::spawn(|| {
                if let Err(error) = mint_core::library::LibraryStore::open_default() {
                    eprintln!("[mint] Library startup: {error}")
                }
            });
            install_tray(app.handle())?;
            install_shortcuts(app.handle())?;
            start_system_events(app.handle().clone());
            start_headless_queue(app.handle().clone());
            start_proactive_loop(app.handle().clone());
            tauri::async_runtime::spawn(async {
                mint_core::companion::start();
                if let Err(error) = mint_core::start_api_server(3000).await {
                    eprintln!("[mint] embedded API unavailable: {error}");
                    start_channels();
                    start_cron_scheduler();
                }
            });
            if load_config()
                .map(|config| config.show_desktop_widget)
                .unwrap_or(false)
            {
                let _ = open_desktop_window(app.handle(), "widget");
                position_widget(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::mods::mods_operation,
            native::notifications::send_mint_notification,
            native::terminal::start_interactive_terminal,
            native::terminal::write_interactive_terminal,
            native::terminal::resize_interactive_terminal,
            native::terminal::stop_interactive_terminal,
            native::browser::open_mint_browser,
            native::browser::close_mint_browser,
            native::browser::show_mint_browser_suggestions,
            native::browser::hide_mint_browser_suggestions,
            native::browser::get_mint_browser_suggestion_query,
            native::browser::navigate_mint_browser,
            native::browser::browser_go_back,
            native::browser::browser_go_forward,
            native::browser::browser_reload,
            commands::settings::get_runtime_status,
            commands::settings::detect_system_tools,
            commands::integrations::reauth_mcp_server,
            commands::integrations::list_mcp_server_tools,
            commands::integrations::test_mcp_connection,
            commands::integrations::read_mcp_artifact,
            commands::settings::fetch_provider_models,
            commands::media::fetch_image_provider_models,
            commands::media::fetch_video_provider_models,
            commands::media::fetch_gemini_live_models,
            commands::workspace::get_workspace_snapshot,
            commands::workspace::get_git_branch_info,
            commands::workspace::switch_git_branch,
            commands::workspace::create_git_branch,
            commands::workspace::checkout_remote_git_branch,
            commands::workspace::get_git_graph,
            commands::workspace::get_workspace_git_diff,
            commands::workspace::create_workspace_file,
            commands::workspace::create_workspace_folder,
            commands::workspace::delete_workspace_item,
            commands::workspace::move_workspace_item,
            commands::workspace::list_workspace_history,
            commands::workspace::undo_workspace_action,
            commands::media::generate_images,
            commands::media::generate_video,
            commands::workspace::select_workspace_directory,
            commands::settings::get_config,
            commands::settings::get_updater_status,
            commands::settings::check_for_updates,
            commands::settings::install_available_update,
            commands::settings::update_config,
            commands::settings::set_active_model,
            commands::settings::inspect_shell_command,
            commands::conversation::send_chat_message,
            commands::conversation::stream_chat_message,
            commands::conversation::cancel_chat_message,
            commands::voice::start_gemini_live_session,
            commands::voice::send_gemini_live_audio_chunk,
            commands::voice::stop_gemini_live_session,
            commands::voice::start_mic_recording,
            commands::voice::stop_mic_recording_and_transcribe,
            commands::voice::cancel_mic_recording,
            commands::conversation::submit_tool_approval,
            commands::conversation::get_recent_interactions,
            commands::conversation::get_conversation_snapshot,
            commands::conversation::get_conversation_sources,
            commands::conversation::save_source_attachments,
            commands::conversation::get_conversation_changes,
            commands::conversation::save_interaction_agent_activity,
            commands::conversation::list_chat_sessions,
            commands::conversation::delete_chat_session,
            commands::conversation::rename_chat_session,
            commands::auth::get_profile_value,
            commands::auth::set_profile_value,
            commands::auth::auth_register,
            commands::auth::auth_login,
            commands::auth::auth_logout,
            commands::auth::auth_current_user,
            commands::auth::auth_update_profile,
            commands::auth::auth_upload_avatar,
            commands::conversation::clear_chat_history,
            commands::integrations::list_learned_skills,
            commands::integrations::add_learned_skill,
            commands::integrations::delete_learned_skill,
            commands::integrations::list_subagents,
            commands::integrations::save_subagent,
            commands::integrations::delete_subagent,
            commands::conversation::run_slash_command,
            commands::workspace::list_background_jobs,
            commands::workspace::start_html_preview,
            commands::workspace::get_background_job,
            commands::workspace::stop_background_job,
            commands::workspace::remove_background_job,
            commands::integrations::list_cron_jobs,
            commands::integrations::add_cron_job,
            commands::integrations::remove_cron_job,
            commands::integrations::set_cron_job_enabled,
            commands::workspace::list_linked_folders,
            commands::workspace::add_linked_folder,
            commands::workspace::remove_linked_folder,
            commands::workspace::linked_folder_status,
            commands::workspace::refresh_linked_folder,
            commands::workspace::list_linked_folder_notes,
            commands::workspace::read_linked_folder_note,
            commands::workspace::save_linked_folder_note,
            commands::workspace::open_linked_folder_note,
            commands::media::list_library,
            commands::media::get_library_item,
            commands::media::set_library_trashed,
            commands::media::remove_library_item,
            commands::media::read_library_file,
            commands::media::list_pictures,
            commands::media::delete_picture,
            commands::media::save_pictures,
            commands::media::upload_file,
            commands::workspace::open_folder,
            commands::media::get_tts_urls,
            commands::media::get_weather,
            commands::workspace::propose_desktop_code_edits,
            commands::workspace::apply_desktop_code_edits,
            commands::workspace::list_git_checkpoints,
            commands::workspace::rollback_git_checkpoint,
            commands::workspace::undo_git_checkpoint,
            commands::workspace::read_workspace_file,
            native::windows::open_window,
            native::windows::hide_desktop_window,
            native::windows::close_desktop_window,
            native::windows::resize_desktop_window,
            native::actions::run_desktop_action,
            commands::integrations::get_integration_inventory,
            commands::integrations::run_native_plugin,
            native::capture::capture_chat_screen,
            native::capture::read_clipboard_image,
            native::capture::capture_translation_frame,
            native::capture::translate_captured_frame,
            native::actions::get_smart_context,
            native::browser::get_browser_tabs,
            native::browser::navigate_browser,
            native::browser::read_browser_page,
            native::browser::click_browser_selector,
            native::browser::type_in_browser,
            native::capture::start_live_translate,
            native::capture::open_live_translate_controls,
            native::capture::live_translate_geometry,
            native::capture::set_live_translate_passthrough,
            native::windows::submit_spotlight,
            native::windows::set_ai_state,
            background::proactive::toggle_proactive,
            background::proactive::save_behavior_context,
            background::headless::run_next_queued_task,
            app::lifecycle::exit_app,
            commands::conversation::save_system_interaction,
            commands::conversation::update_chat_session_workspace
        ])
        .build(tauri::generate_context!())
        .expect("error while running Mint desktop")
        .run(|_app_handle, event| {
            // Kill any stdio MCP child processes we spawned before the process
            // goes away — `SESSIONS` is a `static`, so `McpSession::Drop` never
            // runs at exit on its own. Fires for tray "Quit", `exit_app`, and
            // the last window closing.
            if let tauri::RunEvent::Exit = event {
                mint_core::companion::shutdown();
                mint_core::close_all_mcp_sessions();
                mint_core::bg_shell::shutdown();
            }
        });
}
