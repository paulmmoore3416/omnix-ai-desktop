//! Settings, secrets and connection-test commands.

use super::not_implemented;
use crate::ai::{endpoint, ollama};
use crate::error::{AppError, AppResult};
use crate::security::audit::{AuditRecord, Confirmation, Decision};
use crate::security::confirm::{self, ConfirmRequest};
use crate::security::policy::{self, RiskTier, Source};
use crate::security::secrets;
use crate::settings::{self as store, AiSettings, MemResortSettings, Settings, SCHEMA_VERSION};
use crate::state::AppState;
use std::time::Duration;
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

/// Maximum size of an imported settings file.
const MAX_IMPORT_BYTES: u64 = 1024 * 1024;

/// Current settings, with `has_*_key` flags refreshed from the keychain.
#[tauri::command]
pub async fn load_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    let mut s = state.settings.read().await.clone();
    let secrets = state.secrets.clone();
    let s = tokio::task::spawn_blocking(move || {
        s.refresh_secret_flags(secrets.as_ref());
        s
    })
    .await?;
    Ok(s)
}

/// Validate and persist settings. Security-weakening changes require native
/// confirmation (see [`Settings::security_changes`]).
#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> AppResult<Settings> {
    apply(&app, &state, settings, "save").await
}

/// Restore defaults (keychain secrets are left untouched).
#[tauri::command]
pub async fn reset_settings(app: AppHandle, state: State<'_, AppState>) -> AppResult<Settings> {
    apply(&app, &state, Settings::default(), "reset").await
}

/// Export settings to a user-chosen JSON file. Secrets are never included
/// (they are not in [`Settings`] at all). Returns the path, or `None` if the
/// dialog was cancelled.
#[tauri::command]
pub async fn export_settings(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<String>> {
    let mut export = state.settings.read().await.clone();
    export.ai.has_openai_key = false;
    export.ai.has_anthropic_key = false;
    export.ai.has_gemini_key = false;
    export.ai.has_xai_key = false;
    export.sanitize();
    let json = serde_json::to_string_pretty(&export)?;

    let dialog_app = app.clone();
    let picked = tokio::task::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .set_title("Export OMNIX settings")
            .add_filter("JSON", &["json"])
            .set_file_name("omnix-settings.json")
            .blocking_save_file()
    })
    .await?;
    let Some(fp) = picked else {
        return Ok(None);
    };
    let path = fp
        .into_path()
        .map_err(|e| AppError::InvalidInput(format!("unsupported location: {e}")))?;
    let home = state.home.as_ref().map(|h| h.to_string_lossy().to_string());
    let p = path.to_string_lossy();
    if policy::is_sensitive_path(&p, home.as_deref())
        || policy::is_protected_path(&p, &state.protected_paths(), home.as_deref())
    {
        return Err(AppError::PolicyDenied(format!(
            "refusing to write settings to {}",
            path.display()
        )));
    }
    tokio::fs::write(&path, json).await?;
    audit(
        &state,
        "settings_export",
        &path.display().to_string(),
        RiskTier::ReadOnly,
        Decision::Allowed,
        Confirmation::NotRequired,
        None,
    )
    .await?;
    Ok(Some(path.display().to_string()))
}

/// Import settings from a user-chosen JSON file. Any API keys in the file are
/// discarded (imports never carry secrets). Goes through the same validation
/// and security-change confirmation as a save. Returns `None` if cancelled.
#[tauri::command]
pub async fn import_settings(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<Settings>> {
    let dialog_app = app.clone();
    let picked = tokio::task::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .set_title("Import OMNIX settings")
            .add_filter("JSON", &["json"])
            .blocking_pick_file()
    })
    .await?;
    let Some(fp) = picked else {
        return Ok(None);
    };
    let path = fp
        .into_path()
        .map_err(|e| AppError::InvalidInput(format!("unsupported location: {e}")))?;
    let meta = tokio::fs::metadata(&path).await?;
    if meta.len() > MAX_IMPORT_BYTES {
        return Err(AppError::InvalidInput(
            "settings file is larger than 1 MiB".into(),
        ));
    }
    let text = tokio::fs::read_to_string(&path).await?;
    let mut raw: serde_json::Value = serde_json::from_str(&text)?;
    if secrets::strip_plaintext(&mut raw) {
        tracing::warn!("imported settings contained API keys; they were ignored");
    }
    store::migrate_schema(&mut raw);
    let imported = store::from_value(raw)?;
    apply(&app, &state, imported, "import").await.map(Some)
}

/// Store an API key/token in the OS keychain. The value is never returned.
#[tauri::command]
pub async fn set_secret(
    state: State<'_, AppState>,
    provider: String,
    value: String,
) -> AppResult<Settings> {
    secrets::validate_provider(&provider)?;
    secrets::validate_value(&value)?;
    let store = state.secrets.clone();
    let p = provider.clone();
    tokio::task::spawn_blocking(move || store.set(&p, &value)).await??;
    audit(
        &state,
        "secret_set",
        &format!("provider={provider}"),
        RiskTier::Mutating,
        Decision::Allowed,
        Confirmation::NotRequired,
        None,
    )
    .await?;
    refresh_flags(&state).await
}

/// Remove an API key/token from the OS keychain.
#[tauri::command]
pub async fn delete_secret(state: State<'_, AppState>, provider: String) -> AppResult<Settings> {
    secrets::validate_provider(&provider)?;
    let store = state.secrets.clone();
    let p = provider.clone();
    tokio::task::spawn_blocking(move || store.delete(&p)).await??;
    audit(
        &state,
        "secret_delete",
        &format!("provider={provider}"),
        RiskTier::Mutating,
        Decision::Allowed,
        Confirmation::NotRequired,
        None,
    )
    .await?;
    refresh_flags(&state).await
}

