//! Ollama model management: installed and loaded models, load/unload,
//! pull with progress, delete.
//!
//! The Ollama base URL is `ai.ollama_host` and passes the `local_only`
//! endpoint guard. Security per action:
//!
//! * list / show: read-only;
//! * load / unload: changes only which models sit in (V)RAM, reversible,
//!   no dialog, audited (`model_load` / `model_unload`);
//! * pull: the Ollama server downloads from its registry. Native
//!   confirmation (size and bandwidth) + audit (`model_pull`);
//! * delete: destructive, so native confirmation (default deny) + audit
//!   (`model_delete`).

use crate::ai::endpoint;
use crate::error::{AppError, AppResult};
use crate::security::audit::{AuditRecord, Confirmation, Decision};
use crate::security::confirm::{self, ConfirmRequest};
use crate::security::policy::{RiskTier, Source};
use crate::state::AppState;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Runtime};

/// An installed model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstalledModel {
    /// Tag, e.g. `qwen3:8b`.
    pub name: String,
    /// Bytes on disk.
    pub size: u64,
    /// Model family.
    pub family: Option<String>,
    /// Parameter count, e.g. `8.2B`.
    pub parameter_size: Option<String>,
    /// Quantization, e.g. `Q4_K_M`.
    pub quantization: Option<String>,
    /// Last modified.
    pub modified_at: Option<String>,
    /// Currently loaded.
    pub loaded: bool,
}

/// A model resident in memory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoadedModel {
    /// Tag.
    pub name: String,
    /// Total bytes in memory.
    pub size: u64,
    /// Bytes in VRAM (the rest is in system RAM, i.e. slower).
    pub size_vram: u64,
    /// Percentage on GPU.
    pub gpu_percent: f32,
    /// Context length the runner was started with.
    pub context_length: Option<u64>,
    /// When it will be unloaded if idle.
    pub expires_at: Option<String>,
    /// Parameter count.
    pub parameter_size: Option<String>,
    /// Quantization.
    pub quantization: Option<String>,
}

/// Validate a model tag before it goes into a request body.
pub fn validate_model(name: &str) -> AppResult<()> {
    let ok = !name.is_empty()
        && name.len() <= 200
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':' | '/'))
        && !name.contains("..");
    if ok {
        Ok(())
    } else {
        Err(AppError::InvalidInput(format!(
            "`{name}` is not a valid model name"
        )))
    }
}

async fn host(state: &AppState) -> AppResult<String> {
    let s = state.settings.read().await.clone();
    endpoint::ensure_endpoint_allowed(&s.ai.ollama_host, s.security.local_only).await?;
    Ok(s.ai.ollama_host.trim_end_matches('/').to_string())
}

async fn get_json(state: &AppState, path: &str) -> AppResult<Value> {
    let base = host(state).await?;
    let r = state
        .http
        .get(format!("{base}{path}"))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| AppError::Unavailable(format!("Ollama is not reachable at {base}: {e}")))?
        .error_for_status()?;
    Ok(r.json().await?)
}

