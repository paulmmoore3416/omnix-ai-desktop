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
pub mod memory;
pub mod security;
pub mod settings;
pub mod state;
pub mod system;

use tauri::Manager;

/// Initialise `tracing` (stderr). Honors `RUST_LOG`; defaults to `info`.
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    // `try_init` fails only if a subscriber is already set (e.g. in tests).
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

/// Build and run the Tauri application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        // Dialogs are raised from Rust only; the webview has no dialog permissions.
        .plugin(tauri_plugin_dialog::init())
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

            // Devtools only in debug builds; release builds never expose them.
            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }
            Ok(())
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
            commands::settings::test_memresort_connection,
            commands::settings::test_integration,
            // Knowledge
            commands::knowledge::get_knowledge_data,
            commands::knowledge::save_memory,
            commands::knowledge::delete_memory,
            commands::knowledge::semantic_search,
            commands::knowledge::index_document,
            commands::knowledge::create_knowledge_base,
            commands::knowledge::export_knowledge,
            commands::knowledge::import_knowledge,
            commands::knowledge::optimize_vector_db,
            // System
            commands::system::get_system_status,
            commands::system::get_system_info,
            commands::system::get_real_time_stats,
            commands::system::get_processes,
            commands::system::kill_process,
            commands::system::get_system_control_data,
            commands::system::toggle_service,
            commands::system::create_automation,
            commands::system::toggle_automation,
            commands::system::delete_automation,
            commands::system::create_scheduled_task,
            commands::system::toggle_scheduled_task,
            commands::system::create_alert,
            commands::system::toggle_alert,
            commands::system::run_system_cleanup,
            commands::system::optimize_system,
        ])
        .run(tauri::generate_context!())
        // Setup-time failure: nothing to recover to, so exit with a clear message.
        .expect("error while running tauri application");
}
