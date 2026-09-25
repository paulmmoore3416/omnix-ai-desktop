//! Persistent user settings (`<config_dir>/omnix/settings.json`).
//!
//! * The wire format (IPC and disk) is snake_case, matching files written by
//!   earlier versions.
//! * Every section is `#[serde(default)]`, so files that are missing sections
//!   or fields (older versions) still load.
//! * **No secrets live here.** API keys are in the OS keychain
//!   ([`crate::security::secrets`]); settings only carry `has_*_key` flags,
//!   which are recomputed from the keychain on every load and save.

use crate::error::{AppError, AppResult};
use crate::security::secrets::{self, SecretStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Current on-disk schema version. See [`migrate_schema`].
pub const SCHEMA_VERSION: u32 = 2;

/// Root settings document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// On-disk schema version (absent in files written before v2).
    pub schema_version: u32,
    /// UI and desktop-integration preferences.
    pub general: GeneralSettings,
    /// LLM provider configuration.
    pub ai: AiSettings,
    /// Optional MemResort endpoint.
    pub memresort: MemResortSettings,
    /// Legacy third-party integration toggles (secrets stripped). Replaced by
    /// MCP servers in a later phase.
    pub integrations: serde_json::Map<String, Value>,
    /// Voice input/output (planned).
    pub voice: VoiceSettings,
    /// Long-term memory backend (planned).
    pub memory: MemorySettings,
    /// Security controls.
    pub security: SecuritySettings,
    /// Performance tuning.
    pub performance: PerformanceSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            general: GeneralSettings::default(),
            ai: AiSettings::default(),
            memresort: MemResortSettings::default(),
            integrations: serde_json::Map::new(),
            voice: VoiceSettings::default(),
            memory: MemorySettings::default(),
            security: SecuritySettings::default(),
            performance: PerformanceSettings::default(),
        }
    }
}

/// General UI preferences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralSettings {
    /// UI theme id.
    pub theme: String,
    /// UI language code.
    pub language: String,
    /// Launch at login.
    pub auto_start: bool,
    /// Show desktop notifications.
    pub notifications: bool,
    /// Play UI sounds.
    pub sound_effects: bool,
    /// Hide to the system tray instead of quitting on close.
    pub minimize_to_tray: bool,
    /// Check for application updates.
    pub check_updates: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            theme: "cosmic".into(),
            language: "en".into(),
            auto_start: false,
            notifications: true,
            sound_effects: true,
            minimize_to_tray: true,
            check_updates: false,
        }
    }
}

/// LLM provider configuration. Keys are never stored here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSettings {
    /// `ollama`, `openai`, `anthropic`, `gemini`, `xai`.
    pub provider: String,
    /// Ollama base URL.
    pub ollama_host: String,
    /// Selected Ollama model; empty until the user picks an installed model.
    pub ollama_model: String,
    /// Keychain has an OpenAI key (read-only for the frontend).
    pub has_openai_key: bool,
    /// Keychain has an Anthropic key.
    pub has_anthropic_key: bool,
    /// Keychain has a Gemini key.
    pub has_gemini_key: bool,
    /// Keychain has an xAI key.
    pub has_xai_key: bool,
    /// Sampling temperature (0–2).
    pub temperature: f32,
    /// Maximum tokens to generate per response.
    pub max_tokens: u32,
    /// Stream tokens to the UI.
    pub stream_responses: bool,
    /// Model context window in tokens.
    pub context_window: u32,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            provider: "ollama".into(),
            ollama_host: "http://localhost:11434".into(),
            ollama_model: String::new(),
            has_openai_key: false,
            has_anthropic_key: false,
            has_gemini_key: false,
            has_xai_key: false,
            temperature: 0.7,
            max_tokens: 2048,
            stream_responses: true,
            context_window: 8192,
        }
    }
}

/// Optional MemResort OpenAI-compatible endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemResortSettings {
    /// Enable the connection.
    pub enabled: bool,
    /// Host name or IP.
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// Connect at startup.
    pub auto_connect: bool,
}

impl Default for MemResortSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            host: "localhost".into(),
            port: 8080,
            auto_connect: false,
        }
    }
}

