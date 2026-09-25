//! Voice commands.

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{AppHandle, Manager, State};

/// Transcribe a push-to-talk recording. The audio is sent as the raw
/// invoke body (`Uint8Array`) with its MIME type in the `x-audio-mime` header.
#[tauri::command]
pub async fn voice_transcribe(
    state: State<'_, AppState>,
    request: Request<'_>,
) -> AppResult<String> {
    let InvokeBody::Raw(audio) = request.body() else {
        return Err(AppError::InvalidInput("expected raw audio bytes".into()));
    };
    let mime = request
        .headers()
        .get("x-audio-mime")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("audio/webm")
        .to_string();
    crate::voice::transcribe(&state, audio.clone(), &mime).await
}

/// Synthesize speech with Piper; returns WAV bytes as an `ArrayBuffer`.
#[tauri::command]
pub async fn voice_speak(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> AppResult<Response> {
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|e| AppError::Unavailable(format!("cache directory: {e}")))?;
    Ok(Response::new(
        crate::voice::speak(&state, cache, &text).await?,
    ))
}
