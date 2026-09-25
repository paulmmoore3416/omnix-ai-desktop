//! System monitoring and control commands.

use super::not_implemented;
use crate::error::AppResult;
use crate::state::AppState;
use crate::system::metrics::{self, RealTimeStats, SystemInfo, SystemStatus};
use crate::system::processes::{self, ProcessInfo};
use serde_json::{json, Value};
use tauri::{AppHandle, State};

/// CPU/memory snapshot for the sidebar.
#[tauri::command]
pub async fn get_system_status(state: State<'_, AppState>) -> AppResult<SystemStatus> {
    metrics::status(&mut *state.system()?)
}

/// Static host information.
#[tauri::command]
pub async fn get_system_info(state: State<'_, AppState>) -> AppResult<SystemInfo> {
    metrics::info(&mut *state.system()?)
}

/// Live metrics.
#[tauri::command]
pub async fn get_real_time_stats(state: State<'_, AppState>) -> AppResult<RealTimeStats> {
    metrics::real_time(&mut *state.system()?)
}

/// Top processes by CPU.
#[tauri::command]
pub async fn get_processes(state: State<'_, AppState>) -> AppResult<Vec<ProcessInfo>> {
    Ok(processes::list(&mut *state.system()?))
}

/// Terminate a process (native confirmation + audit).
#[tauri::command]
pub async fn kill_process(app: AppHandle, state: State<'_, AppState>, pid: u32) -> AppResult<()> {
    processes::kill(&app, &state, pid).await
}

/// Services/automations/schedules/alerts are not implemented; `available`
/// lets the UI show that honestly.
#[tauri::command]
pub async fn get_system_control_data() -> AppResult<Value> {
    Ok(json!({
        "available": false,
        "processes": [],
        "services": [],
        "automations": [],
        "scheduledTasks": [],
        "alerts": []
    }))
}

/// Not implemented: start/stop a system service.
#[tauri::command]
pub async fn toggle_service() -> AppResult<()> {
    not_implemented("toggle_service")
}

/// Not implemented: create an automation.
#[tauri::command]
pub async fn create_automation() -> AppResult<()> {
    not_implemented("create_automation")
}

/// Not implemented: enable/disable an automation.
#[tauri::command]
pub async fn toggle_automation() -> AppResult<()> {
    not_implemented("toggle_automation")
}

/// Not implemented: delete an automation.
#[tauri::command]
pub async fn delete_automation() -> AppResult<()> {
    not_implemented("delete_automation")
}

/// Not implemented: create a scheduled task.
#[tauri::command]
pub async fn create_scheduled_task() -> AppResult<()> {
    not_implemented("create_scheduled_task")
}

/// Not implemented: enable/disable a scheduled task.
#[tauri::command]
pub async fn toggle_scheduled_task() -> AppResult<()> {
    not_implemented("toggle_scheduled_task")
}

/// Not implemented: create an alert.
#[tauri::command]
pub async fn create_alert() -> AppResult<()> {
    not_implemented("create_alert")
}

/// Not implemented: enable/disable an alert.
#[tauri::command]
pub async fn toggle_alert() -> AppResult<()> {
    not_implemented("toggle_alert")
}

/// Not implemented: system cleanup.
#[tauri::command]
pub async fn run_system_cleanup() -> AppResult<()> {
    not_implemented("run_system_cleanup")
}

/// Not implemented: system optimization.
#[tauri::command]
pub async fn optimize_system() -> AppResult<()> {
    not_implemented("optimize_system")
}
