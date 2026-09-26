//! Long-term memory.
//!
//! [`MemoryStore`] abstracts the backend; [`kb_core::KbCoreStore`] talks to a
//! kb-core service over the REST contract in `docs/kb-core-contract.md`. The
//! reference service ships in `kb-core/` (installed by
//! `scripts/setup-memory.sh`). The six core endpoints are required; the
//! extension endpoints (stats, extraction, export/import, maintenance, …)
//! are optional and map to [`AppError::NotImplemented`] when a service does
//! not offer them. When `memory.backend_url` is empty, memory is disabled
//! and the UI says so.
//!
//! Everything read back from the store is **untrusted text**: the agent wraps
//! it in `<tool_result untrusted="true">` before a model sees it, including
//! the automatic recall block.

pub mod kb_core;

use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    /// Who created it: `user`, `assistant`, `extract`, `import` (kb-core extension).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Pinned memories never fade (kb-core extension).
    #[serde(default)]
    pub pinned: bool,
    /// Times the same fact was stated again (kb-core extension).
    #[serde(default)]
    pub reinforced: u32,
    /// Times recalled by search (kb-core extension).
    #[serde(default)]
    pub access_count: u32,
    /// Times the user flagged it as wrong when recalled (kb-core extension).
    #[serde(default)]
    pub rejected: u32,
    /// 0–1 liveliness from importance, recency and use (kb-core extension).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation: Option<f32>,
    /// Knowledge base (kb-core collection).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
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
    /// Who created it (`user`, `assistant`, …). kb-core extension; other
    /// services ignore it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Knowledge base to file it in (kb-core extension; default collection when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
}

/// A semantic-search result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    /// Backend id.
    pub id: String,
    /// Matched text.
    pub content: String,
    /// Relevance score (higher is better; kb-core: calibrated 0–1).
    #[serde(default)]
    pub score: f32,
    /// Tags of the matched memory.
    #[serde(default)]
    pub tags: Vec<String>,
    /// RFC 3339 creation time.
    #[serde(default)]
    pub created_at: Option<String>,
    /// Raw cosine similarity (kb-core extension).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub similarity: Option<f32>,
    /// `memory` or `document` (kb-core extension).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// `memory` or the document name (kb-core extension).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Knowledge base (kb-core collection).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
    /// Provenance (kb-core extension): `user`, `extract`, `assistant`,
    /// `import` for memories, `document` for note chunks. Drives the trust
    /// label in the recall block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
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

