//! LLM integration.
//!
//! * [`provider`]: provider-neutral types and the [`provider::LlmProvider`] trait.
//! * [`ollama`], [`anthropic`], [`openai_compat`]: backends.
//! * [`stream`]: NDJSON / SSE parsing.
//! * [`endpoint`]: `local_only` enforcement (no PHI egress to non-local hosts).
//! * [`context`]: context-window-aware history truncation.
//! * [`agent`]: the tool-using agent loop.

pub mod agent;
#[cfg(feature = "aiorc")]
pub mod aiorc;
pub mod anthropic;
pub mod context;
pub mod endpoint;
pub mod ollama;
pub mod openai_compat;
pub mod provider;
pub mod stream;

use crate::error::{AppError, AppResult};
use crate::settings::AiSettings;
use crate::state::AppState;
use provider::LlmProvider;

/// A ready-to-use provider plus the model id to request.
pub struct Selected {
    /// The backend.
    pub provider: Box<dyn LlmProvider>,
    /// Model id (from settings; discovered at runtime, never hardcoded).
    pub model: String,
}

/// Build the provider described by `ai`, enforcing `local_only` and loading
/// any API key from the OS keychain (never from settings or the webview).
///
/// `require_model` is false for discovery calls (listing models).
pub async fn build_provider(
    state: &AppState,
    ai: &AiSettings,
    local_only: bool,
    require_model: bool,
) -> AppResult<Selected> {
    endpoint::ensure_provider_allowed(&ai.provider, local_only)?;
    let (provider, model): (Box<dyn LlmProvider>, String) = match ai.provider.as_str() {
        "ollama" => {
            endpoint::ensure_endpoint_allowed(&ai.ollama_host, local_only).await?;
            (
                Box::new(ollama::OllamaProvider::new(
                    state.http.clone(),
                    &ai.ollama_host,
                )),
                ai.ollama_model.clone(),
            )
        }
        "anthropic" => (
            Box::new(anthropic::AnthropicProvider::new(
                state.http.clone(),
                api_key(state, "anthropic").await?,
            )),
            ai.cloud_model.clone(),
        ),
        id @ ("openai" | "xai" | "gemini") => {
            let id: &'static str = match id {
                "openai" => "openai",
                "xai" => "xai",
                _ => "gemini",
            };
            (
                Box::new(openai_compat::OpenAiCompatProvider::new(
                    id,
                    state.http.clone(),
                    api_key(state, id).await?,
                )?),
                ai.cloud_model.clone(),
            )
        }
        other => {
            return Err(AppError::InvalidInput(format!(
                "unsupported provider `{other}`"
            )))
        }
    };
    if require_model && model.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "no model selected: choose one in Settings → AI Models".into(),
        ));
    }
    Ok(Selected { provider, model })
}

/// Fetch a provider key from the keychain (blocking call moved off the runtime).
async fn api_key(state: &AppState, provider: &str) -> AppResult<String> {
    let store = state.secrets.clone();
    let p = provider.to_string();
    tokio::task::spawn_blocking(move || store.get(&p))
        .await??
        .ok_or_else(|| {
            AppError::InvalidInput(format!(
                "no API key saved for `{provider}` (Settings → AI Models)"
            ))
        })
}
