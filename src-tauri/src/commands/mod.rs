//! `#[tauri::command]` entry points.
//!
//! Commands are thin: they validate/unpack IPC arguments and delegate to the
//! domain modules (`security`, `system`, `ai`, `settings`). Anything that is
//! not built yet returns [`crate::error::AppError::NotImplemented`] so the UI can disable the
//! control; no command reports success for work it did not do.
//!
//! Stub commands intentionally take **no parameters**: Tauri ignores extra
//! keys in the invoke payload, so the frontend always receives the structured
//! `not_implemented` error instead of an argument-deserialization failure.

pub mod chat;
pub mod exec;
pub mod files;
pub mod google;
pub mod knowledge;
pub mod license;
pub mod models;
pub mod ops;
pub mod phone;
pub mod settings;
pub mod system;
pub mod tasks;
pub mod usage;
pub mod voice;
