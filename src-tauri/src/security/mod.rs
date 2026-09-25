//! Security subsystem: every privileged capability the webview can reach is
//! funnelled through this module.
//!
//! * [`policy`]: pure command classifier (risk tiers, deny rules).
//! * [`confirm`]: native (Rust-side) confirmation dialogs.
//! * [`executor`]: the only code path that spawns processes.
//! * [`elevation`]: pkexec / osascript / UAC adapters.
//! * [`files`]: guarded file read/list/write.
//! * [`audit`]: hash-chained, redacted JSONL audit log.
//! * [`secrets`]: OS keychain storage (no read-back to the webview).

pub mod audit;
pub mod confirm;
pub mod elevation;
pub mod executor;
pub mod files;
pub mod policy;
pub mod secrets;
