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
    /// MCP servers (replace the old placeholder integrations).
    pub mcp: McpSettings,
    /// Optional external log shipping.
    pub observability: ObservabilitySettings,
    /// Voice input/output (planned).
    pub voice: VoiceSettings,
    /// Long-term memory backend (planned).
    pub memory: MemorySettings,
    /// Security controls.
    pub security: SecuritySettings,
    /// Performance tuning.
    pub performance: PerformanceSettings,
    /// Texts and calls to the owner's phone (Twilio, outbound only).
    pub phone: PhoneSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            general: GeneralSettings::default(),
            ai: AiSettings::default(),
            memresort: MemResortSettings::default(),
            mcp: McpSettings::default(),
            observability: ObservabilitySettings::default(),
            voice: VoiceSettings::default(),
            memory: MemorySettings::default(),
            security: SecuritySettings::default(),
            performance: PerformanceSettings::default(),
            phone: PhoneSettings::default(),
        }
    }
}

/// Outbound texts and calls to the owner's phone through Twilio. Off by
/// default. The Twilio auth token lives in the keychain (`twilio`); the
/// account SID and numbers are identifiers, not secrets. This file never
/// leaves the machine, so the numbers are not in the repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PhoneSettings {
    /// Allow OMNIX to send texts and place calls. Enabling it is a
    /// confirmed change: message text goes to Twilio even in local-only mode.
    pub enabled: bool,
    /// Twilio account SID (`AC` + 32 hex digits).
    pub account_sid: String,
    /// The Twilio number messages come from (E.164, e.g. `+15551234567`).
    pub from_number: String,
    /// The owner's phone (E.164). The only recipient OMNIX ever uses.
    pub to_number: String,
    /// Cap on texts + calls per rolling hour, so a flapping alert or a
    /// runaway rule cannot run up a bill (1–60).
    pub max_per_hour: u32,
}

impl Default for PhoneSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            account_sid: String::new(),
            from_number: String::new(),
            to_number: String::new(),
            max_per_hour: 10,
        }
    }
}

/// `+` and 8–15 digits, first digit non-zero (E.164).
pub fn is_e164(n: &str) -> bool {
    n.strip_prefix('+').is_some_and(|d| {
        (8..=15).contains(&d.len()) && d.chars().all(|c| c.is_ascii_digit()) && !d.starts_with('0')
    })
}

/// `AC` + 32 lowercase hex digits. Interpolated into the Twilio URL path,
/// so nothing else is accepted.
pub fn is_twilio_sid(s: &str) -> bool {
    s.len() == 34
        && s.starts_with("AC")
        && s[2..]
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// MCP client configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct McpSettings {
    /// Registered servers.
    pub servers: Vec<McpServerConfig>,
}

/// One MCP server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Identifier (`[a-z0-9_-]{1,32}`), used to namespace tools.
    pub name: String,
    /// Whether the agent may use it.
    #[serde(default)]
    pub enabled: bool,
    /// How to reach it.
    pub transport: McpTransport,
    /// Tools the user declares read-only (called without confirmation unless
    /// `require_confirmation`). Every other tool is treated as Mutating.
    #[serde(default)]
    pub read_only_tools: Vec<String>,
}

/// MCP transport.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpTransport {
    /// Local subprocess speaking MCP over stdio.
    Stdio {
        /// Executable.
        command: String,
        /// Arguments.
        #[serde(default)]
        args: Vec<String>,
        /// Non-secret environment variables.
        #[serde(default)]
        env: std::collections::BTreeMap<String, String>,
        /// Names of environment variables whose values live in the keychain
        /// under `mcp.<server>.<NAME>`.
        #[serde(default)]
        secret_env: Vec<String>,
    },
    /// Remote server speaking streamable HTTP.
    Http {
        /// Endpoint URL.
        url: String,
        /// Send `Authorization: Bearer` with the keychain secret `mcp.<server>.token`.
        #[serde(default)]
        bearer_token: bool,
    },
}

/// Optional audit-log shipping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ObservabilitySettings {
    /// Grafana Loki base URL (e.g. `http://loki:3100`). Empty = off.
    pub loki_url: String,
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
    /// Selected model for the cloud provider (discovered via its models
    /// endpoint; never hardcoded).
    pub cloud_model: String,
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
            cloud_model: String::new(),
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
    /// Master switch.
    pub enabled: bool,
    /// faster-whisper server base URL (OpenAI-compatible
    /// `/v1/audio/transcriptions`). Empty = speech input off.
    pub stt_url: String,
    /// Piper executable (name on PATH or absolute path).
    pub piper_path: String,
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
            stt_url: String::new(),
            piper_path: "piper".into(),
            whisper_model: "base".into(),
            language: "en".into(),
            tts_engine: "piper".into(),
            tts_voice: "en_US-lessac-medium".into(),
            wake_word: "omnix".into(),
            continuous_listening: false,
        }
    }
}