/// Memory backend. The first six methods are the core contract; the rest
/// are optional extensions whose default implementations report
/// [`AppError::NotImplemented`].
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

    /// Index a document into a named collection (kb-core extension; other
    /// services ignore the collection).
    async fn index_document_in(
        &self,
        name: &str,
        content: &str,
        _collection: &str,
    ) -> AppResult<DocumentReceipt> {
        self.index_document(name, content).await
    }

    /// Search within one knowledge base (kb-core extension; others search everything).
    async fn search_in(
        &self,
        query: &str,
        limit: u32,
        _collection: &str,
    ) -> AppResult<Vec<SearchHit>> {
        self.search(query, limit).await
    }

    /// Number of active memories, if the service reports it (kb-core:
    /// `/health`). `None` means unknown, and `max_memory_size` can't be
    /// enforced against that service.
    async fn memory_count(&self) -> AppResult<Option<u64>> {
        Ok(None)
    }

    /// Search only one kind of result (`memory` or `document`). kb-core
    /// filters server-side (`kinds` extension); other services are searched
    /// normally and filtered here (a hit without a kind is a memory).
    async fn search_kind(&self, query: &str, limit: u32, kind: &str) -> AppResult<Vec<SearchHit>> {
        let mut hits = self.search(query, limit).await?;
        hits.retain(|h| h.kind.as_deref().unwrap_or("memory") == kind);
        Ok(hits)
    }

    /// Search, keeping only hits scoring at least `min_score`.
    async fn recall(&self, query: &str, limit: u32, min_score: f32) -> AppResult<Vec<SearchHit>> {
        let mut hits = self.search(query, limit).await?;
        hits.retain(|h| h.score >= min_score);
        Ok(hits)
    }
    /// Store a memory and return the backend's full receipt (kb-core:
    /// `{id, status: created|reinforced|updated, related: [...]}`).
    async fn save_detailed(&self, memory: NewMemory) -> AppResult<Value> {
        let id = self.save(memory).await?;
        Ok(serde_json::json!({ "id": id, "status": "created" }))
    }
    /// Analytics (counts, categories, storage, activity).
    async fn stats(&self) -> AppResult<Value> {
        Err(AppError::NotImplemented("memory statistics"))
    }
    /// Indexed documents.
    async fn list_documents(&self) -> AppResult<Vec<Value>> {
        Err(AppError::NotImplemented("document listing"))
    }
    /// Remove an indexed document.
    async fn delete_document(&self, _id: &str) -> AppResult<()> {
        Err(AppError::NotImplemented("document deletion"))
    }
    /// Merged and superseded memories (hidden from recall, restorable),
    /// newest first.
    async fn list_hidden(&self, _limit: u32) -> AppResult<Vec<Value>> {
        Err(AppError::NotImplemented("memory history"))
    }
    /// Edit, pin or restore a memory.
    async fn update_memory(&self, _id: &str, _patch: Value) -> AppResult<Value> {
        Err(AppError::NotImplemented("memory editing"))
    }
    /// Recall feedback: `helpful = false` when a recalled memory was wrong or
    /// beside the point (ranks it lower from then on), `true` when it helped.
    async fn feedback(&self, _id: &str, _helpful: bool) -> AppResult<Value> {
        Err(AppError::NotImplemented("recall feedback"))
    }
    /// Extract durable facts from the user's text and store them.
    async fn extract(&self, _text: &str) -> AppResult<Vec<Value>> {
        Err(AppError::NotImplemented("fact capture"))
    }
    /// NDJSON backup of memories and documents.
    async fn export(&self) -> AppResult<String> {
        Err(AppError::NotImplemented("export"))
    }
    /// Restore / merge records from an export.
    async fn import(&self, _records: Vec<Value>) -> AppResult<Value> {
        Err(AppError::NotImplemented("import"))
    }
    /// Consolidate, backfill and compact.
    async fn maintenance(&self) -> AppResult<Value> {
        Err(AppError::NotImplemented("optimization"))
    }
    /// Re-scan watched folders now.
    async fn sync_folders(&self) -> AppResult<Value> {
        Err(AppError::NotImplemented("folder sync"))
    }
    /// Collections (knowledge bases) with counts.
    async fn list_collections(&self) -> AppResult<Vec<Value>> {
        Err(AppError::NotImplemented("knowledge bases"))
    }
    /// Create (or re-describe) a collection.
    async fn create_collection(&self, _name: &str, _description: &str) -> AppResult<Value> {
        Err(AppError::NotImplemented("knowledge bases"))
    }
    /// Delete a collection and everything in it.
    async fn delete_collection(&self, _name: &str) -> AppResult<Value> {
        Err(AppError::NotImplemented("knowledge bases"))
    }
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
        settings.memory.enable_semantic_search,
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

/// Refuse a new memory when `memory.max_memory_size` (0 = unlimited) is
/// reached. Deleting old memories never happens behind the user's back: they
/// choose what to remove in the Knowledge view or raise the limit.
pub async fn check_capacity(
    state: &crate::state::AppState,
    store: &dyn MemoryStore,
) -> AppResult<()> {
    let max = state.settings.read().await.memory.max_memory_size;
    let count = store.memory_count().await?;
    capacity_error(count, max).map_or(Ok(()), Err)
}

fn capacity_error(count: Option<u64>, max: u32) -> Option<AppError> {
    match count {
        Some(n) if max > 0 && n >= u64::from(max) => Some(AppError::InvalidInput(format!(
            "long-term memory is full ({n} of {max} memories). Delete memories in the Knowledge \
             view or raise the limit in Settings → Memory"
        ))),
        _ => None,
    }
}

