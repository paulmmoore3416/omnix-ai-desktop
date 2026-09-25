//! Secret storage in the OS keychain.
//!
//! * macOS: Keychain Services. Windows: Credential Manager. Linux: Secret
//!   Service (GNOME Keyring / KWallet) over D-Bus.
//! * Secrets are addressed by a **provider id** from [`PROVIDERS`]. The
//!   frontend can set, delete and ask whether a secret exists, but there is
//!   deliberately **no command that returns a secret value** to the webview.
//!   Only Rust code (LLM providers, integrations) calls [`SecretStore::get`].
//! * [`migrate_plaintext`] moves keys that older versions wrote into
//!   `settings.json` into the keychain on startup.

use crate::error::{AppError, AppResult};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

/// Keychain service name (matches the Tauri bundle identifier).
pub const SERVICE: &str = "com.paulmmoore.omnix";

/// Maximum accepted secret length (bytes).
pub const MAX_SECRET_LEN: usize = 8192;

/// Provider ids that may have a stored secret. Anything else is rejected so a
/// compromised webview cannot use the keychain as arbitrary storage.
pub const PROVIDERS: &[&str] = &[
    "openai",
    "anthropic",
    "gemini",
    "xai",
    "github",
    "google_drive",
    "jira",
    "notion",
    "kb_core",
];

/// Abstraction over the keychain so tests can use an in-memory store.
pub trait SecretStore: Send + Sync {
    /// Store (or replace) the secret for `provider`.
    fn set(&self, provider: &str, value: &str) -> AppResult<()>;
    /// Fetch the secret for `provider`; `Ok(None)` when absent.
    fn get(&self, provider: &str) -> AppResult<Option<String>>;
    /// Delete the secret for `provider`; deleting a missing secret is `Ok`.
    fn delete(&self, provider: &str) -> AppResult<()>;
    /// Whether a secret exists for `provider`.
    fn has(&self, provider: &str) -> AppResult<bool> {
        Ok(self.get(provider)?.is_some())
    }
}

/// Validate a provider id against [`PROVIDERS`] or the MCP secret pattern
/// `mcp.<server>.<NAME>` (server `[a-z0-9_-]{1,32}`, name an env-var name or
/// `token`).
pub fn validate_provider(provider: &str) -> AppResult<()> {
    if PROVIDERS.contains(&provider) || is_mcp_secret_id(provider) {
        Ok(())
    } else {
        Err(AppError::InvalidInput(format!(
            "unknown secret provider `{provider}`"
        )))
    }
}

fn is_mcp_secret_id(id: &str) -> bool {
    let mut parts = id.splitn(3, '.');
    let (Some("mcp"), Some(server), Some(name)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let server_ok = !server.is_empty()
        && server.len() <= 32
        && server
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    let name_ok = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    server_ok && name_ok
}

/// Validate a secret value (non-empty, bounded, single line).
pub fn validate_value(value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::InvalidInput("secret must not be empty".into()));
    }
    if value.len() > MAX_SECRET_LEN {
        return Err(AppError::InvalidInput("secret is too long".into()));
    }
    if value.contains(['\n', '\r', '\0']) {
        return Err(AppError::InvalidInput(
            "secret must be a single line".into(),
        ));
    }
    Ok(())
}

/// OS keychain implementation backed by the `keyring` crate.
#[derive(Debug, Default)]
pub struct KeyringStore;

impl KeyringStore {
    fn entry(provider: &str) -> AppResult<keyring::Entry> {
        validate_provider(provider)?;
        keyring::Entry::new(SERVICE, provider).map_err(map_keyring_err)
    }
}

/// Map keyring errors to [`AppError`] without echoing any secret material.
fn map_keyring_err(e: keyring::Error) -> AppError {
    match e {
        keyring::Error::NoDefaultStore | keyring::Error::NoStorageAccess(_) => {
            AppError::Unavailable(format!(
                "OS keychain is not available ({e}). On Linux, make sure a Secret Service \
                 provider such as GNOME Keyring or KWallet is running and unlocked."
            ))
        }
        other => AppError::Secret(other.to_string()),
    }
}

