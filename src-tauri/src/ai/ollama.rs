//! Ollama provider: streaming `POST /api/chat` (NDJSON) with native tool
//! calling, and model discovery via `GET /api/tags`.

use crate::ai::provider::{
    parse_args, ChatEvent, ChatMessage, ChatOptions, ChatStream, LlmProvider, Role, StopReason,
    ToolCall, ToolSpec, Usage,
};
use crate::ai::stream::{receiver_stream, LineBuffer};
use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
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

/// Names of locally installed models (`GET /api/tags`), sorted.
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

/// Ollama backend.
pub struct OllamaProvider {
    http: reqwest::Client,
    host: String,
}

impl OllamaProvider {
    /// Create a provider for `host` (endpoint policy is checked by the factory).
    pub fn new(http: reqwest::Client, host: impl Into<String>) -> Self {
        Self {
            http,
            host: host.into().trim_end_matches('/').to_string(),
        }
    }
}

/// Convert history to Ollama's `/api/chat` message format.
fn to_ollama_messages(messages: &[ChatMessage]) -> Vec<Value> {
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
                        .map(
                            |c| json!({ "function": { "name": c.name, "arguments": c.arguments } }),
                        )
                        .collect();
                }
                v
            }
            Role::Tool => json!({
                "role": "tool",
                "content": m.content,
                "tool_name": m.tool_name.clone().unwrap_or_default(),
            }),
        })
        .collect()
}

