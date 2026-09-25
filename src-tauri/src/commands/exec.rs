//! Command execution and audit verification.

use crate::error::AppResult;
use crate::security::audit::{self, VerifyReport};
use crate::security::executor::{self, ExecRequest, ExecResult};
use crate::security::policy::Source;
use crate::state::AppState;
use tauri::{AppHandle, State};

/// The **only** IPC entry point that runs a shell command. Everything goes
/// through the policy engine, native confirmation and the audit log.
#[tauri::command]
pub async fn request_execution(
    app: AppHandle,
    state: State<'_, AppState>,
    command: String,
    cwd: Option<String>,
) -> AppResult<ExecResult> {
    executor::execute(
        &app,
        &state,
        ExecRequest {
            command,
            cwd,
            source: Source::User,
        },
    )
    .await
}

/// Recompute the audit log hash chain.
#[tauri::command]
pub async fn verify_audit_log(state: State<'_, AppState>) -> AppResult<VerifyReport> {
    let path = state.audit.path().to_path_buf();
    tokio::task::spawn_blocking(move || audit::verify_file(&path)).await?
}