impl SecretStore for KeyringStore {
    fn set(&self, provider: &str, value: &str) -> AppResult<()> {
        validate_value(value)?;
        Self::entry(provider)?
            .set_password(value.trim())
            .map_err(map_keyring_err)
    }

    fn get(&self, provider: &str) -> AppResult<Option<String>> {
        match Self::entry(provider)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(map_keyring_err(e)),
        }
    }

    fn delete(&self, provider: &str) -> AppResult<()> {
        match Self::entry(provider)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(map_keyring_err(e)),
        }
    }
}

/// In-memory store for tests and for environments without a keychain.
#[derive(Debug, Default)]
pub struct MemoryStore {
    inner: Mutex<HashMap<String, String>>,
}

impl SecretStore for MemoryStore {
    fn set(&self, provider: &str, value: &str) -> AppResult<()> {
        validate_provider(provider)?;
        validate_value(value)?;
        self.inner
            .lock()
            .map_err(|_| AppError::Internal("secret store lock poisoned".into()))?
            .insert(provider.to_string(), value.trim().to_string());
        Ok(())
    }

    fn get(&self, provider: &str) -> AppResult<Option<String>> {
        validate_provider(provider)?;
        Ok(self
            .inner
            .lock()
            .map_err(|_| AppError::Internal("secret store lock poisoned".into()))?
            .get(provider)
            .cloned())
    }

    fn delete(&self, provider: &str) -> AppResult<()> {
        validate_provider(provider)?;
        self.inner
            .lock()
            .map_err(|_| AppError::Internal("secret store lock poisoned".into()))?
            .remove(provider);
        Ok(())
    }
}

/// Locations of plaintext secrets written by older OMNIX versions:
/// `(json pointer to the parent object, field names (snake and camel), provider)`.
const LEGACY_FIELDS: &[(&str, &[&str], &str)] = &[
    ("/ai", &["openai_key", "openaiKey"], "openai"),
    ("/ai", &["anthropic_key", "anthropicKey"], "anthropic"),
    ("/ai", &["gemini_key", "geminiKey"], "gemini"),
    ("/ai", &["xai_key", "xaiKey"], "xai"),
    ("/integrations/github", &["token"], "github"),
    (
        "/integrations/google_drive",
        &["api_key", "apiKey"],
        "google_drive",
    ),
    (
        "/integrations/googleDrive",
        &["api_key", "apiKey"],
        "google_drive",
    ),
    ("/integrations/jira", &["api_token", "apiToken"], "jira"),
    ("/integrations/notion", &["api_key", "apiKey"], "notion"),
];

/// Outcome of [`migrate_plaintext`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Migration {
    /// Providers whose plaintext key was moved into the store.
    pub migrated: Vec<String>,
    /// True when `raw` was modified and must be written back.
    pub changed: bool,
}

/// Move plaintext secrets found in a raw settings document into `store` and
/// remove them from `raw`.
///
/// A field is removed from `raw` **only after** the store accepted it, so a
/// keychain failure never loses a key. Empty legacy fields are simply removed.
/// Never logs secret values.
pub fn migrate_plaintext(raw: &mut Value, store: &dyn SecretStore) -> AppResult<Migration> {
    let mut out = Migration::default();
    for (pointer, fields, provider) in LEGACY_FIELDS {
        let Some(obj) = raw.pointer_mut(pointer).and_then(Value::as_object_mut) else {
            continue;
        };
        for field in *fields {
            let Some(val) = obj.get(*field) else {
                continue;
            };
            let secret = val.as_str().unwrap_or_default().trim().to_string();
            if !secret.is_empty() {
                store.set(provider, &secret)?;
                out.migrated.push((*provider).to_string());
            }
            obj.remove(*field);
            out.changed = true;
        }
    }
    out.migrated.dedup();
    Ok(out)
}