/// Whether a secret exists for `provider`.
#[tauri::command]
pub async fn has_secret(state: State<'_, AppState>, provider: String) -> AppResult<bool> {
    secrets::validate_provider(&provider)?;
    let store = state.secrets.clone();
    tokio::task::spawn_blocking(move || store.has(&provider)).await?
}

/// Real connectivity check against the configured provider.
#[tauri::command]
pub async fn test_ai_model(state: State<'_, AppState>, config: AiSettings) -> AppResult<String> {
    let local_only = state.settings.read().await.security.local_only;
    endpoint::ensure_provider_allowed(&config.provider, local_only)?;
    if config.provider != "ollama" {
        return not_implemented("testing cloud providers");
    }
    endpoint::ensure_endpoint_allowed(&config.ollama_host, local_only).await?;
    let models = ollama::list_models(&state.http, &config.ollama_host).await?;
    let host = &config.ollama_host;
    if models.is_empty() {
        return Ok(format!(
            "Connected to Ollama at {host}, but no models are installed (run `ollama pull <model>`)."
        ));
    }
    if config.ollama_model.is_empty() {
        return Ok(format!(
            "Connected to Ollama at {host}. Installed models: {}. Select one and save.",
            models.join(", ")
        ));
    }
    if !models.contains(&config.ollama_model) {
        return Err(AppError::InvalidInput(format!(
            "model `{}` is not installed on {host}. Installed: {}",
            config.ollama_model,
            models.join(", ")
        )));
    }
    Ok(format!(
        "Connected to Ollama at {host}; model `{}` is installed ({} models available).",
        config.ollama_model,
        models.len()
    ))
}

/// Check that a MemResort server answers `GET /v1/models`.
#[tauri::command]
pub async fn test_memresort_connection(
    state: State<'_, AppState>,
    config: MemResortSettings,
) -> AppResult<String> {
    let local_only = state.settings.read().await.security.local_only;
    if config.host.trim().is_empty() || config.host.contains(['/', ' ', '@']) {
        return Err(AppError::InvalidInput(
            "host must be a bare host name or IP".into(),
        ));
    }
    let url = format!("http://{}:{}/v1/models", config.host, config.port);
    endpoint::ensure_endpoint_allowed(&url, local_only).await?;
    let resp = state
        .http
        .get(&url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| {
            AppError::Unavailable(format!("MemResort at {}:{}: {e}", config.host, config.port))
        })?;
    if resp.status().is_success() {
        Ok(format!(
            "MemResort is reachable at {}:{}",
            config.host, config.port
        ))
    } else {
        Err(AppError::Unavailable(format!(
            "MemResort responded with {}",
            resp.status()
        )))
    }
}

/// Not implemented: legacy integration tests (to be replaced by MCP servers).
#[tauri::command]
pub async fn test_integration() -> AppResult<String> {
    not_implemented("integration connection tests")
}

/// Shared save path for save/reset/import.
async fn apply<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    mut new: Settings,
    how: &str,
) -> AppResult<Settings> {
    new.validate()?;
    let _write = state.settings_write.lock().await;
    let current = state.settings.read().await.clone();

    // Keychain-derived and schema fields are owned by the backend.
    new.schema_version = SCHEMA_VERSION;
    new.ai.has_openai_key = current.ai.has_openai_key;
    new.ai.has_anthropic_key = current.ai.has_anthropic_key;
    new.ai.has_gemini_key = current.ai.has_gemini_key;
    new.ai.has_xai_key = current.ai.has_xai_key;
    new.sanitize();

    let changes = current.security_changes(&new);
    if !changes.is_empty() {
        let c = confirm::ask(
            app,
            &ConfirmRequest {
                title: "Change security settings?".into(),
                subject: format!(
                    "Settings {how} will change these security controls:\n{}",
                    changes
                        .iter()
                        .map(|c| format!("  • {c}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
                details: vec![],
                tier: RiskTier::Mutating,
                source: Source::User,
                reasons: vec!["weakens or redirects OMNIX's protections".into()],
                approve_label: "Apply".into(),
            },
            Duration::from_secs(current.security.confirmation_timeout_secs),
        )
        .await;
        let approved = c == Confirmation::Approved;
        audit(
            state,
            "settings_change",
            &changes.join("; "),
            RiskTier::Mutating,
            if approved {
                Decision::Allowed
            } else {
                Decision::NotApproved
            },
            c,
            Some(format!("via {how}")),
        )
        .await?;
        if !approved {
            return Err(AppError::NotApproved(
                "security settings were not changed".into(),
            ));
        }
    }

    let path = state.settings_path.clone();
    let to_save = new.clone();
    tokio::task::spawn_blocking(move || store::save(&path, &to_save)).await??;
    *state.settings.write().await = new.clone();
    Ok(new)
}

async fn refresh_flags(state: &AppState) -> AppResult<Settings> {
    let secrets = state.secrets.clone();
    let mut s = state.settings.read().await.clone();
    let s = tokio::task::spawn_blocking(move || {
        s.refresh_secret_flags(secrets.as_ref());
        s
    })
    .await?;
    *state.settings.write().await = s.clone();
    Ok(s)
}

async fn audit(
    state: &AppState,
    action: &str,
    command: &str,
    tier: RiskTier,
    decision: Decision,
    confirmation: Confirmation,
    detail: Option<String>,
) -> AppResult<()> {
    state
        .audit
        .record(AuditRecord {
            id: uuid::Uuid::new_v4().to_string(),
            source: Source::User,
            action: action.into(),
            command: command.into(),
            cwd: None,
            tier,
            decision,
            confirmation,
            exit_code: None,
            duration_ms: None,
            detail,
        })
        .await
}
