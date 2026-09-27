//! Settings → Google commands. The services themselves live in `crate::google`.

use crate::error::AppResult;
use crate::google::{self, GoogleStatus};
use crate::state::AppState;
use tauri::{AppHandle, State};

/// Connection status (never includes a secret).
#[tauri::command]
pub async fn google_status(state: State<'_, AppState>) -> AppResult<GoogleStatus> {
    google::status(&state).await
}

/// Open Google's consent page in the browser and wait for the redirect.
/// The webview only triggers it; the client, scopes and redirect are chosen
/// in Rust from saved (natively confirmed) settings.
#[tauri::command]
pub async fn google_connect(app: AppHandle, state: State<'_, AppState>) -> AppResult<String> {
    google::connect(&app, &state).await
}

/// Revoke and forget the Google connection.
#[tauri::command]
pub async fn google_disconnect(state: State<'_, AppState>) -> AppResult<GoogleStatus> {
    google::disconnect(&state).await?;
    google::status(&state).await
}
