//! Alerts, automations and scheduled tasks.

use crate::error::{AppError, AppResult};
use crate::ops::model::{Alert, Automation, ScheduledTask};
use crate::ops::rules::{self, AlertInput, AutomationInput, Kind, TaskInput};
use crate::security::policy::Source;
use crate::state::AppState;
use serde_json::{json, Value};
use tauri::{AppHandle, State};

fn kind(k: &str) -> AppResult<Kind> {
    match k {
        "alert" => Ok(Kind::Alert),
        "automation" => Ok(Kind::Automation),
        "task" => Ok(Kind::Task),
        _ => Err(AppError::InvalidInput(
            "kind must be alert, automation or task".into(),
        )),
    }
}

/// All rules, live alert values and the activity feed.
#[tauri::command]
pub async fn get_system_control_data(state: State<'_, AppState>) -> AppResult<Value> {
    let f = state.ops.snapshot();
    let values = state
        .ops
        .runtime
        .lock()
        .map(|r| r.last_values.clone())
        .unwrap_or_default();
    let alerts: Vec<Value> = f
        .alerts
        .iter()
        .map(|a| {
            json!({
                "alert": a,
                "description": a.condition.describe(),
                "value": values.get(&a.id),
                "unit": a.condition.metric.unit(),
            })
        })
        .collect();
    let automations: Vec<Value> = f
        .automations
        .iter()
        .map(|a| json!({ "automation": a, "trigger": a.trigger.describe(), "action": a.action.describe(), "approved": approved(&a.action) }))
        .collect();
    let tasks: Vec<Value> = f
        .tasks
        .iter()
        .map(|t| json!({ "task": t, "when": crate::ops::cron::Schedule::describe(&t.schedule), "action": t.action.describe(), "approved": approved(&t.action) }))
        .collect();
    let mut activity = f.activity;
    activity.reverse();
    Ok(
        json!({ "available": true, "alerts": alerts, "automations": automations, "tasks": tasks, "activity": activity }),
    )
}

fn approved(a: &crate::ops::model::Action) -> Option<bool> {
    match a {
        crate::ops::model::Action::Command { approval, .. } => Some(approval.is_some()),
        _ => None,
    }
}

/// Create an alert.
#[tauri::command]
pub async fn create_alert(
    app: AppHandle,
    state: State<'_, AppState>,
    input: AlertInput,
) -> AppResult<Alert> {
    rules::create_alert(&app, &state, input, Source::User).await
}

/// Create an automation (command actions need a one-time native approval).
#[tauri::command]
pub async fn create_automation(
    app: AppHandle,
    state: State<'_, AppState>,
    input: AutomationInput,
) -> AppResult<Automation> {
    rules::create_automation(&app, &state, input, Source::User).await
}

/// Create a scheduled task (command actions need a one-time native approval).
#[tauri::command]
pub async fn create_scheduled_task(
    app: AppHandle,
    state: State<'_, AppState>,
    input: TaskInput,
) -> AppResult<ScheduledTask> {
    rules::create_task(&app, &state, input, Source::User).await
}

/// Enable/disable an alert.
#[tauri::command]
pub async fn toggle_alert(state: State<'_, AppState>, id: String, enabled: bool) -> AppResult<()> {
    rules::set_enabled(&state, Kind::Alert, &id, enabled)
}

/// Enable/disable an automation.
#[tauri::command]
pub async fn toggle_automation(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> AppResult<()> {
    rules::set_enabled(&state, Kind::Automation, &id, enabled)
}

/// Enable/disable a scheduled task.
#[tauri::command]
pub async fn toggle_scheduled_task(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> AppResult<()> {
    rules::set_enabled(&state, Kind::Task, &id, enabled)
}

/// Delete a rule (`kind`: alert, automation, task).
#[tauri::command]
pub async fn delete_rule(
    state: State<'_, AppState>,
    rule_kind: String,
    id: String,
) -> AppResult<()> {
    rules::delete(&state, kind(&rule_kind)?, &id)
}

/// Delete an automation (kept for the existing UI binding).
#[tauri::command]
pub async fn delete_automation(state: State<'_, AppState>, id: String) -> AppResult<()> {
    rules::delete(&state, Kind::Automation, &id)
}

/// Run an automation or scheduled task now (same guarded path as a
/// triggered run).
#[tauri::command]
pub async fn run_rule_now(
    app: AppHandle,
    state: State<'_, AppState>,
    rule_kind: String,
    id: String,
) -> AppResult<()> {
    let f = state.ops.snapshot();
    let (k, name, action) = match kind(&rule_kind)? {
        Kind::Automation => f
            .automations
            .into_iter()
            .find(|a| a.id == id)
            .map(|a| ("automation", a.name, a.action)),
        Kind::Task => f
            .tasks
            .into_iter()
            .find(|t| t.id == id)
            .map(|t| ("schedule", t.name, t.action)),
        Kind::Alert => return Err(AppError::InvalidInput("alerts can't be run".into())),
    }
    .ok_or_else(|| AppError::InvalidInput("no such rule".into()))?;
    crate::ops::engine::spawn_run(app, k, id, name, action);
    Ok(())
}
