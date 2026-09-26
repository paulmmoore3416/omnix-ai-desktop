//! kb-core REST adapter (see `docs/kb-core-contract.md`).
//!
//! Core endpoints map errors per the contract. Extension endpoints are
//! optional: a 404/405/501 from one of them means "this service doesn't
//! offer it" and becomes [`AppError::NotImplemented`], so OMNIX keeps working
//! against a minimal kb-core.

use super::{DocumentReceipt, MemoryRecord, MemoryStore, NewMemory, SearchHit};
use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

/// HTTP client for a kb-core service.
pub struct KbCoreStore {
    http: reqwest::Client,
    base: String,
    token: Option<String>,
}

impl KbCoreStore {
    /// `base` is `memory.backend_url` (endpoint policy checked by the caller);
    /// `token` is the optional bearer token from the keychain (`kb_core`).
    pub fn new(http: reqwest::Client, base: &str, token: Option<String>) -> Self {
        Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            token,
        }
    }

    fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let r = self
            .http
            .request(method, format!("{}{path}", self.base))
            .timeout(Duration::from_secs(30));
        match &self.token {
            Some(t) => r.bearer_auth(t),
            None => r,
        }
    }

    async fn send(&self, rb: reqwest::RequestBuilder) -> AppResult<reqwest::Response> {
        let resp = rb
            .send()
            .await
            .map_err(|e| AppError::Unavailable(format!("kb-core at {}: {e}", self.base)))?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let text = resp.text().await.unwrap_or_default();
        Err(match status.as_u16() {
            401 | 403 => AppError::Secret(format!("kb-core rejected the token ({status})")),
            404 => AppError::InvalidInput(format!("kb-core: not found ({status})")),
            _ => AppError::Unavailable(format!(
                "kb-core returned {status}: {}",
                text.chars().take(300).collect::<String>()
            )),
        })
    }
}

impl KbCoreStore {
    /// Call an optional extension endpoint and decode its JSON body.
    async fn ext(&self, rb: reqwest::RequestBuilder, feature: &'static str) -> AppResult<Value> {
        let resp = rb
            .send()
            .await
            .map_err(|e| AppError::Unavailable(format!("kb-core at {}: {e}", self.base)))?;
        match resp.status().as_u16() {
            404 | 405 | 501 => {
                let body = resp.text().await.unwrap_or_default();
                // A 404 about a missing *record* is a real "not found"; a 404
                // for the route itself means the extension isn't offered.
                if body.contains("not found") && !body.contains("no such endpoint") {
                    return Err(AppError::InvalidInput(format!(
                        "kb-core: {feature}: not found"
                    )));
                }
                Err(AppError::NotImplemented(feature))
            }
            401 | 403 => Err(AppError::Secret(format!(
                "kb-core rejected the token ({})",
                resp.status()
            ))),
            s if (200..300).contains(&s) => Ok(resp.json().await?),
            _ => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                Err(AppError::Unavailable(format!(
                    "kb-core {feature} returned {status}: {}",
                    text.chars().take(300).collect::<String>()
                )))
            }
        }
    }
}

/// Ids are interpolated into URL paths, so only a safe character set is allowed.
fn validate_id(id: &str) -> AppResult<()> {
    if !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Ok(())
    } else {
        Err(AppError::InvalidInput("invalid memory id".into()))
    }
}

#[derive(Deserialize)]
struct ListResp {
    memories: Vec<MemoryRecord>,
}
#[derive(Deserialize)]
struct IdResp {
    id: String,
}
#[derive(Deserialize)]
struct SearchResp {
    results: Vec<SearchHit>,
}

#[async_trait]
impl MemoryStore for KbCoreStore {
    async fn health(&self) -> AppResult<String> {
        self.send(
            self.req(reqwest::Method::GET, "/health")
                .timeout(Duration::from_secs(5)),
        )
        .await?;
        Ok(format!("kb-core is reachable at {}", self.base))
    }

    async fn list(&self, limit: u32) -> AppResult<Vec<MemoryRecord>> {
        let r = self
            .send(
                self.req(reqwest::Method::GET, "/memories")
                    .query(&[("limit", limit.min(500))]),
            )
            .await?;
        Ok(r.json::<ListResp>().await?.memories)
    }

    async fn save(&self, m: NewMemory) -> AppResult<String> {
        let r = self
            .send(self.req(reqwest::Method::POST, "/memories").json(&m))
            .await?;
        Ok(r.json::<IdResp>().await?.id)
    }

    async fn delete(&self, id: &str) -> AppResult<()> {
        validate_id(id)?;
        self.send(self.req(reqwest::Method::DELETE, &format!("/memories/{id}")))
            .await?;
        Ok(())
    }

