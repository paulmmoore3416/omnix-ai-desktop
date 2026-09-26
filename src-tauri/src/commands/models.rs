//! Ollama model manager commands.

use crate::ai::ollama_admin::{self, InstalledModel, LoadedModel, PullProgress};
use crate::error::AppResult;
use crate::security::policy::Source;
use crate::state::AppState;
use serde_json::{json, Value};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

/// Installed and loaded models.
#[tauri::command]
pub async fn list_models_detail(state: State<'_, AppState>) -> AppResult<Value> {
    let installed: Vec<InstalledModel> = ollama_admin::installed(&state).await?;
    let loaded: Vec<LoadedModel> = ollama_admin::loaded(&state).await?;
    let configured = state.settings.read().await.ai.ollama_model.clone();
    Ok(json!({ "installed": installed, "loaded": loaded, "configured": configured }))
}

/// Load a model into memory now.
#[tauri::command]
pub async fn model_load(
    state: State<'_, AppState>,
    model: String,
    keep_alive: Option<String>,
) -> AppResult<()> {
    ollama_admin::load(
        &state,
        &model,
        keep_alive.as_deref().unwrap_or("30m"),
        Source::User,
    )
    .await
}

/// Unload a model (frees its VRAM/RAM).
#[tauri::command]
pub async fn model_unload(state: State<'_, AppState>, model: String) -> AppResult<()> {
    ollama_admin::unload(&state, &model, Source::User).await
}

/// Delete a model (native confirmation).
#[tauri::command]
pub async fn model_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    model: String,
) -> AppResult<()> {
    ollama_admin::delete(&app, &state, &model, Source::User).await
}

/// Download a model (native confirmation); progress streams over `on_progress`.
#[tauri::command]
pub async fn model_pull(
    app: AppHandle,
    state: State<'_, AppState>,
    model: String,
    on_progress: Channel<PullProgress>,
) -> AppResult<()> {
    let send = move |p: PullProgress| {
        let _ = on_progress.send(p);
    };
    ollama_admin::pull(&app, &state, &model, Source::User, &send).await
}