/// Parse one NDJSON line into events. Returns `Ok(true)` when `done`.
fn parse_line(line: &str, saw_tools: &mut bool, out: &mut Vec<ChatEvent>) -> AppResult<bool> {
    let v: Value = serde_json::from_str(line)?;
    if let Some(err) = v.get("error").and_then(Value::as_str) {
        return Err(AppError::Unavailable(format!("Ollama: {err}")));
    }
    if let Some(msg) = v.get("message") {
        if let Some(text) = msg.get("content").and_then(Value::as_str) {
            if !text.is_empty() {
                out.push(ChatEvent::Token(text.to_string()));
            }
        }
        if let Some(calls) = msg.get("tool_calls").and_then(Value::as_array) {
            for c in calls {
                let f = &c["function"];
                let arguments = match &f["arguments"] {
                    Value::String(s) => parse_args(s),
                    Value::Null => Value::Object(Default::default()),
                    other => other.clone(),
                };
                *saw_tools = true;
                out.push(ChatEvent::ToolCall(ToolCall {
                    id: c
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("call_{}", uuid::Uuid::new_v4().simple())),
                    name: f["name"].as_str().unwrap_or_default().to_string(),
                    arguments,
                }));
            }
        }
    }
    if v.get("done").and_then(Value::as_bool) == Some(true) {
        let stop = match v.get("done_reason").and_then(Value::as_str) {
            Some("length") => StopReason::MaxTokens,
            _ if *saw_tools => StopReason::ToolUse,
            Some("stop") | None => StopReason::EndTurn,
            Some(other) => StopReason::Other(other.to_string()),
        };
        if v.get("eval_count").is_some() {
            let ns_ms = |k: &str| v.get(k).and_then(Value::as_u64).map(|ns| ns / 1_000_000);
            out.push(ChatEvent::Usage(Usage {
                prompt_tokens: v.get("prompt_eval_count").and_then(Value::as_u64),
                output_tokens: v.get("eval_count").and_then(Value::as_u64),
                prompt_ms: ns_ms("prompt_eval_duration"),
                generation_ms: ns_ms("eval_duration"),
                load_ms: ns_ms("load_duration"),
            }));
        }
        out.push(ChatEvent::Done { stop, raw: None });
        return Ok(true);
    }
    Ok(false)
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    async fn list_models(&self) -> AppResult<Vec<String>> {
        list_models(&self.http, &self.host).await
    }

    async fn health_check(&self) -> AppResult<String> {
        let models = self.list_models().await?;
        Ok(format!(
            "Connected to Ollama at {} ({} models installed)",
            self.host,
            models.len()
        ))
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSpec],
        opts: &ChatOptions,
    ) -> AppResult<ChatStream> {
        let mut body = json!({
            "model": opts.model,
            "messages": to_ollama_messages(messages),
            "stream": true,
            "options": {
                "temperature": opts.temperature,
                "num_predict": opts.max_tokens,
                "num_ctx": opts.context_window,
            }
        });
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
                .post(format!("{}/api/chat", self.host))
                .json(b)
                .send()
        };
        let unreachable = |e: reqwest::Error| {
            AppError::Unavailable(format!("Ollama is not reachable at {}: {e}", self.host))
        };
        let mut resp = send(&body).await.map_err(unreachable)?;
        let mut notice = None;
        if resp.status() == reqwest::StatusCode::BAD_REQUEST && body.get("tools").is_some() {
            // Many local models cannot call tools. Answer without them rather
            // than failing every message, and tell the user why.
            let text = resp.text().await.unwrap_or_default();
            if !text.contains("does not support tools") {
                return Err(AppError::Unavailable(format!(
                    "Ollama returned 400: {}",
                    text.chars().take(300).collect::<String>()
                )));
            }
            if let Some(obj) = body.as_object_mut() {
                obj.remove("tools");
            }
            notice = Some(format!(
                "Model `{}` does not support tool calling, so OMNIX answered without tools. \
                 Choose a tool-capable model (e.g. one tagged \"tools\" in the Ollama library) to let it run commands.",
                opts.model
            ));
            resp = send(&body).await.map_err(unreachable)?;
        }
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Unavailable(format!(
                "Ollama returned {status}: {}",
                text.chars().take(300).collect::<String>()
            )));
        }

        let (tx, rx) = tokio::sync::mpsc::channel(64);
        tokio::spawn(async move {
            if let Some(n) = notice {
                if tx.send(Ok(ChatEvent::Notice(n))).await.is_err() {
                    return;
                }
            }
            let mut bytes = resp.bytes_stream();
            let mut lines = LineBuffer::default();
            let mut saw_tools = false;
            let mut done = false;
            while let Some(chunk) = bytes.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(Err(AppError::Http(e))).await;
                        return;
                    }
                };
                for line in lines.push(&chunk) {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let mut evs = Vec::new();
                    match parse_line(&line, &mut saw_tools, &mut evs) {
                        Ok(d) => done |= d,
                        Err(e) => {
                            let _ = tx.send(Err(e)).await;
                            return;
                        }
                    }
                    for ev in evs {
                        // Receiver dropped (cancelled): stop reading; dropping
                        // the body aborts the HTTP request.
                        if tx.send(Ok(ev)).await.is_err() {
                            return;
                        }
                    }
                }
            }
            if !done {
                let _ = tx
                    .send(Err(AppError::Unavailable(
                        "Ollama stream ended unexpectedly".into(),
                    )))
                    .await;
            }
        });
        Ok(receiver_stream(rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::provider::INVALID_ARGS;

    #[test]
    fn parses_tokens_tool_calls_and_done() {
        let mut saw = false;
        let mut out = Vec::new();
        parse_line(
            r#"{"message":{"role":"assistant","content":"Hel"},"done":false}"#,
            &mut saw,
            &mut out,
        )
        .expect("line");
        parse_line(
            r#"{"message":{"role":"assistant","content":"","tool_calls":[{"function":{"name":"run_command","arguments":{"command":"ls"}}}]},"done":false}"#,
            &mut saw,
            &mut out,
        )
        .expect("line");
        let done =
            parse_line(r#"{"done":true,"done_reason":"stop"}"#, &mut saw, &mut out).expect("line");
        assert!(done);
        assert_eq!(out[0], ChatEvent::Token("Hel".into()));
        match &out[1] {
            ChatEvent::ToolCall(c) => {
                assert_eq!(c.name, "run_command");
                assert_eq!(c.arguments["command"], "ls");
                assert!(c.id.starts_with("call_"));
            }
            other => panic!("unexpected {other:?}"),
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
    fn length_maps_to_max_tokens_and_errors_surface() {
        let mut saw = false;
        let mut out = Vec::new();
        parse_line(
            r#"{"done":true,"done_reason":"length"}"#,
            &mut saw,
            &mut out,
        )
        .expect("line");
        assert_eq!(
            out[0],
            ChatEvent::Done {
                stop: StopReason::MaxTokens,
                raw: None
            }
        );
        assert!(parse_line(r#"{"error":"model not found"}"#, &mut saw, &mut out).is_err());
    }

    #[test]
    fn string_arguments_are_parsed_or_flagged() {
        let mut saw = false;
        let mut out = Vec::new();
        parse_line(
            r#"{"message":{"tool_calls":[{"function":{"name":"x","arguments":"{bad"}}]},"done":false}"#,
            &mut saw,
            &mut out,
        )
        .expect("line");
        match &out[0] {
            ChatEvent::ToolCall(c) => assert_eq!(c.arguments[INVALID_ARGS], true),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn history_maps_tool_messages() {
        let call = ToolCall {
            id: "1".into(),
            name: "read_file".into(),
            arguments: json!({"path": "/x"}),
        };
        let mut a = ChatMessage::text(Role::Assistant, "");
        a.tool_calls.push(call.clone());
        let msgs = to_ollama_messages(&[a, ChatMessage::tool_result(&call, "ok".into(), false)]);
        assert_eq!(msgs[0]["tool_calls"][0]["function"]["name"], "read_file");
        assert_eq!(msgs[1]["role"], "tool");
        assert_eq!(msgs[1]["tool_name"], "read_file");
    }
}

/// Live check against a local Ollama. Opt-in:
/// `OMNIX_TEST_OLLAMA_MODEL=<model> cargo test -- --ignored live_ollama`.
#[cfg(test)]
mod live {
    use super::*;
    use crate::ai::provider::ChatEvent;

    #[tokio::test]
    #[ignore = "requires a running Ollama and OMNIX_TEST_OLLAMA_MODEL"]
    async fn live_ollama_streams_a_reply() {
        // Also exercises the no-tools fallback when the model can't call tools.
        let Ok(model) = std::env::var("OMNIX_TEST_OLLAMA_MODEL") else {
            return;
        };
        let p = OllamaProvider::new(reqwest::Client::new(), "http://localhost:11434");
        assert!(p.list_models().await.expect("models").contains(&model));
        let opts = ChatOptions {
            model,
            temperature: 0.0,
            max_tokens: 32,
            context_window: 2048,
        };
        let mut s = p
            .chat_stream(
                &[ChatMessage::text(
                    Role::User,
                    "Reply with the single word: pong",
                )],
                &crate::ai::agent::tool_specs(false),
                &opts,
            )
            .await
            .expect("stream");
        let (mut text, mut done) = (String::new(), false);
        while let Some(ev) = s.next().await {
            match ev.expect("event") {
                ChatEvent::Token(t) => text.push_str(&t),
                ChatEvent::Done { .. } => done = true,
                ChatEvent::ToolCall(_) | ChatEvent::Notice(_) | ChatEvent::Usage(_) => {}
            }
        }
        assert!(done, "stream must finish with Done");
        assert!(!text.trim().is_empty(), "expected some text");
        eprintln!("live reply: {text:?}");
    }
}
