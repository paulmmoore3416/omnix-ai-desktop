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

/// Bring the main window to the front (tray click, second instance, shortcut).
pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Tray icon with "Show OMNIX" / "Quit". Clicking the icon also shows the window.
pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let show = MenuItem::with_id(app, "show", "Show OMNIX", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit OMNIX", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut builder = TrayIconBuilder::with_id("omnix")
        .tooltip("OMNIX")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Push-to-talk shortcut. Pressing focuses OMNIX and emits `ptt` with
/// `"pressed"`; releasing emits `"released"`. The frontend only records while
/// voice input is configured.
pub const PTT_SHORTCUT: &str = "ctrl+space";

/// Register [`PTT_SHORTCUT`]. Failure (e.g. the shortcut is taken by the
/// input-method switcher) is logged, not fatal.
pub fn setup_shortcuts<R: Runtime>(app: &AppHandle<R>) {
    use tauri::Emitter;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
    let result = app
        .global_shortcut()
        .on_shortcut(PTT_SHORTCUT, |app, _shortcut, event| {
            let state = match event.state {
                ShortcutState::Pressed => {
                    show_main(app);
                    "pressed"
                }
                ShortcutState::Released => "released",
            };
            let _ = app.emit("ptt", state);
        });
    if let Err(e) = result {
        tracing::warn!(error = %e, shortcut = PTT_SHORTCUT, "could not register push-to-talk shortcut");
    }
}

/// Linux/WebKitGTK: enable `getUserMedia` and answer permission requests.
///
/// * Only **audio-only** capture requests are granted, and only while voice
///   input is enabled with a speech-to-text endpoint configured. Everything
///   else (camera, screen capture, geolocation, notifications, …) is denied.
/// * Recorded audio can only leave the machine through `voice_transcribe`,
///   which enforces `local_only` on the STT endpoint (the CSP blocks the
///   webview from sending it anywhere itself).
///
/// macOS/Windows webviews prompt through the OS, so nothing is needed there.
pub fn setup_microphone<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(target_os = "linux")]
    {
        use tauri::Manager;
        let Some(window) = app.get_webview_window("main") else {
            return;
        };
        let handle = app.clone();
        let result = window.with_webview(move |wv| {
            use webkit2gtk::glib::prelude::Cast;
            use webkit2gtk::{
                PermissionRequestExt, SettingsExt, UserMediaPermissionRequest,
                UserMediaPermissionRequestExt, WebViewExt,
            };
            let view = wv.inner();
            if let Some(settings) = WebViewExt::settings(&view) {
                settings.set_enable_media_stream(true);
            }
            view.connect_permission_request(move |_, req| {
                let voice_ready = handle
                    .try_state::<crate::state::AppState>()
                    .and_then(|s| {
                        s.settings
                            .try_read()
                            .ok()
                            .map(|s| s.voice.enabled && !s.voice.stt_url.trim().is_empty())
                    })
                    .unwrap_or(false);
                let audio_only = req
                    .downcast_ref::<UserMediaPermissionRequest>()
                    .is_some_and(|m| m.is_for_audio_device() && !m.is_for_video_device());
                if voice_ready && audio_only {
                    req.allow();
                } else {
                    req.deny();
                }
                true
            });
        });
        if let Err(e) = result {
            tracing::warn!(error = %e, "could not configure microphone access");
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = app;
}
