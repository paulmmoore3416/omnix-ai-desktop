//! License commands: show, install and remove the offline license key.
//! Verification lives in `crate::license`; changes are audited.

use crate::error::AppResult;
use crate::license::{self, LicenseStatus};
use crate::security::audit::{AuditRecord, Confirmation, Decision};
use crate::security::policy::{RiskTier, Source};
use crate::state::AppState;
use tauri::State;

/// Current license status (Community when none is installed).
#[tauri::command]
pub async fn license_status(state: State<'_, AppState>) -> AppResult<LicenseStatus> {
    Ok(license::load(&license::default_path(&state.settings_path)))
}

/// Verify and install a license key pasted by the user.
#[tauri::command]
pub async fn license_install(state: State<'_, AppState>, key: String) -> AppResult<LicenseStatus> {
    let path = license::default_path(&state.settings_path);
    let result = license::install(&path, &key);
    // Only the id and tier are logged, never the key or the licensee.
    let detail = match &result {
        Ok(s) => format!(
            "installed {:?} license {}",
            s.licensed_tier,
            s.id.as_deref().unwrap_or("?")
        ),
        Err(e) => format!("rejected: {e}"),
    };
    audit(&state, result.is_ok(), detail).await?;
    result
}

/// Remove the installed license key.
#[tauri::command]
pub async fn license_remove(state: State<'_, AppState>) -> AppResult<LicenseStatus> {
    let status = license::remove(&license::default_path(&state.settings_path))?;
    audit(&state, true, "license removed".into()).await?;
    Ok(status)
}

async fn audit(state: &AppState, ok: bool, detail: String) -> AppResult<()> {
    state
        .audit
        .record(AuditRecord {
            id: uuid::Uuid::new_v4().to_string(),
            source: Source::User,
            action: "license_change".into(),
            command: "license".into(),
            cwd: None,
            tier: RiskTier::ReadOnly,
            decision: if ok {
                Decision::Allowed
            } else {
                Decision::Failed
            },
            confirmation: Confirmation::NotRequired,
            exit_code: None,
            duration_ms: None,
            detail: Some(detail),
        })
        .await
}
