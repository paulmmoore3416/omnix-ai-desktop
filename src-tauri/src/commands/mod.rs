//! `#[tauri::command]` entry points.
//!
//! Commands are thin: they validate/unpack IPC arguments and delegate to the
//! domain modules (`security`, `system`, `ai`, `settings`). Anything that is
//! not built yet returns [`AppError::NotImplemented`] so the UI can disable the
//! control; no command reports success for work it did not do.
//!
//! Stub commands intentionally take **no parameters**: Tauri ignores extra
//! keys in the invoke payload, so the frontend always receives the structured
//! `not_implemented` error instead of an argument-deserialization failure.

pub mod chat;
pub mod exec;
pub mod files;
pub mod knowledge;
pub mod models;
pub mod ops;
pub mod settings;
pub mod system;
pub mod voice;

use crate::error::{AppError, AppResult};

/// Shorthand for a command that is wired but not implemented.
pub(crate) fn not_implemented<T>(what: &'static str) -> AppResult<T> {
    Err(AppError::NotImplemented(what))
}