/// Archived conversation transcripts (`conversations` collection) whose last
/// update is older than `memory.retention_days` (0 = keep forever).
/// Memories and the user's own documents are never pruned by age.
pub fn expired_conversations(
    documents: &[Value],
    retention_days: u32,
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<(String, String)> {
    if retention_days == 0 {
        return vec![];
    }
    let cutoff = now - chrono::Duration::days(i64::from(retention_days));
    documents
        .iter()
        .filter(|d| d.get("collection").and_then(Value::as_str) == Some("conversations"))
        .filter_map(|d| {
            let when = d
                .get("updated_at")
                .or_else(|| d.get("created_at"))
                .and_then(Value::as_str)
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())?;
            if when >= cutoff {
                return None;
            }
            let id = d.get("id").and_then(Value::as_str)?.to_string();
            let name = d.get("name").and_then(Value::as_str).unwrap_or("");
            Some((id, name.to_string()))
        })
        .collect()
}

/// Apply `memory.retention_days` to the conversation archive. Deletions
/// are audited. Returns how many transcripts were removed.
pub async fn prune_conversations(state: &crate::state::AppState) -> AppResult<usize> {
    use crate::security::audit::{AuditRecord, Confirmation, Decision};
    use crate::security::policy::{RiskTier, Source};

    let days = state.settings.read().await.memory.retention_days;
    if days == 0 {
        return Ok(0);
    }
    let Some(store) = from_state(state).await? else {
        return Ok(0);
    };
    let expired = expired_conversations(&store.list_documents().await?, days, chrono::Utc::now());
    let mut removed = 0;
    let mut failed = Vec::new();
    for (id, name) in &expired {
        match store.delete_document(id).await {
            Ok(()) => removed += 1,
            Err(e) => failed.push(format!("{name}: {e}")),
        }
    }
    if !expired.is_empty() {
        state
            .audit
            .record(AuditRecord {
                id: uuid::Uuid::new_v4().to_string(),
                // Carries out the retention period the user set.
                source: Source::User,
                action: "memory_retention".into(),
                command: format!(
                    "delete {} archived conversations older than {days} days",
                    expired.len()
                ),
                cwd: None,
                tier: RiskTier::Mutating,
                decision: if failed.is_empty() {
                    Decision::Allowed
                } else {
                    Decision::Failed
                },
                confirmation: Confirmation::NotRequired,
                exit_code: None,
                duration_ms: None,
                detail: Some(format!("{removed} removed; {} failed", failed.len())),
            })
            .await?;
    }
    for f in failed {
        tracing::warn!(error = %f, "could not prune archived conversation");
    }
    Ok(removed)
}

#[cfg(test)]
mod retention_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_old_conversations_expire() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-09-26T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let docs = vec![
            json!({"id": "d1", "name": "old chat", "collection": "conversations", "updated_at": "2026-06-01T00:00:00Z"}),
            json!({"id": "d2", "name": "new chat", "collection": "conversations", "updated_at": "2026-09-20T00:00:00Z"}),
            json!({"id": "d3", "name": "old notes", "collection": "default", "updated_at": "2020-01-01T00:00:00Z"}),
            json!({"id": "d4", "name": "no date", "collection": "conversations"}),
        ];
        let ids: Vec<String> = expired_conversations(&docs, 90, now)
            .into_iter()
            .map(|(i, _)| i)
            .collect();
        assert_eq!(ids, vec!["d1"]);
        assert!(expired_conversations(&docs, 0, now).is_empty());
    }

    #[test]
    fn capacity_is_enforced_only_when_known_and_limited() {
        assert!(capacity_error(Some(10), 10).is_some());
        assert!(capacity_error(Some(9), 10).is_none());
        assert!(capacity_error(Some(10_000), 0).is_none());
        assert!(capacity_error(None, 1).is_none());
    }
}