/// Remove every known secret field from `raw` without storing it. Used when
/// importing a settings file: imports must never carry secrets.
pub fn strip_plaintext(raw: &mut Value) -> bool {
    let mut changed = false;
    for (pointer, fields, _) in LEGACY_FIELDS {
        if let Some(obj) = raw.pointer_mut(pointer).and_then(Value::as_object_mut) {
            for field in *fields {
                changed |= obj.remove(*field).is_some();
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_unknown_providers_and_bad_values() {
        let s = MemoryStore::default();
        assert!(s.set("evil", "x").is_err());
        assert!(s.set("mcp.github.GITHUB_TOKEN", "x").is_ok());
        assert!(s.set("mcp.github.token", "x").is_ok());
        assert!(s.set("mcp.../x", "x").is_err());
        assert!(s.set("mcp.GitHub.X", "x").is_err());
        assert!(s.set("mcp.a.b.c", "x").is_err());
        assert!(s.set("openai", "").is_err());
        assert!(s.set("openai", "a\nb").is_err());
        assert!(s.set("openai", &"x".repeat(MAX_SECRET_LEN + 1)).is_err());
        assert!(s.set("openai", "sk-good").is_ok());
        assert!(s.has("openai").expect("has"));
        s.delete("openai").expect("delete");
        assert!(!s.has("openai").expect("has"));
        s.delete("openai").expect("deleting a missing secret is ok");
    }

    #[test]
    fn migrates_snake_and_camel_keys_and_removes_them() {
        let s = MemoryStore::default();
        let mut raw = json!({
            "ai": { "provider": "ollama", "openai_key": "sk-one", "anthropicKey": "sk-ant-two", "gemini_key": "" },
            "integrations": { "github": { "token": "ghp_three", "username": "u" }, "googleDrive": { "apiKey": "AIzaFour" } }
        });
        let m = migrate_plaintext(&mut raw, &s).expect("migrate");
        assert!(m.changed);
        assert_eq!(s.get("openai").expect("get").as_deref(), Some("sk-one"));
        assert_eq!(
            s.get("anthropic").expect("get").as_deref(),
            Some("sk-ant-two")
        );
        assert_eq!(s.get("github").expect("get").as_deref(), Some("ghp_three"));
        assert_eq!(
            s.get("google_drive").expect("get").as_deref(),
            Some("AIzaFour")
        );
        assert!(!s.has("gemini").expect("has"));
        let text = raw.to_string();
        for secret in ["sk-one", "sk-ant-two", "ghp_three", "AIzaFour"] {
            assert!(!text.contains(secret), "{secret} left in settings");
        }
        assert_eq!(raw["integrations"]["github"]["username"], "u");
        assert!(raw["ai"].get("gemini_key").is_none());
    }

    #[test]
    fn migration_is_idempotent() {
        let s = MemoryStore::default();
        let mut raw = json!({ "ai": { "provider": "ollama" } });
        let m = migrate_plaintext(&mut raw, &s).expect("migrate");
        assert_eq!(m, Migration::default());
    }

    /// A store that always fails, to prove keys are not dropped on error.
    struct Broken;
    impl SecretStore for Broken {
        fn set(&self, _: &str, _: &str) -> AppResult<()> {
            Err(AppError::Unavailable("no keychain".into()))
        }
        fn get(&self, _: &str) -> AppResult<Option<String>> {
            Ok(None)
        }
        fn delete(&self, _: &str) -> AppResult<()> {
            Ok(())
        }
    }

    #[test]
    fn failed_migration_keeps_the_key() {
        let mut raw = json!({ "ai": { "openai_key": "sk-keep" } });
        assert!(migrate_plaintext(&mut raw, &Broken).is_err());
        assert_eq!(raw["ai"]["openai_key"], "sk-keep");
    }

    #[test]
    fn strip_removes_secrets() {
        let mut raw = json!({ "ai": { "openai_key": "sk-x", "provider": "ollama" } });
        assert!(strip_plaintext(&mut raw));
        assert!(raw["ai"].get("openai_key").is_none());
        assert_eq!(raw["ai"]["provider"], "ollama");
    }
}
