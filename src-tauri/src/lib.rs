//! OMNIX desktop backend.
//!
//! Layout:
//! * [`commands`]: thin `#[tauri::command]` wrappers (the IPC surface).
//! * [`security`]: policy engine, native confirmation, executor, audit log, keychain.
//! * [`ai`]: LLM providers, agent loop and `local_only` enforcement.
//! * [`memory`]: long-term memory (kb-core adapter).
//! * [`system`]: metrics and process management.
//! * [`settings`]: persisted configuration (no secrets).
//! * [`state`]: [`state::AppState`], managed by Tauri.
//! * [`error`]: [`error::AppError`], serialized to the frontend as `{kind, message}`.

pub mod ai;
pub mod commands;
pub mod desktop;
pub mod error;
pub mod mcp;
pub mod memory;
pub mod observability;
pub mod ops;
pub mod security;
pub mod settings;
pub mod state;
pub mod system;
pub mod voice;

use tauri::Manager;

/// App logging: stdout plus a rotating `omnix.log` in the app log directory.
/// This is operational logging, deliberately separate from the hash-chained
/// `audit.jsonl`. `tracing` events reach it through tracing's `log` bridge.
/// No webview target: the frontend has no log permissions.
fn log_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    use tauri_plugin_log::{RotationStrategy, Target, TargetKind};
    tauri_plugin_log::Builder::new()
        .clear_targets()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir {
                file_name: Some("omnix".into()),
            }),
        ])
        .level(if cfg!(debug_assertions) {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        // Dependencies are noisy at debug level.
        .level_for("hyper_util", log::LevelFilter::Info)
        .level_for("rustls", log::LevelFilter::Info)
        .level_for("tao", log::LevelFilter::Info)
        .level_for("zbus", log::LevelFilter::Warn)
        .level_for("tracing::span", log::LevelFilter::Warn)
        .level_for("keyring_core", log::LevelFilter::Info)
        .level_for("rustls_platform_verifier", log::LevelFilter::Info)
        .max_file_size(5 * 1024 * 1024)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .build()
}

/// Build and run the Tauri application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch focuses the running instance instead.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            desktop::show_main(app);
        }))
        .plugin(log_plugin())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // Dialogs are raised from Rust only; the webview has no dialog permissions.
        .plugin(tauri_plugin_dialog::init())
        // Desktop notifications for alerts/automations, sent from Rust only;
        // the webview has no notification permissions.
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let state = state::AppState::init(app.handle())?;
            let auto_start = tauri::async_runtime::block_on(state.settings.read())
                .general
                .auto_start;
            app.manage(state);

            // Keep the OS launch-at-login entry in sync with settings. A
            // failure here is logged, not fatal (e.g. read-only home).
            if let Err(e) = desktop::apply_autostart(app.handle(), auto_start) {
                tracing::warn!(error = %e, "could not apply auto_start setting");
            }
            if let Err(e) = desktop::setup_tray(app.handle()) {
                tracing::warn!(error = %e, "could not create tray icon");
            }
            desktop::setup_shortcuts(app.handle());
            desktop::setup_microphone(app.handle());
            // Metrics sampling, alerts, automations and the scheduler.
            ops::engine::start(app.handle().clone());
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<state::AppState>();
                if let Err(e) = observability::configure(&state).await {
                    tracing::warn!(error = %e, "audit shipping not enabled");
                }
            });
            // The updater plugin is intentionally NOT registered until release
            // signing is configured (see docs/ARCHITECTURE.md).

            // Devtools only in debug builds; release builds never expose them.
            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }
            Ok(())
        })
        // Close-to-tray when `general.minimize_to_tray` is on.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let to_tray = window
                    .app_handle()
                    .try_state::<state::AppState>()
                    .and_then(|s| {
                        s.settings
                            .try_read()
                            .ok()
                            .map(|s| s.general.minimize_to_tray)
                    })
                    .unwrap_or(false);
                if to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Chat
            commands::chat::process_command,
            commands::chat::chat_send,
            commands::chat::chat_cancel,
            commands::chat::chat_reset,
            // Execution (the only shell entry point) and audit
            commands::exec::request_execution,
            commands::exec::verify_audit_log,
            // Files
            commands::files::select_file,
            // Settings & secrets
            commands::settings::load_settings,
            commands::settings::save_settings,
            commands::settings::reset_settings,
            commands::settings::export_settings,
            commands::settings::import_settings,
            commands::settings::set_secret,
            commands::settings::delete_secret,
            commands::settings::has_secret,
            commands::settings::test_ai_model,
            commands::settings::list_ollama_models,
            commands::settings::list_models,
            commands::settings::test_memory_backend,
            commands::settings::mcp_test_server,
            commands::settings::test_memresort_connection,
            // Knowledge
            commands::knowledge::get_knowledge_data,
            commands::knowledge::save_memory,
            commands::knowledge::delete_memory,
            commands::knowledge::update_memory,
            commands::knowledge::list_hidden_memories,
            commands::knowledge::delete_document,
            commands::knowledge::sync_knowledge_folders,
            commands::knowledge::semantic_search,
            commands::knowledge::index_document,
            commands::knowledge::create_knowledge_base,
            commands::knowledge::list_knowledge_bases,
            commands::knowledge::delete_knowledge_base,
            commands::knowledge::export_knowledge,
            commands::knowledge::import_knowledge,
            commands::knowledge::optimize_vector_db,
            // System
            commands::system::get_system_status,
            commands::system::get_system_info,
            commands::system::get_real_time_stats,
            commands::system::get_processes,
            commands::system::kill_process,
            commands::system::get_performance,
            commands::system::get_gpus,
            commands::system::list_services,
            commands::system::toggle_service,
            commands::system::service_logs,
            commands::system::list_containers,
            commands::system::control_container,
            commands::system::container_logs,
            commands::system::scan_cleanup,
            commands::system::run_system_cleanup,
            commands::system::optimize_system,
            commands::system::apply_recommendation,
            // Alerts, automations, scheduler
            commands::ops::get_system_control_data,
            commands::ops::create_alert,
            commands::ops::create_automation,
            commands::ops::create_scheduled_task,
            commands::ops::toggle_alert,
            commands::ops::toggle_automation,
            commands::ops::toggle_scheduled_task,
            commands::ops::delete_rule,
            commands::ops::delete_automation,
            commands::ops::run_rule_now,
            // Model manager
            commands::models::list_models_detail,
            commands::models::model_load,
            commands::models::model_unload,
            commands::models::model_delete,
            commands::models::model_pull,
            // Voice
            commands::voice::voice_transcribe,
            commands::voice::voice_speak,
        ])
        .run(tauri::generate_context!())
        // Setup-time failure: nothing to recover to, so exit with a clear message.
        .expect("error while running tauri application");
}
