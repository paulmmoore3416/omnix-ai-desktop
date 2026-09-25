//! OpenAI-compatible Chat Completions provider (OpenAI, xAI, Google Gemini's
//! OpenAI endpoint). Streaming SSE with incremental `tool_calls` deltas;
//! models discovered via `GET {base}/models`.

use crate::ai::provider::{
    parse_args, ChatEvent, ChatMessage, ChatOptions, ChatStream, LlmProvider, Role, StopReason,
    ToolCall, ToolSpec,
};
use crate::ai::stream::{receiver_stream, SseParser};
use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Duration;

/// API base URL for an OpenAI-compatible provider id.
pub fn base_url(provider: &str) -> Option<&'static str> {
    match provider {
        "openai" => Some("https://api.openai.com/v1"),
        "xai" => Some("https://api.x.ai/v1"),
        "gemini" => Some("https://generativelanguage.googleapis.com/v1beta/openai"),
        _ => None,
    }
}

/// OpenAI-compatible backend.
pub struct OpenAiCompatProvider {
    id: &'static str,
    http: reqwest::Client,
    base: String,
    api_key: String,
}

impl OpenAiCompatProvider {
    /// Create a provider; `id` must be one accepted by [`base_url`].
    pub fn new(id: &'static str, http: reqwest::Client, api_key: String) -> AppResult<Self> {
        let base = base_url(id)
            .ok_or_else(|| AppError::InvalidInput(format!("unknown provider `{id}`")))?;
        Ok(Self {
            id,
            http,
            base: base.to_string(),
            api_key,
        })
    }
}

/// Convert history to Chat Completions messages.
pub fn to_openai_messages(messages: &[ChatMessage]) -> Vec<Value> {
    messages
        .iter()
        .map(|m| match m.role {
            Role::System => json!({ "role": "system", "content": m.content }),
            Role::User => json!({ "role": "user", "content": m.content }),
            Role::Assistant => {
                let mut v = json!({ "role": "assistant", "content": m.content });
                if !m.tool_calls.is_empty() {
                    v["tool_calls"] = m
                        .tool_calls
                        .iter()
                        .map(|c| {
                            json!({ "id": c.id, "type": "function", "function": {
                                "name": c.name, "arguments": c.arguments.to_string() } })
                        })
                        .collect();
                }
                v
            }
            Role::Tool => json!({
                "role": "tool",
                "tool_call_id": m.tool_call_id.clone().unwrap_or_default(),
                "content": m.content,
            }),
        })
        .collect()
}

/// Accumulates streamed `tool_calls` fragments keyed by `index`.
#[derive(Debug, Default)]
pub struct OpenAiStreamState {
    calls: BTreeMap<u64, (String, String, String)>,
    finish: Option<String>,
}

