//! Local usage ledger commands (see `crate::usage`).

use crate::error::AppResult;
use crate::security::audit::{AuditRecord, Confirmation, Decision};
use crate::security::policy::{RiskTier, Source};
use crate::state::AppState;
use crate::usage::UsageSummary;
use tauri::State;

/// Daily totals for the last `days` days (default 30, at most 366).
#[tauri::command]
pub async fn usage_summary(
    state: State<'_, AppState>,
    days: Option<u32>,
) -> AppResult<UsageSummary> {
    Ok(state.usage.summary(days.unwrap_or(30)))
}

/// Delete the recorded usage history.
#[tauri::command]
pub async fn usage_clear(state: State<'_, AppState>) -> AppResult<UsageSummary> {
    state.usage.clear()?;
    state
        .audit
        .record(AuditRecord {
            id: uuid::Uuid::new_v4().to_string(),
            source: Source::User,
            action: "usage_clear".into(),
            command: state.usage.path().display().to_string(),
            cwd: None,
            tier: RiskTier::ReadOnly,
            decision: Decision::Allowed,
            confirmation: Confirmation::NotRequired,
            exit_code: None,
            duration_ms: None,
            detail: None,
        })
        .await?;
    Ok(state.usage.summary(30))
}