/// Voice settings (feature planned; stored for forward compatibility).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceSettings {
    /// Master switch (off until voice is implemented).
    pub enabled: bool,
    /// Whisper model size.
    pub whisper_model: String,
    /// Recognition language.
    pub language: String,
    /// TTS engine id.
    pub tts_engine: String,
    /// TTS voice id.
    pub tts_voice: String,
    /// Wake word (unused; push-to-talk only).
    pub wake_word: String,
    /// Always-on microphone. Must stay off by default.
    pub continuous_listening: bool,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            whisper_model: "base".into(),
            language: "en".into(),
            tts_engine: "piper".into(),
            tts_voice: "en_US-lessac-medium".into(),
            wake_word: "omnix".into(),
            continuous_listening: false,
        }
    }
}

/// Long-term memory settings (feature planned).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemorySettings {
    /// Maximum stored memories.
    pub max_memory_size: u32,
    /// Summarize conversations automatically.
    pub auto_summarize: bool,
    /// Retention in days.
    pub retention_days: u32,
    /// Enable semantic search.
    pub enable_semantic_search: bool,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            max_memory_size: 1000,
            auto_summarize: false,
            retention_days: 90,
            enable_semantic_search: true,
        }
    }
}

/// Security controls. See `docs/SECURITY.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SecuritySettings {
    /// Also confirm `ReadOnly` commands (Mutating/Privileged are always confirmed).
    pub require_confirmation: bool,
    /// Allow `Privileged` commands via pkexec/osascript/UAC. Off by default.
    pub enable_sudo: bool,
    /// Hard-block cloud providers and non-local endpoints (PHI egress control).
    pub local_only: bool,
    /// Informational: every command is always logged to the audit log.
    pub log_all_commands: bool,
    /// Planned: encrypt memory at rest.
    pub encrypt_memory: bool,
    /// Informational: the audit log is always on.
    pub audit_log: bool,
    /// Allowlist; when non-empty only matching commands may run.
    pub allowed_commands: Vec<String>,
    /// Additional blocked command prefixes (built-in rules always apply).
    pub blocked_commands: Vec<String>,
    /// Kill commands that run longer than this.
    pub command_timeout_secs: u64,
    /// Cap on captured stdout/stderr per stream.
    pub max_output_bytes: usize,
    /// Unanswered confirmation dialogs are denied after this long.
    pub confirmation_timeout_secs: u64,
    /// Allow the agent loop to chain tool calls without a user message per step.
    pub autonomous_mode: bool,
    /// Upper bound on agent-loop steps per user message.
    pub max_autonomous_steps: u32,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            require_confirmation: false,
            enable_sudo: false,
            local_only: true,
            log_all_commands: true,
            encrypt_memory: false,
            audit_log: true,
            allowed_commands: vec![],
            blocked_commands: vec![],
            command_timeout_secs: 60,
            max_output_bytes: 1024 * 1024,
            confirmation_timeout_secs: 60,
            autonomous_mode: false,
            max_autonomous_steps: 10,
        }
    }
}

/// Performance tuning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceSettings {
    /// Concurrency cap for background tasks.
    pub max_concurrent_tasks: u32,
    /// Enable caching.
    pub cache_enabled: bool,
    /// Cache size in MB.
    pub cache_size: u32,
    /// UI polling interval in ms.
    pub monitoring_interval: u32,
    /// Poll faster under load.
    pub adaptive_refresh: bool,
    /// Reduce background work.
    pub low_power_mode: bool,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 5,
            cache_enabled: true,
            cache_size: 500,
            monitoring_interval: 5000,
            adaptive_refresh: true,
            low_power_mode: false,
        }
    }
}

