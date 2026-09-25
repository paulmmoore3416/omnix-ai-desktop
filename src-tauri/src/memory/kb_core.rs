//! kb-core REST adapter (see `docs/kb-core-contract.md`).

use super::{DocumentReceipt, MemoryRecord, MemoryStore, NewMemory, SearchHit};
use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
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

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
