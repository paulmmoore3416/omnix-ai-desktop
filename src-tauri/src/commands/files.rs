//! Native file picker. Raw file read/write is *not* exposed over IPC; see
//! `security::files` for the guarded operations used by chat commands.

use crate::error::{AppError, AppResult};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

/// Show a native "open file" dialog and return the chosen path (or `None`
/// if cancelled). The user, not the webview, chooses the path.
#[tauri::command]
pub async fn select_file(app: AppHandle) -> AppResult<Option<String>> {
    let picked = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Select a document")
            .blocking_pick_file()
    })
    .await?;
    picked
        .map(|fp| {
            fp.into_path()
                .map(|p| p.display().to_string())
                .map_err(|e| AppError::InvalidInput(format!("unsupported file location: {e}")))
        })
        .transpose()
}