impl Settings {
    /// Reject out-of-range or malformed values before they are persisted.
    pub fn validate(&self) -> AppResult<()> {
        let bad = |m: &str| Err(AppError::InvalidInput(m.to_string()));
        let ai = &self.ai;
        if !(0.0..=2.0).contains(&ai.temperature) || ai.temperature.is_nan() {
            return bad("ai.temperature must be between 0 and 2");
        }
        if !(1..=200_000).contains(&ai.max_tokens) {
            return bad("ai.max_tokens must be between 1 and 200000");
        }
        if !(512..=2_000_000).contains(&ai.context_window) {
            return bad("ai.context_window must be between 512 and 2000000");
        }
        if !["ollama", "openai", "anthropic", "gemini", "xai"].contains(&ai.provider.as_str()) {
            return bad("ai.provider is not a supported provider");
        }
        match reqwest::Url::parse(&ai.ollama_host) {
            Ok(u) if matches!(u.scheme(), "http" | "https") => {}
            _ => return bad("ai.ollama_host must be an http(s) URL"),
        }
        if self.memresort.host.trim().is_empty() || self.memresort.host.contains(['/', ' ', '@']) {
            return bad("memresort.host must be a bare host name or IP");
        }
        let s = &self.security;
        if !(1..=3600).contains(&s.command_timeout_secs) {
            return bad("security.command_timeout_secs must be between 1 and 3600");
        }
        if !(5..=600).contains(&s.confirmation_timeout_secs) {
            return bad("security.confirmation_timeout_secs must be between 5 and 600");
        }
        if !(1024..=16 * 1024 * 1024).contains(&s.max_output_bytes) {
            return bad("security.max_output_bytes must be between 1 KiB and 16 MiB");
        }
        if !(1..=50).contains(&s.max_autonomous_steps) {
            return bad("security.max_autonomous_steps must be between 1 and 50");
        }
        for list in [&s.allowed_commands, &s.blocked_commands] {
            if list.len() > 500 || list.iter().any(|e| e.len() > 300) {
                return bad("command lists are limited to 500 entries of 300 characters");
            }
        }
        if !(500..=600_000).contains(&self.performance.monitoring_interval) {
            return bad("performance.monitoring_interval must be between 500 and 600000 ms");
        }
        Ok(())
    }

