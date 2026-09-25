//! Minimal Ollama client (non-streaming). Replaced by the streaming
//! `LlmProvider` implementation in the intelligence phase.

use crate::error::{AppError, AppResult};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagModel>,
}

#[derive(Debug, Deserialize)]
struct TagModel {
    name: String,
}

#[derive(Debug, Deserialize)]
struct GenerateResponse {
    response: String,
}

/// Names of locally installed models (`GET /api/tags`).
pub async fn list_models(http: &reqwest::Client, host: &str) -> AppResult<Vec<String>> {
    let url = format!("{}/api/tags", host.trim_end_matches('/'));
    let resp = http
        .get(&url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| AppError::Unavailable(format!("Ollama is not reachable at {host}: {e}")))?
        .error_for_status()?;
    let tags: TagsResponse = resp.json().await?;
    let mut names: Vec<String> = tags.models.into_iter().map(|m| m.name).collect();
    names.sort();
    Ok(names)
}

/// One-shot completion (`POST /api/generate`, `stream: false`).
pub async fn generate(
    http: &reqwest::Client,
    host: &str,
    model: &str,
    prompt: &str,
    temperature: f32,
    max_tokens: u32,
) -> AppResult<String> {
    let url = format!("{}/api/generate", host.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "options": { "temperature": temperature, "num_predict": max_tokens }
    });
    let resp =
        http.post(&url).json(&body).send().await.map_err(|e| {
            AppError::Unavailable(format!("Ollama is not reachable at {host}: {e}"))
        })?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::Unavailable(format!(
            "Ollama returned {status}: {}",
            text.chars().take(300).collect::<String>()
        )));
    }
    let parsed: GenerateResponse = resp.json().await?;
    Ok(parsed.response)
}
