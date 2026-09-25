//! LLM integration.
//!
//! * [`endpoint`]: `local_only` enforcement (no PHI egress to non-local hosts).
//! * [`ollama`]: Ollama HTTP client.

pub mod endpoint;
pub mod ollama;
