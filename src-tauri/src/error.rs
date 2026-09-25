//! Application-wide error type.
//!
//! Every `#[tauri::command]` returns [`AppResult`]. Errors are serialized to the
//! frontend as `{ "kind": "<snake_case>", "message": "<human text>" }` so the UI
//! can branch on `kind` (for example to disable a control on `not_implemented`)
//! instead of pattern-matching on English strings.

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// Convenience alias used throughout the crate.
pub type AppResult<T> = Result<T, AppError>;

/// All failure modes surfaced to the frontend.
///
/// Variants deliberately carry *already redacted* human-readable context. Never
/// put secrets (API keys, tokens) into an error message: errors are shown in the
/// UI and may be written to logs.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// The command exists so the UI can be wired, but the feature is not built yet.
    /// Commands must return this rather than pretending to succeed.
    #[error("{0} is not implemented yet")]
    NotImplemented(&'static str),

    /// The command-execution policy engine refused the request.
    #[error("blocked by security policy: {0}")]
    PolicyDenied(String),

    /// The user declined (or did not answer) a native confirmation dialog.
    #[error("not approved: {0}")]
    NotApproved(String),

    /// The caller supplied malformed or out-of-range input.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// `local_only` mode blocked a request to a non-local endpoint or cloud provider.
    #[error("blocked by local-only mode: {0}")]
    LocalOnly(String),

    /// An external dependency (Ollama, keychain, elevation tool, ...) is not reachable.
    #[error("unavailable: {0}")]
    Unavailable(String),

    /// A spawned process failed to start, timed out, or could not be awaited.
    #[error("execution failed: {0}")]
    Execution(String),

    /// OS keychain failure.
    #[error("secret store error: {0}")]
    Secret(String),

    /// Filesystem error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP client error (connection refused, timeout, bad status, ...).
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON (de)serialization error.
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// Anything else that indicates a bug or broken invariant.
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    /// Stable machine-readable discriminant sent to the frontend.
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::NotImplemented(_) => "not_implemented",
            AppError::PolicyDenied(_) => "policy_denied",
            AppError::NotApproved(_) => "not_approved",
            AppError::InvalidInput(_) => "invalid_input",
            AppError::LocalOnly(_) => "local_only",
            AppError::Unavailable(_) => "unavailable",
            AppError::Execution(_) => "execution",
            AppError::Secret(_) => "secret",
            AppError::Io(_) => "io",
            AppError::Http(_) => "http",
            AppError::Json(_) => "json",
            AppError::Internal(_) => "internal",
        }
    }
}

impl From<tokio::task::JoinError> for AppError {
    fn from(e: tokio::task::JoinError) -> Self {
        AppError::Internal(format!("background task failed: {e}"))
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_kind_and_message() {
        let e = AppError::NotImplemented("create_automation");
        let v = serde_json::to_value(&e).expect("serializable");
        assert_eq!(v["kind"], "not_implemented");
        assert_eq!(v["message"], "create_automation is not implemented yet");
    }
}