    /// Security-relevant differences between `self` (current) and `new`, as
    /// human-readable lines. A non-empty result requires native confirmation
    /// before `new` is saved, so a compromised webview cannot silently weaken
    /// protections.
    pub fn security_changes(&self, new: &Settings) -> Vec<String> {
        let (o, n) = (&self.security, &new.security);
        let mut out = Vec::new();
        if !o.enable_sudo && n.enable_sudo {
            out.push("Allow privileged (elevated) commands".to_string());
        }
        if o.local_only && !n.local_only {
            out.push(
                "Turn OFF local-only mode (allows cloud providers and remote endpoints)".into(),
            );
        }
        if o.require_confirmation && !n.require_confirmation {
            out.push("Stop confirming read-only commands".into());
        }
        if !o.autonomous_mode && n.autonomous_mode {
            out.push("Enable autonomous agent mode".into());
        }
        if n.max_autonomous_steps > o.max_autonomous_steps {
            out.push(format!(
                "Raise max autonomous steps {} → {}",
                o.max_autonomous_steps, n.max_autonomous_steps
            ));
        }
        let removed: Vec<&String> = o
            .blocked_commands
            .iter()
            .filter(|b| !n.blocked_commands.contains(b))
            .collect();
        if !removed.is_empty() {
            out.push(format!(
                "Remove blocked-command rules: {}",
                removed
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let added: Vec<&String> = n
            .allowed_commands
            .iter()
            .filter(|a| !o.allowed_commands.contains(a))
            .collect();
        if !o.allowed_commands.is_empty() && n.allowed_commands.is_empty() {
            out.push("Turn OFF allowlist mode".into());
        } else if !o.allowed_commands.is_empty() && !added.is_empty() {
            out.push(format!(
                "Add allowlist entries: {}",
                added
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if n.command_timeout_secs > o.command_timeout_secs {
            out.push(format!(
                "Raise command timeout {}s → {}s",
                o.command_timeout_secs, n.command_timeout_secs
            ));
        }
        if self.ai.provider != new.ai.provider && new.ai.provider != "ollama" {
            out.push(format!(
                "Switch AI provider to `{}` (cloud)",
                new.ai.provider
            ));
        }
        if self.ai.ollama_host != new.ai.ollama_host {
            out.push(format!("Change Ollama host to {}", new.ai.ollama_host));
        }
        if new.memresort.enabled
            && (self.memresort.host != new.memresort.host
                || self.memresort.port != new.memresort.port
                || !self.memresort.enabled)
        {
            out.push(format!(
                "Send requests to MemResort at {}:{}",
                new.memresort.host, new.memresort.port
            ));
        }
        out
    }

    /// Remove secret-bearing fields from legacy integration entries. Tokens,
    /// API keys and webhook URLs (which embed credentials) must never be
    /// persisted in plaintext.
    pub fn sanitize(&mut self) {
        const SECRET_FIELDS: &[&str] = &[
            "token",
            "apiKey",
            "api_key",
            "apiToken",
            "api_token",
            "webhookUrl",
            "webhook_url",
        ];
        for service in self.integrations.values_mut() {
            if let Some(obj) = service.as_object_mut() {
                for f in SECRET_FIELDS {
                    obj.remove(*f);
                }
            }
        }
    }

    /// Recompute `has_*_key` flags from the keychain. Keychain errors are
    /// logged and reported as "no key" (the UI then offers to set one).
    pub fn refresh_secret_flags(&mut self, store: &dyn SecretStore) {
        let has = |p: &str| match store.has(p) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(provider = p, error = %e, "could not query keychain");
                false
            }
        };
        self.ai.has_openai_key = has("openai");
        self.ai.has_anthropic_key = has("anthropic");
        self.ai.has_gemini_key = has("gemini");
        self.ai.has_xai_key = has("xai");
    }
}

/// Default settings location: `<config_dir>/omnix/settings.json`.
pub fn default_path() -> AppResult<PathBuf> {
    let mut p = dirs::config_dir()
        .ok_or_else(|| AppError::Unavailable("no user config directory".into()))?;
    p.push("omnix");
    p.push("settings.json");
    Ok(p)
}

/// Parse a raw settings document into [`Settings`], stripping any secret
/// fields first (so a plaintext key can never be deserialized into memory
/// through an unexpected field).
pub fn from_value(mut raw: Value) -> AppResult<Settings> {
    secrets::strip_plaintext(&mut raw);
    Ok(serde_json::from_value(raw)?)
}

/// Load settings from `path`, migrating plaintext secrets into `store`.
///
/// * Missing file → defaults.
/// * Corrupt file → moved aside to `settings.json.corrupt` and defaults used.
/// * Plaintext keys → moved to the keychain and removed from the file. If the
///   keychain is unavailable the file is left untouched (keys are not lost)
///   and a warning is logged.
pub fn load(path: &Path, store: &dyn SecretStore) -> AppResult<Settings> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut s = Settings::default();
            s.refresh_secret_flags(store);
            return Ok(s);
        }
        Err(e) => return Err(e.into()),
    };
    let mut raw: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            let aside = path.with_extension("json.corrupt");
            tracing::error!(error = %e, moved_to = %aside.display(), "settings.json is corrupt; using defaults");
            std::fs::rename(path, &aside)?;
            let mut s = Settings::default();
            s.refresh_secret_flags(store);
            return Ok(s);
        }
    };

    let schema_changed = migrate_schema(&mut raw);

    match secrets::migrate_plaintext(&mut raw, store) {
        Ok(m) if m.changed || schema_changed => {
            if !m.migrated.is_empty() {
                // Never log the values, only which providers were moved.
                tracing::warn!(
                    providers = ?m.migrated,
                    "moved plaintext API keys from settings.json into the OS keychain"
                );
            }
            write_atomic(path, &serde_json::to_string_pretty(&raw)?)?;
        }
        Ok(_) => {}
        Err(e) => {
            if schema_changed {
                // Still persist the security-default reset, but keep the
                // unmigrated keys on disk so they are not lost.
                write_atomic(path, &serde_json::to_string_pretty(&raw)?)?;
            }
            tracing::error!(
            error = %e,
            "could not migrate plaintext API keys to the keychain; they remain in settings.json \
             until the keychain is available (they are ignored by OMNIX in the meantime)"
            )
        }
    }

    let mut settings = from_value(raw)?;
    settings.refresh_secret_flags(store);
    Ok(settings)
}

/// Upgrade a raw settings document written by an older version.
///
/// v1 → v2: v1 shipped `enable_sudo: true` and `encrypt_memory: true` as
/// defaults although neither did anything. v2 makes elevation real, so these
/// are reset to `false` once; the user can re-enable sudo explicitly (with a
/// native confirmation). Returns true if `raw` changed.
pub fn migrate_schema(raw: &mut Value) -> bool {
    let version = raw
        .get("schema_version")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    if version >= u64::from(SCHEMA_VERSION) {
        return false;
    }
    if let Some(sec) = raw.get_mut("security").and_then(Value::as_object_mut) {
        if sec.get("enable_sudo").and_then(Value::as_bool) == Some(true) {
            tracing::warn!(
                "settings v1→v2: resetting security.enable_sudo to false (elevation is now real)"
            );
        }
        sec.insert("enable_sudo".into(), Value::Bool(false));
        sec.insert("encrypt_memory".into(), Value::Bool(false));
    }
    if let Some(obj) = raw.as_object_mut() {
        obj.insert("schema_version".into(), Value::from(SCHEMA_VERSION));
    }
    true
}