impl OpenAiStreamState {
    /// Handle one `data:` payload. Returns events and whether the stream ended.
    pub fn handle(&mut self, data: &str) -> AppResult<(Vec<ChatEvent>, bool)> {
        let mut out = Vec::new();
        if data.trim() == "[DONE]" {
            for (_, (id, name, args)) in std::mem::take(&mut self.calls) {
                out.push(ChatEvent::ToolCall(ToolCall {
                    id: if id.is_empty() {
                        format!("call_{}", uuid::Uuid::new_v4().simple())
                    } else {
                        id
                    },
                    name,
                    arguments: parse_args(&args),
                }));
            }
            let stop = match self.finish.as_deref() {
                Some("length") => StopReason::MaxTokens,
                Some("tool_calls") => StopReason::ToolUse,
                Some("content_filter") => StopReason::Refusal,
                Some("stop") | None => {
                    if out.is_empty() {
                        StopReason::EndTurn
                    } else {
                        StopReason::ToolUse
                    }
                }
                Some(o) => StopReason::Other(o.to_string()),
            };
            out.push(ChatEvent::Done { stop, raw: None });
            return Ok((out, true));
        }
        let v: Value = serde_json::from_str(data)?;
        if let Some(err) = v.get("error") {
            let msg = err["message"].as_str().unwrap_or("stream error");
            return Err(AppError::Unavailable(msg.to_string()));
        }
        let choice = &v["choices"][0];
        let delta = &choice["delta"];
        if let Some(t) = delta["content"].as_str() {
            if !t.is_empty() {
                out.push(ChatEvent::Token(t.to_string()));
            }
        }
        if let Some(calls) = delta["tool_calls"].as_array() {
            for c in calls {
                let idx = c["index"].as_u64().unwrap_or(0);
                let entry = self.calls.entry(idx).or_default();
                if let Some(id) = c["id"].as_str() {
                    entry.0 = id.to_string();
                }
                if let Some(n) = c["function"]["name"].as_str() {
                    entry.1.push_str(n);
                }
                if let Some(a) = c["function"]["arguments"].as_str() {
                    entry.2.push_str(a);
                }
            }
        }
        if let Some(f) = choice["finish_reason"].as_str() {
            self.finish = Some(f.to_string());
        }
        Ok((out, false))
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatProvider {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn list_models(&self) -> AppResult<Vec<String>> {
        let resp = self
            .http
            .get(format!("{}/models", self.base))
            .bearer_auth(&self.api_key)
            .timeout(Duration::from_secs(15))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            return Err(match status.as_u16() {
                401 | 403 => {
                    AppError::Secret(format!("{} rejected the API key ({status})", self.id))
                }
                _ => AppError::Unavailable(format!("{} returned {status}", self.id)),
            });
        }
        let v: Value = resp.json().await?;
        let mut ids: Vec<String> = v["data"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| m["id"].as_str())
                    // Gemini prefixes ids with "models/".
                    .map(|s| s.trim_start_matches("models/").to_string())
                    .collect()
            })
            .unwrap_or_default();
        ids.sort();
        Ok(ids)
    }

    async fn health_check(&self) -> AppResult<String> {
        let n = self.list_models().await?.len();
        Ok(format!("Connected to {} ({n} models available)", self.id))
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSpec],
        opts: &ChatOptions,
    ) -> AppResult<ChatStream> {
        let mut body = json!({
            "model": opts.model,
            "messages": to_openai_messages(messages),
            "stream": true,
            "temperature": opts.temperature,
        });
        // OpenAI's newer models only accept `max_completion_tokens`.
        let limit_key = if self.id == "openai" {
            "max_completion_tokens"
        } else {
            "max_tokens"
        };
        body[limit_key] = json!(opts.max_tokens);
        if !tools.is_empty() {
            body["tools"] = tools
                .iter()
                .map(|t| {
                    json!({ "type": "function", "function": {
                    "name": t.name, "description": t.description, "parameters": t.parameters } })
                })
                .collect();
        }
        let send = |b: &Value| {
            self.http
                .post(format!("{}/chat/completions", self.base))
                .bearer_auth(&self.api_key)
                .json(b)
                .send()
        };
        let mut resp = send(&body)
            .await
            .map_err(|e| AppError::Unavailable(format!("{} is not reachable: {e}", self.id)))?;
        if resp.status() == reqwest::StatusCode::BAD_REQUEST {
            // Reasoning models reject sampling parameters; retry once without.
            let text = resp.text().await.unwrap_or_default();
            if text.contains("temperature") {
                if let Some(obj) = body.as_object_mut() {
                    obj.remove("temperature");
                }
                resp = send(&body).await?;
            } else {
                return Err(AppError::Unavailable(format!(
                    "{} returned 400: {}",
                    self.id,
                    text.chars().take(300).collect::<String>()
                )));
            }
        }
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(match status.as_u16() {
                401 | 403 => {
                    AppError::Secret(format!("{} rejected the API key ({status})", self.id))
                }
                _ => AppError::Unavailable(format!(
                    "{} returned {status}: {}",
                    self.id,
                    text.chars().take(300).collect::<String>()
                )),
            });
        }

        let (tx, rx) = tokio::sync::mpsc::channel(64);
        let id = self.id;
        tokio::spawn(async move {
            let mut bytes = resp.bytes_stream();
            let mut sse = SseParser::default();
            let mut state = OpenAiStreamState::default();
            while let Some(chunk) = bytes.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(Err(AppError::Http(e))).await;
                        return;
                    }
                };
                for ev in sse.push(&chunk) {
                    match state.handle(&ev.data) {
                        Ok((events, finished)) => {
                            for e in events {
                                if tx.send(Ok(e)).await.is_err() {
                                    return;
                                }
                            }
                            if finished {
                                return;
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e)).await;
                            return;
                        }
                    }
                }
            }
            let _ = tx
                .send(Err(AppError::Unavailable(format!(
                    "{id} stream ended unexpectedly"
                ))))
                .await;
        });
        Ok(receiver_stream(rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulates_fragmented_tool_calls() {
        let mut s = OpenAiStreamState::default();
        let mut out = Vec::new();
        for d in [
            r#"{"choices":[{"delta":{"content":"Let me look."}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read_file","arguments":"{\"pa"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"th\":\"/x\"}"}}]},"finish_reason":"tool_calls"}]}"#,
            "[DONE]",
        ] {
            out.extend(s.handle(d).expect("chunk").0);
        }
        assert_eq!(out[0], ChatEvent::Token("Let me look.".into()));
        match &out[1] {
            ChatEvent::ToolCall(c) => {
                assert_eq!(c.id, "call_1");
                assert_eq!(c.arguments["path"], "/x");
            }
            o => panic!("unexpected {o:?}"),
        }
        assert_eq!(
            out[2],
            ChatEvent::Done {
                stop: StopReason::ToolUse,
                raw: None
            }
        );
    }

    #[test]
    fn maps_history_and_bases() {
        let call = ToolCall {
            id: "c".into(),
            name: "n".into(),
            arguments: json!({"a": 1}),
        };
        let mut a = ChatMessage::text(Role::Assistant, "");
        a.tool_calls.push(call.clone());
        let m = to_openai_messages(&[a, ChatMessage::tool_result(&call, "ok".into(), false)]);
        assert_eq!(m[0]["tool_calls"][0]["function"]["arguments"], "{\"a\":1}");
        assert_eq!(m[1]["tool_call_id"], "c");
        assert!(base_url("openai").is_some());
        assert!(base_url("ollama").is_none());
    }
}