    async fn search(&self, query: &str, limit: u32) -> AppResult<Vec<SearchHit>> {
        let r = self
            .send(
                self.req(reqwest::Method::POST, "/search")
                    .json(&json!({ "query": query, "limit": limit.clamp(1, 50) })),
            )
            .await?;
        Ok(r.json::<SearchResp>().await?.results)
    }

    async fn recall(&self, query: &str, limit: u32, min_score: f32) -> AppResult<Vec<SearchHit>> {
        // kb-core filters server-side; a service that ignores `min_score` is
        // filtered here too.
        let r = self
            .send(
                self.req(reqwest::Method::POST, "/search")
                    .json(&recall_body(query, limit, min_score)),
            )
            .await?;
        let mut hits = r.json::<SearchResp>().await?.results;
        hits.retain(|h| h.score >= min_score);
        Ok(hits)
    }

    async fn search_in(
        &self,
        query: &str,
        limit: u32,
        collection: &str,
    ) -> AppResult<Vec<SearchHit>> {
        let r = self
            .send(self.req(reqwest::Method::POST, "/search").json(&json!({
                "query": query, "limit": limit.clamp(1, 50), "collections": [collection]
            })))
            .await?;
        Ok(r.json::<SearchResp>().await?.results)
    }

    async fn save_detailed(&self, m: NewMemory) -> AppResult<Value> {
        let r = self
            .send(self.req(reqwest::Method::POST, "/memories").json(&m))
            .await?;
        let v: Value = r.json().await?;
        if v.get("id").and_then(Value::as_str).is_none() {
            return Err(AppError::Unavailable(
                "kb-core returned no memory id".into(),
            ));
        }
        Ok(v)
    }

    async fn stats(&self) -> AppResult<Value> {
        self.ext(
            self.req(reqwest::Method::GET, "/stats"),
            "memory statistics",
        )
        .await
    }

