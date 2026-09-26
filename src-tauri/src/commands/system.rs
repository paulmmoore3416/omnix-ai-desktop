//! System monitoring and control commands: metrics, GPUs, history,
//! processes, services, containers, cleanup and optimisation.

use crate::error::{AppError, AppResult};
use crate::security::policy::Source;
use crate::state::AppState;
use crate::system::metrics::{DetailedStats, RealTimeStats, SystemInfo, SystemStatus};
use crate::system::processes::{self, ProcessInfo};
use crate::system::{advisor, cleanup, docker, gpu, services};
use serde_json::{json, Value};
use tauri::{AppHandle, State};

/// CPU/memory snapshot for the sidebar.
#[tauri::command]
pub async fn get_system_status(state: State<'_, AppState>) -> AppResult<SystemStatus> {
    state.monitor()?.status()
}

/// Static host information.
#[tauri::command]
pub async fn get_system_info(state: State<'_, AppState>) -> AppResult<SystemInfo> {
    state.monitor()?.info()
}

/// Live metrics.
#[tauri::command]
pub async fn get_real_time_stats(state: State<'_, AppState>) -> AppResult<RealTimeStats> {
    state.monitor()?.real_time()
}

/// Top processes by CPU.
#[tauri::command]
pub async fn get_processes(state: State<'_, AppState>) -> AppResult<Vec<ProcessInfo>> {
    let mut monitor = state.monitor()?;
    Ok(processes::list(&mut monitor))
}

/// Terminate a process (native confirmation + audit).
#[tauri::command]
pub async fn kill_process(app: AppHandle, state: State<'_, AppState>, pid: u32) -> AppResult<()> {
    processes::kill(&app, &state, pid).await
}

/// Everything the Performance view needs: host details, GPUs, history,
/// agent/model metrics and memory-store stats.
#[tauri::command]
pub async fn get_performance(
    state: State<'_, AppState>,
    history_secs: Option<u64>,
) -> AppResult<Value> {
    let host: DetailedStats = state.monitor()?.detailed()?;
    let gpus = gpu::snapshot().await;
    let history = state
        .history
        .lock()
        .map_err(|_| AppError::Internal("history lock poisoned".into()))?
        .since(Some(history_secs.unwrap_or(900).min(3600)));
    let (installed, loaded) = tokio::join!(
        crate::ai::ollama_admin::installed(&state),
        crate::ai::ollama_admin::loaded(&state)
    );
    let kb = match crate::memory::from_state(&state).await {
        Ok(Some(store)) => store.stats().await.ok(),
        _ => None,
    };
    let model = state.settings.read().await.ai.ollama_model.clone();
    Ok(json!({
        "host": host,
        "gpus": gpus,
        "history": history,
        "agent": state.agent_metrics.snapshot(),
        "models": {
            "configured": model,
            "installed": installed.as_ref().ok(),
            "loaded": loaded.as_ref().ok(),
            "error": loaded.as_ref().err().map(ToString::to_string),
        },
        "memory_store": kb.map(|s| json!({
            "embed_model": s.get("embed_model"), "llm_model": s.get("llm_model"),
            "avg_search_ms": s.get("avg_search_ms"), "searches_24h": s.get("searches_24h"),
            "vectors": s.get("vectors"), "pending": s.get("pending_embeddings"),
            "storage_bytes": s.get("storage_bytes"),
        })),
    }))
}

/// GPUs only (cheap poll).
#[tauri::command]
pub async fn get_gpus() -> AppResult<Vec<gpu::GpuInfo>> {
    Ok(gpu::snapshot().await)
}

/// systemd services (system + user).
#[tauri::command]
pub async fn list_services() -> AppResult<Vec<services::Service>> {
    Ok(services::list().await)
}

/// Start/stop/restart/reload/enable/disable a service (native confirmation;
/// system units also get the desktop's polkit prompt).
#[tauri::command]
pub async fn toggle_service(
    app: AppHandle,
    state: State<'_, AppState>,
    scope: String,
    unit: String,
    action: String,
) -> AppResult<Value> {
    let r = services::control(&app, &state, &scope, &unit, &action, Source::User).await?;
    Ok(json!({ "exit_code": r.exit_code, "output": format!("{}{}", r.stdout, r.stderr) }))
}

/// Journal lines for a service.
#[tauri::command]
pub async fn service_logs(
    app: AppHandle,
    state: State<'_, AppState>,
    scope: String,
    unit: String,
    lines: Option<u32>,
) -> AppResult<String> {
    services::logs(
        &app,
        &state,
        &scope,
        &unit,
        lines.unwrap_or(200),
        Source::User,
    )
    .await
}

/// Docker containers.
#[tauri::command]
pub async fn list_containers() -> AppResult<docker::DockerStatus> {
    Ok(docker::list().await)
}

/// Start/stop/restart/pause/unpause a container (native confirmation).
#[tauri::command]
pub async fn control_container(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    action: String,
) -> AppResult<Value> {
    let r = docker::control(&app, &state, &name, &action, Source::User).await?;
    Ok(json!({ "exit_code": r.exit_code, "output": format!("{}{}", r.stdout, r.stderr) }))
}

/// Container logs.
#[tauri::command]
pub async fn container_logs(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    lines: Option<u32>,
) -> AppResult<String> {
    docker::logs(&app, &state, &name, lines.unwrap_or(200), Source::User).await
}

/// Reclaimable space by category.
#[tauri::command]
pub async fn scan_cleanup(state: State<'_, AppState>) -> AppResult<Vec<cleanup::CleanupItem>> {
    cleanup::scan(&state).await
}

/// Clean the selected categories (native confirmation, audited).
#[tauri::command]
pub async fn run_system_cleanup(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> AppResult<Vec<cleanup::CleanupResult>> {
    cleanup::run(&app, &state, &ids).await
}

/// Optimization findings.
#[tauri::command]
pub async fn optimize_system(
    state: State<'_, AppState>,
) -> AppResult<Vec<advisor::Recommendation>> {
    Ok(advisor::analyse(&state).await)
}

/// Apply a recommendation's fix. Only known kinds, each through its own
/// guarded path.
#[tauri::command]
pub async fn apply_recommendation(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    params: Value,
) -> AppResult<String> {
    let s = |k: &str| {
        params
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    match kind.as_str() {
        "unload_model" => {
            let m = s("model");
            crate::ai::ollama_admin::unload(&state, &m, Source::User).await?;
            Ok(format!("Unloaded {m}"))
        }
        "restart_service" => {
            let r = services::control(
                &app,
                &state,
                &s("scope"),
                &s("unit"),
                "restart",
                Source::User,
            )
            .await?;
            Ok(if r.exit_code == Some(0) {
                format!("Restarted {}", s("unit"))
            } else {
                format!("systemctl exited {:?}: {}", r.exit_code, r.stderr.trim())
            })
        }
        "kb_maintenance" => {
            let r = crate::memory::require(&state).await?.maintenance().await?;
            Ok(format!(
                "Optimized memory: merged {} duplicates, embedded {} items",
                r.get("merged_duplicates")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                r.get("embedded_pending")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
            ))
        }
        other => Err(AppError::InvalidInput(format!("unknown fix `{other}`"))),
    }
}