/// Long-term memory settings (kb-core).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemorySettings {
    /// Base URL of the kb-core service. Empty = memory disabled.
    pub backend_url: String,
    /// Before each chat turn, search memory for the user's message and give
    /// the model the relevant hits (as untrusted data). Small local models
    /// rarely decide to call `search_memory` on their own; this makes memory
    /// work without that.
    pub auto_recall: bool,
    /// Maximum memories/snippets injected by auto-recall (1–10).
    pub recall_limit: u32,
    /// Minimum relevance (0–1) for auto-recall hits.
    pub recall_min_score: f32,
    /// After each chat turn, let kb-core's local LLM extract durable facts
    /// from the user's message and store them. Off by default: it records
    /// what you say without an explicit "remember".
    pub auto_capture: bool,
    /// Save each conversation's user/assistant text (never tool output) to
    /// long-term memory as a searchable transcript in the `conversations`
    /// collection. Excluded from automatic recall.
    pub archive_conversations: bool,
    /// Maximum active memories (0 = unlimited). New memories are refused
    /// when it is reached; nothing is deleted automatically.
    pub max_memory_size: u32,
    /// When a conversation is cleared, have the chat model summarize it and
    /// save the summary as a memory (tagged `source: assistant`).
    pub auto_summarize: bool,
    /// Delete archived conversation transcripts older than this many days
    /// (0 = keep forever). Memories and documents are never pruned by age.
    pub retention_days: u32,
    /// Semantic + keyword (hybrid) search; off = keyword-only.
    pub enable_semantic_search: bool,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            backend_url: String::new(),
            auto_recall: true,
            recall_limit: 4,
            recall_min_score: 0.4,
            auto_capture: false,
            archive_conversations: true,
            max_memory_size: 1000,
            auto_summarize: false,
            retention_days: 0,
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
        if !self.memory.backend_url.is_empty() {
            match reqwest::Url::parse(&self.memory.backend_url) {
                Ok(u) if matches!(u.scheme(), "http" | "https") => {}
                _ => return bad("memory.backend_url must be an http(s) URL"),
            }
        }
        if !(1..=10).contains(&self.memory.recall_limit) {
            return bad("memory.recall_limit must be between 1 and 10");
        }
        if self.memory.max_memory_size > 1_000_000 {
            return bad("memory.max_memory_size must be at most 1000000 (0 = unlimited)");
        }
        if self.memory.retention_days > 36_500 {
            return bad("memory.retention_days must be at most 36500 (0 = keep forever)");
        }
        let ph = &self.phone;
        if !ph.account_sid.is_empty() && !is_twilio_sid(&ph.account_sid) {
            return bad("phone.account_sid must look like AC followed by 32 hex digits");
        }
        for (n, what) in [
            (&ph.from_number, "from_number"),
            (&ph.to_number, "to_number"),
        ] {
            if !n.is_empty() && !is_e164(n) {
                return Err(AppError::InvalidInput(format!(
                    "phone.{what} must be in international format, e.g. +15551234567"
                )));
            }
        }
        if ph.enabled
            && (ph.account_sid.is_empty() || ph.from_number.is_empty() || ph.to_number.is_empty())
        {
            return bad("to enable the phone, fill in the account SID and both numbers");
        }
        if !(1..=60).contains(&ph.max_per_hour) {
            return bad("phone.max_per_hour must be between 1 and 60");
        }
        if !(0.0..=1.0).contains(&self.memory.recall_min_score) {
            return bad("memory.recall_min_score must be between 0 and 1");
        }
        for (label, url) in [
            ("observability.loki_url", &self.observability.loki_url),
            ("voice.stt_url", &self.voice.stt_url),
        ] {
            if !url.is_empty() {
                match reqwest::Url::parse(url) {
                    Ok(u) if matches!(u.scheme(), "http" | "https") => {}
                    _ => return bad(&format!("{label} must be an http(s) URL")),
                }
            }
        }
        if self.voice.piper_path.trim().is_empty() || self.voice.piper_path.contains(['\n', '\0']) {
            return bad("voice.piper_path must be a program name or path");
        }
        if self.voice.tts_voice.contains(['\n', '\0']) || self.voice.tts_voice.starts_with('-') {
            return bad("voice.tts_voice is invalid");
        }
        if self.mcp.servers.len() > 20 {
            return bad("at most 20 MCP servers");
        }
        let mut names = std::collections::HashSet::new();
        for srv in &self.mcp.servers {
            let valid = !srv.name.is_empty()
                && srv.name.len() <= 32
                && srv
                    .name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
            if !valid {
                return bad("MCP server names must match [a-z0-9_-]{1,32}");
            }
            if !names.insert(srv.name.clone()) {
                return bad("MCP server names must be unique");
            }
            match &srv.transport {
                McpTransport::Stdio {
                    command,
                    secret_env,
                    env,
                    ..
                } => {
                    if command.trim().is_empty() {
                        return bad("MCP stdio servers need a command");
                    }
                    let env_ok = |k: &String| {
                        !k.is_empty()
                            && k.len() <= 64
                            && k.chars()
                                .next()
                                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                            && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    };
                    if !secret_env.iter().all(env_ok) || !env.keys().all(env_ok) {
                        return bad(
                            "MCP environment variable names must match [A-Za-z_][A-Za-z0-9_]*",
                        );
                    }
                }
                McpTransport::Http { url, .. } => match reqwest::Url::parse(url) {
                    Ok(u) if matches!(u.scheme(), "http" | "https") => {}
                    _ => return bad("MCP HTTP servers need an http(s) URL"),
                },
            }
        }
        if self.ai.cloud_model.len() > 200 {
            return bad("ai.cloud_model is too long");
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
        for srv in &new.mcp.servers {
            let old = self.mcp.servers.iter().find(|o| o.name == srv.name);
            let changed = old.is_none_or(|o| o.transport != srv.transport);
            let newly_enabled = srv.enabled && old.is_none_or(|o| !o.enabled);
            let ro_added: Vec<&String> = srv
                .read_only_tools
                .iter()
                .filter(|t| old.is_none_or(|o| !o.read_only_tools.contains(t)))
                .collect();
            if srv.enabled && (changed || newly_enabled) {
                let what = match &srv.transport {
                    McpTransport::Stdio { command, args, .. } => {
                        format!("run `{} {}`", command, args.join(" "))
                    }
                    McpTransport::Http { url, .. } => format!("connect to {url}"),
                };
                out.push(format!("Enable MCP server `{}` ({what})", srv.name));
            }
            if !ro_added.is_empty() {
                out.push(format!(
                    "Let MCP server `{}` run tools without confirmation: {}",
                    srv.name,
                    ro_added
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        if !new.observability.loki_url.is_empty()
            && self.observability.loki_url != new.observability.loki_url
        {
            out.push(format!(
                "Ship audit log to Loki at {}",
                new.observability.loki_url
            ));
        }
        if !new.voice.stt_url.is_empty() && self.voice.stt_url != new.voice.stt_url {
            out.push(format!("Send recorded audio to {}", new.voice.stt_url));
        }
        if self.voice.piper_path != new.voice.piper_path {
            out.push(format!("Run `{}` for text-to-speech", new.voice.piper_path));
        }
        if !new.memory.backend_url.is_empty() && self.memory.backend_url != new.memory.backend_url {
            out.push(format!(
                "Send memories to kb-core at {}",
                new.memory.backend_url
            ));
        }
        if new.memory.auto_capture && !self.memory.auto_capture {
            out.push("Automatically save facts from your chat messages to long-term memory".into());
        }
        if new.memory.auto_summarize && !self.memory.auto_summarize {
            out.push(
                "Save an AI-written summary of each cleared conversation to long-term memory"
                    .into(),
            );
        }
        // Phone: message text leaves the machine (Twilio), even in local-only
        // mode, and a changed recipient would redirect every alert.
        if new.phone.enabled
            && (!self.phone.enabled
                || self.phone.to_number != new.phone.to_number
                || self.phone.from_number != new.phone.from_number
                || self.phone.account_sid != new.phone.account_sid)
        {
            out.push(format!(
                "Send texts and calls to {} through Twilio (message text leaves this computer, even in local-only mode)",
                new.phone.to_number
            ));
        }
        // Retention deletes data: turning it on or shortening it is confirmed.
        let (old_days, new_days) = (self.memory.retention_days, new.memory.retention_days);
        if new_days != 0 && (old_days == 0 || new_days < old_days) {
            out.push(format!(
                "Delete archived conversations older than {new_days} days"
            ));
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

        // Memory settings that store or delete data are confirmed too;
        // keeping transcripts longer is not.
        let mut new = old.clone();
        new.memory.auto_summarize = true;
        new.memory.retention_days = 7;
        assert_eq!(old.security_changes(&new).len(), 2);
        let mut old = old;
        old.memory.retention_days = 90;
        new = old.clone();
        new.memory.retention_days = 0;
        assert!(old.security_changes(&new).is_empty());
        new.memory.retention_days = 365;
        assert!(old.security_changes(&new).is_empty());
        new.memory.retention_days = 30;
        assert_eq!(old.security_changes(&new).len(), 1);
    }

    #[test]
    fn phone_settings_are_validated_and_confirmed() {
        assert!(is_e164("+13145550100"));
        assert!(!is_e164("3145550100"));
        assert!(!is_e164("+0123456789"));
        assert!(!is_e164("+1314555abcd"));
        assert!(is_twilio_sid(&format!("AC{}", "0a".repeat(16))));
        assert!(!is_twilio_sid("AC../../evil"));

        let old = Settings::default();
        let mut new = old.clone();
        new.phone.enabled = true;
        assert!(new.validate().is_err(), "enabled without numbers");
        new.phone.account_sid = format!("AC{}", "0a".repeat(16));
        new.phone.from_number = "+13145550100".into();
        new.phone.to_number = "+13145550101".into();
        assert!(new.validate().is_ok());
        assert_eq!(old.security_changes(&new).len(), 1);
        // Redirecting to another number while enabled is confirmed again.
        let mut moved = new.clone();
        moved.phone.to_number = "+13145550199".into();
        assert_eq!(new.security_changes(&moved).len(), 1);
        assert!(new.security_changes(&new.clone()).is_empty());
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