    async fn list_documents(&self) -> AppResult<Vec<Value>> {
        let v = self
            .ext(
                self.req(reqwest::Method::GET, "/documents"),
                "document listing",
            )
            .await?;
        Ok(v.get("documents")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    async fn list_hidden(&self, limit: u32) -> AppResult<Vec<Value>> {
        let v = self
            .ext(
                self.req(reqwest::Method::GET, "/memories").query(&[
                    ("hidden_only", "true".to_string()),
                    ("limit", limit.clamp(1, 500).to_string()),
                ]),
                "memory history",
            )
            .await?;
        Ok(hidden_only(v))
    }

    async fn delete_document(&self, id: &str) -> AppResult<()> {
        validate_id(id)?;
        self.ext(
            self.req(reqwest::Method::DELETE, &format!("/documents/{id}")),
            "document deletion",
        )
        .await
        .map(|_| ())
    }

    async fn update_memory(&self, id: &str, patch: Value) -> AppResult<Value> {
        validate_id(id)?;
        self.ext(
            self.req(reqwest::Method::PATCH, &format!("/memories/{id}"))
                .json(&patch),
            "memory editing",
        )
        .await
    }

    async fn extract(&self, text: &str) -> AppResult<Vec<Value>> {
        let v = self
            .ext(
                self.req(reqwest::Method::POST, "/extract")
                    .timeout(Duration::from_secs(180))
                    .json(&json!({ "text": text })),
                "fact capture",
            )
            .await?;
        Ok(v.get("memories")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    async fn export(&self) -> AppResult<String> {
        let resp = self
            .req(reqwest::Method::GET, "/export")
            .timeout(Duration::from_secs(300))
            .send()
            .await
            .map_err(|e| AppError::Unavailable(format!("kb-core at {}: {e}", self.base)))?;
        match resp.status().as_u16() {
            404 | 405 | 501 => Err(AppError::NotImplemented("export")),
            s if (200..300).contains(&s) => Ok(resp.text().await?),
            _ => Err(AppError::Unavailable(format!(
                "kb-core export returned {}",
                resp.status()
            ))),
        }
    }

    async fn import(&self, records: Vec<Value>) -> AppResult<Value> {
        self.ext(
            self.req(reqwest::Method::POST, "/import")
                .timeout(Duration::from_secs(1800))
                .json(&json!({ "records": records })),
            "import",
        )
        .await
    }

    async fn maintenance(&self) -> AppResult<Value> {
        self.ext(
            self.req(reqwest::Method::POST, "/maintenance")
                .timeout(Duration::from_secs(600)),
            "optimization",
        )
        .await
    }

    async fn index_document_in(
        &self,
        name: &str,
        content: &str,
        collection: &str,
    ) -> AppResult<DocumentReceipt> {
        let r = self
            .send(
                self.req(reqwest::Method::POST, "/documents")
                    .timeout(Duration::from_secs(300))
                    .json(&json!({ "name": name, "content": content, "mime_type": "text/markdown", "collection": collection })),
            )
            .await?;
        Ok(r.json().await?)
    }

    async fn list_collections(&self) -> AppResult<Vec<Value>> {
        let v = self
            .ext(
                self.req(reqwest::Method::GET, "/collections"),
                "knowledge bases",
            )
            .await?;
        Ok(v.get("collections")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    async fn create_collection(&self, name: &str, description: &str) -> AppResult<Value> {
        self.ext(
            self.req(reqwest::Method::POST, "/collections")
                .json(&json!({ "name": name, "description": description })),
            "knowledge bases",
        )
        .await
    }

    async fn delete_collection(&self, name: &str) -> AppResult<Value> {
        validate_id(name)?;
        self.ext(
            self.req(reqwest::Method::DELETE, &format!("/collections/{name}")),
            "knowledge bases",
        )
        .await
    }

    async fn sync_folders(&self) -> AppResult<Value> {
        self.ext(
            self.req(reqwest::Method::POST, "/sync")
                .timeout(Duration::from_secs(1800)),
            "folder sync",
        )
        .await
    }

    async fn index_document(&self, name: &str, content: &str) -> AppResult<DocumentReceipt> {
        let r = self
            .send(
                self.req(reqwest::Method::POST, "/documents")
                    .timeout(Duration::from_secs(300))
                    .json(&json!({ "name": name, "content": content, "mime_type": "text/plain" })),
            )
            .await?;
        Ok(r.json().await?)
    }
}

/// Request body for automatic recall.
fn recall_body(query: &str, limit: u32, min_score: f32) -> Value {
    json!({
        "query": query, "limit": limit.clamp(1, 50), "min_score": min_score,
        // The archive of the current conversation would only echo it.
        "exclude_collections": ["conversations"],
        // Auto-recall runs on every message, so counting its hits as "use"
        // would feed activation back into itself: a memory recalled once
        // ranks higher and gets recalled again (a filter bubble). Only
        // deliberate use (the `search_memory` tool, restating, pinning)
        // strengthens a memory.
        "track": false,
    })
}

/// Keep only memories that are actually hidden. A contract-only service
/// ignores `hidden_only` and returns live memories; listing those as
/// history with a Restore button would be misleading.
fn hidden_only(v: Value) -> Vec<Value> {
    v.get("memories")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|m| m.get("superseded_by").is_some_and(|s| !s.is_null()))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_recall_does_not_count_as_use() {
        let b = recall_body("what editor do I use", 99, 0.4);
        assert_eq!(b["track"], false);
        assert_eq!(b["limit"], 50);
        assert_eq!(b["exclude_collections"], json!(["conversations"]));
    }

    #[test]
    fn hidden_listing_ignores_live_memories() {
        let v = json!({"memories": [
            {"id": "m1", "content": "old", "superseded_by": "m2", "hidden_reason": "merged"},
            {"id": "m3", "content": "live", "superseded_by": null},
            {"id": "m4", "content": "contract-only service, no field"}
        ]});
        let h = hidden_only(v);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0]["id"], "m1");
        assert!(hidden_only(json!({})).is_empty());
    }

    #[test]
    fn ids_are_path_safe() {
        assert!(validate_id("abc-123_X").is_ok());
        assert!(validate_id("../etc").is_err());
        assert!(validate_id("a/b").is_err());
        assert!(validate_id("").is_err());
        assert!(validate_id(&"a".repeat(129)).is_err());
    }

    #[test]
    fn response_shapes_deserialize() {
        let l: ListResp =
            serde_json::from_str(r#"{"memories":[{"id":"1","content":"x"}]}"#).expect("list");
        assert_eq!(l.memories[0].importance, 0);
        let s: SearchResp =
            serde_json::from_str(r#"{"results":[{"id":"1","content":"x","score":0.9}]}"#)
                .expect("search");
        assert!((s.results[0].score - 0.9).abs() < 1e-6);
        let d: DocumentReceipt = serde_json::from_str(r#"{"id":"d1","chunks":4}"#).expect("doc");
        assert_eq!(d.chunks, 4);
        // kb-core 2 adds fields; they are optional and extra ones are ignored.
        let s: SearchResp = serde_json::from_str(
            r#"{"results":[{"id":"c_1","content":"x","score":0.7,"similarity":0.81,"kind":"document",
                "source":"notes/a.md","explain":{"semantic_rank":1},"superseded_by":null}],"degraded":false}"#,
        )
        .expect("search v2");
        assert_eq!(s.results[0].kind.as_deref(), Some("document"));
        assert_eq!(s.results[0].source.as_deref(), Some("notes/a.md"));
        assert_eq!(s.results[0].similarity, Some(0.81));
        assert_eq!(s.results[0].origin, None);
        let s: SearchResp = serde_json::from_str(
            r#"{"results":[{"id":"m_1","content":"x","score":0.6,"kind":"memory","origin":"assistant"}]}"#,
        )
        .expect("search with origin");
        assert_eq!(s.results[0].origin.as_deref(), Some("assistant"));
    }
}
