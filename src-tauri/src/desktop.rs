//! Desktop integration (launch at login, and later tray / shortcuts / window state).

use crate::error::{AppError, AppResult};
use tauri::{AppHandle, Runtime};
use tauri_plugin_autostart::ManagerExt;

/// Make the OS launch-at-login entry match `enabled`.
///
/// Replaces the old hand-written `.desktop` file (which hardcoded a user's
/// home path). The plugin writes the platform-appropriate entry pointing at
/// the *current* executable: XDG autostart on Linux, a LaunchAgent on macOS,
/// or the registry Run key on Windows. Only touches the OS when the state
/// differs, so startup is idempotent.
pub fn apply_autostart<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> AppResult<()> {
    let launcher = app.autolaunch();
    let current = launcher
        .is_enabled()
        .map_err(|e| AppError::Unavailable(format!("autostart status: {e}")))?;
    if current == enabled {
        return Ok(());
    }
    let result = if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    };
    result.map_err(|e| AppError::Unavailable(format!("could not update launch-at-login: {e}")))?;
    tracing::info!(enabled, "launch-at-login updated");
    Ok(())
}