/// Persist settings atomically (temp file + rename) with mode 0600 on Unix.
pub fn save(path: &Path, settings: &Settings) -> AppResult<()> {
    write_atomic(path, &serde_json::to_string_pretty(settings)?)
}

/// Write `content` to `path` via a sibling temp file and rename, so a crash
/// never leaves a half-written settings file.
pub fn write_atomic(path: &Path, content: &str) -> AppResult<()> {
    let dir = path
        .parent()
        .ok_or_else(|| AppError::Internal("settings path has no parent".into()))?;
    std::fs::create_dir_all(dir)?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).write(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::secrets::MemoryStore;
    use serde_json::json;

    #[test]
    fn defaults_are_secure() {
        let s = Settings::default();
        assert!(!s.security.enable_sudo);
        assert!(s.security.local_only);
        assert!(!s.security.autonomous_mode);
        assert!(!s.voice.continuous_listening);
        assert!(s.ai.ollama_model.is_empty(), "no hardcoded model id");
        s.validate().expect("defaults validate");
    }

    #[test]
    fn loads_legacy_file_missing_sections() {
        let raw = json!({ "general": { "theme": "dark" }, "ai": { "ollama_model": "llama3" } });
        let s = from_value(raw).expect("parse");
        assert_eq!(s.general.theme, "dark");
        assert_eq!(s.ai.ollama_model, "llama3");
        assert!(s.security.local_only);
    }

    #[test]
    fn load_migrates_and_rewrites_file_without_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            json!({ "ai": { "provider": "ollama", "openai_key": "sk-secret-value" } }).to_string(),
        )
        .expect("write");
        let store = MemoryStore::default();
        let s = load(&path, &store).expect("load");
        assert!(s.ai.has_openai_key);
        let on_disk = std::fs::read_to_string(&path).expect("read");
        assert!(!on_disk.contains("sk-secret-value"));
        assert_eq!(
            store.get("openai").expect("get").as_deref(),
            Some("sk-secret-value")
        );
    }

    #[test]
    fn v1_file_gets_sudo_reset_once() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            json!({ "security": { "enable_sudo": true, "encrypt_memory": true, "blocked_commands": ["dd if="] } })
                .to_string(),
        )
        .expect("write");
        let s = load(&path, &MemoryStore::default()).expect("load");
        assert!(!s.security.enable_sudo);
        assert_eq!(s.schema_version, SCHEMA_VERSION);
        assert_eq!(s.security.blocked_commands, vec!["dd if=".to_string()]);
        // A v2 file keeps an explicit opt-in.
        let mut v2 = s.clone();
        v2.security.enable_sudo = true;
        save(&path, &v2).expect("save");
        assert!(
            load(&path, &MemoryStore::default())
                .expect("load")
                .security
                .enable_sudo
        );
    }

    #[test]
    fn corrupt_file_is_moved_aside() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ not json").expect("write");
        let s = load(&path, &MemoryStore::default()).expect("load");
        assert_eq!(s, {
            let mut d = Settings::default();
            d.refresh_secret_flags(&MemoryStore::default());
            d
        });
        assert!(dir.path().join("settings.json.corrupt").exists());
    }

    #[test]
    fn save_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sub").join("settings.json");
        let mut s = Settings::default();
        s.general.theme = "dark".into();
        save(&path, &s).expect("save");
        let back = load(&path, &MemoryStore::default()).expect("load");
        assert_eq!(back.general.theme, "dark");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn security_weakening_is_detected() {
        let old = Settings::default();
        let mut new = old.clone();
        assert!(old.security_changes(&new).is_empty());
        new.security.enable_sudo = true;
        new.security.local_only = false;
        new.ai.provider = "openai".into();
        let changes = old.security_changes(&new);
        assert_eq!(changes.len(), 3, "{changes:?}");
    }

    #[test]
    fn validation_rejects_bad_values() {
        let mut s = Settings::default();
        s.ai.temperature = 5.0;
        assert!(s.validate().is_err());
        let mut s = Settings::default();
        s.ai.ollama_host = "file:///etc/passwd".into();
        assert!(s.validate().is_err());
        let mut s = Settings::default();
        s.memresort.host = "evil.com/path".into();
        assert!(s.validate().is_err());
    }
}
