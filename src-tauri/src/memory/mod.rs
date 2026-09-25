//! Long-term memory.
//!
//! [`MemoryStore`] abstracts the backend; [`kb_core::KbCoreStore`] talks to an
//! external kb-core service (FastAPI over PostgreSQL/pgvector) using the
//! minimal REST contract in `docs/kb-core-contract.md`. When
//! `memory.backend_url` is empty, memory is disabled and the UI says so.

pub mod kb_core;

use crate::error::AppResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A stored memory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryRecord {
    /// Backend id.
    pub id: String,
    /// Memory text.
    pub content: String,
    /// Free-form tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// 1–10.
    #[serde(default)]
    pub importance: u8,
    /// Category label.
    #[serde(default)]
    pub category: String,
    /// RFC 3339 creation time.
    #[serde(default)]
    pub created_at: Option<String>,
}

/// Input for [`MemoryStore::save`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMemory {
    /// Memory text.
    pub content: String,
    /// Free-form tags.
    pub tags: Vec<String>,
    /// 1–10.
    pub importance: u8,
    /// Category label.
    pub category: String,
}

/// A semantic-search result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    /// Backend id.
    pub id: String,
    /// Matched text.
    pub content: String,
    /// Similarity score (higher is better).
    #[serde(default)]
    pub score: f32,
    /// Tags of the matched memory.
    #[serde(default)]
    pub tags: Vec<String>,
    /// RFC 3339 creation time.
    #[serde(default)]
    pub created_at: Option<String>,
}

/// Result of [`MemoryStore::index_document`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentReceipt {
    /// Backend document id.
    pub id: String,
    /// Number of chunks embedded.
    #[serde(default)]
    pub chunks: u32,
}

/// Memory backend.
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Human summary if the backend is reachable.
    async fn health(&self) -> AppResult<String>;
    /// Most recent memories.
    async fn list(&self, limit: u32) -> AppResult<Vec<MemoryRecord>>;
    /// Store a memory; returns its id.
    async fn save(&self, memory: NewMemory) -> AppResult<String>;
    /// Delete a memory by id.
    async fn delete(&self, id: &str) -> AppResult<()>;
    /// Semantic search.
    async fn search(&self, query: &str, limit: u32) -> AppResult<Vec<SearchHit>>;
    /// Chunk + embed a text document.
    async fn index_document(&self, name: &str, content: &str) -> AppResult<DocumentReceipt>;
}

/// Build the configured memory store, or `None` when `memory.backend_url` is
/// empty. Enforces `local_only` on the backend URL and loads the optional
/// bearer token from the keychain.
pub async fn from_state(state: &crate::state::AppState) -> AppResult<Option<Box<dyn MemoryStore>>> {
    let settings = state.settings.read().await.clone();
    let url = settings.memory.backend_url.trim().to_string();
    if url.is_empty() {
        return Ok(None);
    }
    crate::ai::endpoint::ensure_endpoint_allowed(&url, settings.security.local_only).await?;
    let store = state.secrets.clone();
    let token = match tokio::task::spawn_blocking(move || store.get("kb_core")).await? {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "could not read kb-core token; continuing without it");
            None
        }
    };
    Ok(Some(Box::new(kb_core::KbCoreStore::new(
        state.http.clone(),
        &url,
        token,
    ))))
}

/// Like [`from_state`] but errors when memory is not configured.
pub async fn require(state: &crate::state::AppState) -> AppResult<Box<dyn MemoryStore>> {
    from_state(state).await?.ok_or_else(|| {
        crate::error::AppError::Unavailable(
            "long-term memory is not configured: set a kb-core URL in Settings → Memory".into(),
        )
    })
}
