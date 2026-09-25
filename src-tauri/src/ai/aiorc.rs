//! AIORC routing backend (scaffold, behind the `aiorc` cargo feature).
//!
//! AIORC is reached over gRPC (`tonic`). Its `.proto` schema is **not** in
//! this repository, and inventing one would produce a client that cannot
//! interoperate, so this provider deliberately returns
//! [`AppError::NotImplemented`] for every call. To finish it:
//!
//! 1. add the AIORC `.proto` under `src-tauri/proto/`;
//! 2. add `tonic`/`prost` (+ `tonic-build` in `build.rs`) under the `aiorc`
//!    feature and generate the client;
//! 3. map `ChatMessage`/`ToolSpec`/`ChatEvent` onto the generated types here
//!    and register `"aiorc"` in `ai::build_provider` and settings validation
//!    (with `local_only` enforcement on the endpoint).

use crate::ai::provider::{ChatMessage, ChatOptions, ChatStream, LlmProvider, ToolSpec};
use crate::error::{AppError, AppResult};
use async_trait::async_trait;

/// Placeholder AIORC provider.
#[derive(Debug, Default)]
pub struct AiorcProvider {
    /// gRPC endpoint (e.g. `http://aiorc.tailnet:50051`).
    pub endpoint: String,
}

#[async_trait]
impl LlmProvider for AiorcProvider {
    fn id(&self) -> &'static str {
        "aiorc"
    }

    async fn list_models(&self) -> AppResult<Vec<String>> {
        Err(AppError::NotImplemented("AIORC provider (awaiting .proto)"))
    }

    async fn health_check(&self) -> AppResult<String> {
        Err(AppError::NotImplemented("AIORC provider (awaiting .proto)"))
    }

    async fn chat_stream(
        &self,
        _messages: &[ChatMessage],
        _tools: &[ToolSpec],
        _opts: &ChatOptions,
    ) -> AppResult<ChatStream> {
        Err(AppError::NotImplemented("AIORC provider (awaiting .proto)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scaffold_is_honest() {
        let p = AiorcProvider::default();
        assert!(matches!(
            p.health_check().await,
            Err(AppError::NotImplemented(_))
        ));
        assert!(matches!(
            p.list_models().await,
            Err(AppError::NotImplemented(_))
        ));
    }
}
