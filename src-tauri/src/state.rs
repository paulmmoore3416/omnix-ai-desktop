//! Application state managed by Tauri (`app.manage(AppState)`).
//!
//! Replaces the global `Lazy<Mutex<..>>` statics of earlier versions. Commands
//! receive it as `tauri::State<'_, AppState>`.

use crate::error::{AppError, AppResult};
use crate::security::audit::AuditLog;
use crate::security::policy::PolicyConfig;
use crate::security::secrets::{KeyringStore, SecretStore};
use crate::settings::{self, Settings};
use crate::system::metrics::Monitor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime};

/// Shared state for all commands.
pub struct AppState {
    /// Current settings (source of truth in memory; persisted on save).
    pub settings: tokio::sync::RwLock<Settings>,
    /// Serializes settings writes so a confirmation dialog cannot race
    /// another save (time-of-check/time-of-use).
    pub settings_write: tokio::sync::Mutex<()>,
    /// Location of `settings.json`.
    pub settings_path: PathBuf,
    /// Hash-chained audit log.
    pub audit: AuditLog,
    /// Single shared HTTP client (connection pooling, consistent timeouts).
    pub http: reqwest::Client,
    /// sysinfo state. A std mutex is fine: refreshes are short and the guard
    /// is never held across `.await`.
    pub monitor: Mutex<Monitor>,
    /// OS keychain (trait object so tests can substitute a memory store).
    pub secrets: Arc<dyn SecretStore>,
    /// User home directory.
    pub home: Option<PathBuf>,
    /// Chat history for the agent loop (excludes the system prompt). The
    /// mutex also serializes turns: a second message while one is streaming
    /// is rejected.
    pub conversation: tokio::sync::Mutex<Vec<crate::ai::provider::ChatMessage>>,
    /// Set by `chat_cancel`; checked between stream events and tool calls.
    pub chat_cancel: std::sync::atomic::AtomicBool,
    /// MCP server connections.
    pub mcp: crate::mcp::McpManager,
    /// Agent and model metrics since start.
    pub agent_metrics: crate::ai::metrics::AgentMetrics,
    /// Rolling host metrics (filled by the ops engine).
    pub history: Mutex<crate::system::history::History>,
    /// Alerts, automations and scheduled tasks.
    pub ops: crate::ops::OpsState,
    /// Document name of the current conversation's archive (reset by
    /// `chat_reset`).
    pub conversation_doc: Mutex<Option<String>>,
    /// Recent phone sends (`phone.max_per_hour`).
    pub phone_limit: crate::phone::RateLimiter,
}

impl AppState {
    /// Build the state during Tauri `setup`.
    pub fn init<R: Runtime>(app: &AppHandle<R>) -> AppResult<Self> {
        let log_dir = app
            .path()
            .app_log_dir()
            .map_err(|e| AppError::Unavailable(format!("app log directory: {e}")))?;
        let audit = AuditLog::open(log_dir.join("audit.jsonl"))?;
        tracing::info!(path = %audit.path().display(), "audit log opened");

        let secrets: Arc<dyn SecretStore> = Arc::new(KeyringStore);
        let settings_path = settings::default_path()?;
        let settings = settings::load(&settings_path, secrets.as_ref())?;

        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(300))
            .user_agent(concat!("OMNIX/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| AppError::Internal(format!("HTTP client: {e}")))?;

        Ok(Self {
            settings: tokio::sync::RwLock::new(settings),
            settings_write: tokio::sync::Mutex::new(()),
            settings_path,
            audit,
            http,
            monitor: Mutex::new(Monitor::new()),
            secrets,
            home: dirs::home_dir(),
            conversation: tokio::sync::Mutex::new(Vec::new()),
            chat_cancel: std::sync::atomic::AtomicBool::new(false),
            mcp: crate::mcp::McpManager::default(),
            agent_metrics: crate::ai::metrics::AgentMetrics::default(),
            history: Mutex::new(crate::system::history::History::default()),
            ops: crate::ops::OpsState::load(crate::ops::default_path()?),
            conversation_doc: Mutex::new(None),
            phone_limit: crate::phone::RateLimiter::default(),
        })
    }

    /// Files that executed commands may read but never modify.
    pub fn protected_paths(&self) -> Vec<String> {
        let mut v = vec![
            self.settings_path.to_string_lossy().to_string(),
            // Rules that can run commands unattended: never writable by commands.
            self.ops.path().to_string_lossy().to_string(),
        ];
        if let Some(dir) = self.audit.path().parent() {
            v.push(dir.to_string_lossy().to_string());
        }
        v
    }

    /// Policy inputs for a command running in `cwd`.
    pub fn policy_config(&self, settings: &Settings, cwd: Option<&Path>) -> PolicyConfig {
        PolicyConfig {
            allowed_commands: settings.security.allowed_commands.clone(),
            blocked_commands: settings.security.blocked_commands.clone(),
            protected_paths: self.protected_paths(),
            home: self.home.as_ref().map(|h| h.to_string_lossy().to_string()),
            cwd: cwd.map(|c| c.to_string_lossy().to_string()),
        }
    }

    /// Lock the system monitor, mapping a poisoned mutex to an error instead of panicking.
    pub fn monitor(&self) -> AppResult<std::sync::MutexGuard<'_, Monitor>> {
        self.monitor
            .lock()
            .map_err(|_| AppError::Internal("system info lock poisoned".into()))
    }
}