fn detail(v: &Value, k: &str) -> Option<String> {
    v.get("details")
        .and_then(|d| d.get(k))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Models resident in memory (`/api/ps`).
pub async fn loaded(state: &AppState) -> AppResult<Vec<LoadedModel>> {
    let v = get_json(state, "/api/ps").await?;
    Ok(parse_loaded(&v))
}

/// Parse `/api/ps`.
pub fn parse_loaded(v: &Value) -> Vec<LoadedModel> {
    v.get("models")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|m| {
                    let size = m.get("size").and_then(Value::as_u64).unwrap_or(0);
                    let size_vram = m.get("size_vram").and_then(Value::as_u64).unwrap_or(0);
                    LoadedModel {
                        name: m
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        size,
                        size_vram,
                        gpu_percent: if size > 0 {
                            (size_vram as f64 / size as f64 * 100.0) as f32
                        } else {
                            0.0
                        },
                        context_length: m.get("context_length").and_then(Value::as_u64),
                        expires_at: m
                            .get("expires_at")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        parameter_size: detail(m, "parameter_size"),
                        quantization: detail(m, "quantization_level"),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Installed models (`/api/tags`), flagged when loaded.
pub async fn installed(state: &AppState) -> AppResult<Vec<InstalledModel>> {
    let tags = get_json(state, "/api/tags").await?;
    let loaded: Vec<String> = loaded(state)
        .await
        .map(|l| l.into_iter().map(|m| m.name).collect())
        .unwrap_or_default();
    let mut v: Vec<InstalledModel> = tags
        .get("models")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|m| {
                    let name = m
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    InstalledModel {
                        loaded: loaded.contains(&name),
                        size: m.get("size").and_then(Value::as_u64).unwrap_or(0),
                        family: detail(m, "family"),
                        parameter_size: detail(m, "parameter_size"),
                        quantization: detail(m, "quantization_level"),
                        modified_at: m
                            .get("modified_at")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        name,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| b.loaded.cmp(&a.loaded).then(a.name.cmp(&b.name)));
    Ok(v)
}

fn record(
    action: &str,
    model: &str,
    source: Source,
    decision: Decision,
    c: Confirmation,
    detail: Option<String>,
    ms: u64,
) -> AuditRecord {
    AuditRecord {
        id: uuid::Uuid::new_v4().to_string(),
        source,
        action: action.into(),
        command: model.to_string(),
        cwd: None,
        tier: RiskTier::Mutating,
        decision,
        confirmation: c,
        exit_code: None,
        duration_ms: Some(ms),
        detail,
    }
}

async fn generate_keep_alive(state: &AppState, model: &str, keep_alive: Value) -> AppResult<()> {
    let base = host(state).await?;
    state
        .http
        .post(format!("{base}/api/generate"))
        .timeout(Duration::from_secs(300))
        .json(&json!({ "model": model, "keep_alive": keep_alive }))
        .send()
        .await
        .map_err(|e| AppError::Unavailable(format!("Ollama at {base}: {e}")))?
        .error_for_status()?;
    Ok(())
}

/// Free a model's (V)RAM now (it reloads on next use). Audited.
pub async fn unload(state: &AppState, model: &str, source: Source) -> AppResult<()> {
    validate_model(model)?;
    let t = Instant::now();
    let r = generate_keep_alive(state, model, json!(0)).await;
    state
        .audit
        .record(record(
            "model_unload",
            model,
            source,
            if r.is_ok() {
                Decision::Allowed
            } else {
                Decision::Failed
            },
            Confirmation::NotRequired,
            r.as_ref().err().map(ToString::to_string),
            t.elapsed().as_millis() as u64,
        ))
        .await?;
    r
}

/// Load a model into memory now and keep it for `keep_alive` (e.g. `30m`). Audited.
pub async fn load(
    state: &AppState,
    model: &str,
    keep_alive: &str,
    source: Source,
) -> AppResult<()> {
    validate_model(model)?;
    let ka = keep_alive.trim();
    let valid_ka = ka == "-1"
        || (ka.len() <= 6
            && ka.len() >= 2
            && ka[..ka.len() - 1].chars().all(|c| c.is_ascii_digit())
            && matches!(ka.chars().last(), Some('s' | 'm' | 'h')));
    if !valid_ka {
        return Err(AppError::InvalidInput(
            "keep_alive must look like 30m, 2h or -1".into(),
        ));
    }
    let t = Instant::now();
    let r = generate_keep_alive(state, model, json!(ka)).await;
    state
        .audit
        .record(record(
            "model_load",
            model,
            source,
            if r.is_ok() {
                Decision::Allowed
            } else {
                Decision::Failed
            },
            Confirmation::NotRequired,
            Some(format!("keep_alive {ka}")),
            t.elapsed().as_millis() as u64,
        ))
        .await?;
    r
}

/// Delete a model from disk after native confirmation. Audited.
pub async fn delete<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    model: &str,
    source: Source,
) -> AppResult<()> {
    validate_model(model)?;
    let size = installed(state)
        .await?
        .into_iter()
        .find(|m| m.name == model)
        .map(|m| m.size)
        .ok_or_else(|| AppError::InvalidInput(format!("model `{model}` is not installed")))?;
    let current = state.settings.read().await.ai.ollama_model.clone();
    let mut details = vec![format!("Frees {:.1} GB on disk", size as f64 / 1e9)];
    if current == model {
        details.push(
            "This is the model OMNIX is set to use; chat will fail until you pick another.".into(),
        );
    }
    let c = confirm::require(
        app,
        state,
        &ConfirmRequest {
            title: "Delete model?".into(),
            subject: format!("Model:\n{model}"),
            details,
            tier: RiskTier::Mutating,
            source,
            reasons: vec!["removes the model files; pulling it again downloads it in full".into()],
            approve_label: "Delete".into(),
        },
    )
    .await;
    let c = match c {
        Ok(c) => c,
        Err(c) => {
            state
                .audit
                .record(record(
                    "model_delete",
                    model,
                    source,
                    Decision::NotApproved,
                    c,
                    None,
                    0,
                ))
                .await?;
            return Err(AppError::NotApproved(
                "model deletion was not approved".into(),
            ));
        }
    };
    let t = Instant::now();
    let base = host(state).await?;
    let r = state
        .http
        .delete(format!("{base}/api/delete"))
        .timeout(Duration::from_secs(60))
        .json(&json!({ "model": model }))
        .send()
        .await
        .map_err(|e| AppError::Unavailable(format!("Ollama at {base}: {e}")))
        .and_then(|r| r.error_for_status().map_err(AppError::from));
    state
        .audit
        .record(record(
            "model_delete",
            model,
            source,
            if r.is_ok() {
                Decision::Allowed
            } else {
                Decision::Failed
            },
            c,
            r.as_ref().err().map(ToString::to_string),
            t.elapsed().as_millis() as u64,
        ))
        .await?;
    r.map(|_| ())
}

/// Pull progress for the UI.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PullProgress {
    /// Ollama's status line (`pulling manifest`, `downloading …`, `success`).
    pub status: String,
    /// Bytes done for the current layer.
    pub completed: Option<u64>,
    /// Layer size.
    pub total: Option<u64>,
}

/// Download a model (native confirmation first). `progress` receives
/// Ollama's streamed status lines.
pub async fn pull<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    model: &str,
    source: Source,
    progress: &(dyn Fn(PullProgress) + Send + Sync),
) -> AppResult<()> {
    validate_model(model)?;
    let c = confirm::require(
        app,
        state,
        &ConfirmRequest {
            title: "Download model?".into(),
            subject: format!("Model:\n{model}"),
            details: vec![
                "Ollama downloads it from its model registry; models are typically 1–40 GB.".into(),
                "No prompts or personal data are sent.".into(),
            ],
            tier: RiskTier::Mutating,
            source,
            reasons: vec!["uses disk space and network bandwidth".into()],
            approve_label: "Download".into(),
        },
    )
    .await;
    let c = match c {
        Ok(c) => c,
        Err(c) => {
            state
                .audit
                .record(record(
                    "model_pull",
                    model,
                    source,
                    Decision::NotApproved,
                    c,
                    None,
                    0,
                ))
                .await?;
            return Err(AppError::NotApproved(
                "model download was not approved".into(),
            ));
        }
    };
    let t = Instant::now();
    let r = pull_stream(state, model, progress).await;
    let long = u64::from(state.settings.read().await.phone.long_job_minutes);
    if long > 0 && t.elapsed() >= std::time::Duration::from_secs(long * 60) {
        let mins = t.elapsed().as_secs() / 60;
        let text = match &r {
            Ok(()) => format!("✓ Model {model} finished downloading ({mins} min)."),
            Err(e) => format!("✗ Model {model} download failed after {mins} min: {e}"),
        };
        crate::phone::spawn_text(app, text, format!("model download {model}"));
    }
    state
        .audit
        .record(record(
            "model_pull",
            model,
            source,
            if r.is_ok() {
                Decision::Allowed
            } else {
                Decision::Failed
            },
            c,
            r.as_ref().err().map(ToString::to_string),
            t.elapsed().as_millis() as u64,
        ))
        .await?;
    r
}

async fn pull_stream(
    state: &AppState,
    model: &str,
    progress: &(dyn Fn(PullProgress) + Send + Sync),
) -> AppResult<()> {
    let base = host(state).await?;
    let resp = state
        .http
        .post(format!("{base}/api/pull"))
        .timeout(Duration::from_secs(6 * 3600))
        .json(&json!({ "model": model, "stream": true }))
        .send()
        .await
        .map_err(|e| AppError::Unavailable(format!("Ollama at {base}: {e}")))?
        .error_for_status()?;
    let mut buf = String::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        buf.push_str(&String::from_utf8_lossy(&chunk?));
        while let Some(i) = buf.find('\n') {
            let line: String = buf.drain(..=i).collect();
            let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
                continue;
            };
            if let Some(err) = v.get("error").and_then(Value::as_str) {
                return Err(AppError::Unavailable(format!("pull failed: {err}")));
            }
            progress(PullProgress {
                status: v
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                completed: v.get("completed").and_then(Value::as_u64),
                total: v.get("total").and_then(Value::as_u64),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_names_are_validated() {
        for ok in [
            "qwen3:8b",
            "nomic-embed-text",
            "hf.co/org/model:Q4_K_M",
            "llama3.2",
        ] {
            assert!(validate_model(ok).is_ok(), "{ok}");
        }
        for bad in ["", ":x", "a b", "a;b", "../x", "x\"y", "a..b"] {
            assert!(validate_model(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn parses_ps() {
        let v: Value = serde_json::from_str(
            r#"{"models":[{"name":"qwen3:8b","size":6715017789,"size_vram":5000000000,
                "details":{"parameter_size":"8.2B","quantization_level":"Q4_K_M"},
                "expires_at":"2026-09-25T17:30:43-05:00","context_length":8192}]}"#,
        )
        .expect("json");
        let l = parse_loaded(&v);
        assert_eq!(l[0].name, "qwen3:8b");
        assert_eq!(l[0].context_length, Some(8192));
        assert!((l[0].gpu_percent - 74.46).abs() < 0.1);
        assert_eq!(l[0].quantization.as_deref(), Some("Q4_K_M"));
    }
}
