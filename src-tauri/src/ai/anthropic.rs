//! Anthropic Messages API provider (raw HTTP; there is no official Rust SDK).
//!
//! * `POST /v1/messages` with `stream: true`, headers `x-api-key` and
//!   `anthropic-version: 2023-06-01`.
//! * SSE events: `message_start`, `content_block_start` / `_delta` / `_stop`
//!   (`text_delta`, `input_json_delta`, `thinking_delta`, `signature_delta`),
//!   `message_delta` (stop_reason), `message_stop`, `error`, `ping`.
//! * Client tools set `eager_input_streaming: true`; the accumulated JSON is
//!   therefore unvalidated by the API and is parsed defensively here
//!   ([`parse_args`]) and validated again by each tool.
//! * Sampling parameters (`temperature`) are **not** sent: current Claude
//!   models reject them. Thinking is left at the model default.
//! * Assistant content blocks (including `thinking` with its signature) are
//!   returned as `raw` so they are replayed unchanged on the next request.
//! * Models are discovered with paginated `GET /v1/models`; no ids are hardcoded.

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

const API_BASE: &str = "https://api.anthropic.com/v1";
const API_VERSION: &str = "2023-06-01";

/// Anthropic backend.
pub struct AnthropicProvider {
    http: reqwest::Client,
    api_key: String,
}

impl AnthropicProvider {
    /// Create a provider with a key fetched from the OS keychain.
    pub fn new(http: reqwest::Client, api_key: String) -> Self {
        Self { http, api_key }
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{API_BASE}{path}"))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
    }
}

/// Convert history to the Messages API shape: `(system, messages)`.
///
/// Consecutive tool results are merged into **one** user message of
/// `tool_result` blocks (splitting them discourages parallel tool use).
pub fn to_anthropic(messages: &[ChatMessage]) -> (String, Vec<Value>) {
    let system = messages
        .iter()
        .filter(|m| m.role == Role::System)
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut out: Vec<Value> = Vec::new();
    for m in messages.iter().filter(|m| m.role != Role::System) {
        match m.role {
            Role::User => out.push(json!({ "role": "user", "content": m.content })),
            Role::Assistant => {
                let content = match &m.provider_raw {
                    Some(raw @ Value::Array(_)) => raw.clone(),
                    _ => {
                        let mut blocks = Vec::new();
                        if !m.content.is_empty() {
                            blocks.push(json!({ "type": "text", "text": m.content }));
                        }
                        for c in &m.tool_calls {
                            blocks.push(json!({ "type": "tool_use", "id": c.id, "name": c.name, "input": c.arguments }));
                        }
                        Value::Array(blocks)
                    }
                };
                if content.as_array().is_some_and(|a| !a.is_empty()) {
                    out.push(json!({ "role": "assistant", "content": content }));
                }
            }
            Role::Tool => {
                let block = json!({
                    "type": "tool_result",
                    "tool_use_id": m.tool_call_id.clone().unwrap_or_default(),
                    "content": m.content,
                    "is_error": m.is_error,
                });
                let merged = out.last_mut().and_then(|last| {
                    let is_results = last["role"] == "user"
                        && last["content"]
                            .as_array()
                            .and_then(|a| a.first())
                            .is_some_and(|b| b["type"] == "tool_result");
                    if is_results {
                        last["content"].as_array_mut()
                    } else {
                        None
                    }
                });
                match merged {
                    Some(arr) => arr.push(block),
                    None => out.push(json!({ "role": "user", "content": [block] })),
                }
            }
            Role::System => {}
        }
    }
    (system, out)
}

/// Per-content-block accumulator.
#[derive(Debug, Default)]
struct Block {
    kind: String,
    text: String,
    id: String,
    name: String,
    json: String,
    signature: String,
    data: String,
}

/// Streaming state machine over SSE events.
#[derive(Debug, Default)]
pub struct AnthropicStreamState {
    blocks: BTreeMap<u64, Block>,
    stop: Option<String>,
}

impl AnthropicStreamState {
    /// Process one SSE event; returns events to emit and whether the message ended.
    pub fn handle(&mut self, event: &str, data: &str) -> AppResult<(Vec<ChatEvent>, bool)> {
        let mut out = Vec::new();
        if data.is_empty() {
            return Ok((out, false));
        }
        let v: Value = serde_json::from_str(data)?;
        let kind = if event.is_empty() {
            v["type"].as_str().unwrap_or("")
        } else {
            event
        };
        let index = v["index"].as_u64().unwrap_or(0);
        match kind {
            "content_block_start" => {
                let cb = &v["content_block"];
                self.blocks.insert(
                    index,
                    Block {
                        kind: cb["type"].as_str().unwrap_or("").to_string(),
                        id: cb["id"].as_str().unwrap_or("").to_string(),
                        name: cb["name"].as_str().unwrap_or("").to_string(),
                        text: cb["text"].as_str().unwrap_or("").to_string(),
                        data: cb["data"].as_str().unwrap_or("").to_string(),
                        ..Default::default()
                    },
                );
            }
            "content_block_delta" => {
                let d = &v["delta"];
                let b = self.blocks.entry(index).or_default();
                match d["type"].as_str().unwrap_or("") {
                    "text_delta" => {
                        let t = d["text"].as_str().unwrap_or("");
                        b.text.push_str(t);
                        if !t.is_empty() {
                            out.push(ChatEvent::Token(t.to_string()));
                        }
                    }
                    "input_json_delta" => b.json.push_str(d["partial_json"].as_str().unwrap_or("")),
                    "thinking_delta" => b.text.push_str(d["thinking"].as_str().unwrap_or("")),
                    "signature_delta" => {
                        b.signature.push_str(d["signature"].as_str().unwrap_or(""))
                    }
                    _ => {}
                }
            }
            "content_block_stop" => {
                if let Some(b) = self.blocks.get(&index) {
                    if b.kind == "tool_use" {
                        out.push(ChatEvent::ToolCall(ToolCall {
                            id: b.id.clone(),
                            name: b.name.clone(),
                            arguments: parse_args(&b.json),
                        }));
                    }
                }
            }
            "message_delta" => {
                if let Some(s) = v["delta"]["stop_reason"].as_str() {
                    self.stop = Some(s.to_string());
                }
            }
            "message_stop" => {
                out.push(ChatEvent::Done {
                    stop: map_stop(self.stop.as_deref()),
                    raw: Some(self.raw_content()),
                });
                return Ok((out, true));
            }
            "error" => {
                let msg = v["error"]["message"].as_str().unwrap_or("unknown error");
                let ty = v["error"]["type"].as_str().unwrap_or("error");
                return Err(AppError::Unavailable(format!("Anthropic {ty}: {msg}")));
            }
            _ => {} // message_start, ping, unknown future events
        }
        Ok((out, false))
    }

    /// Assistant content blocks for verbatim replay.
    fn raw_content(&self) -> Value {
        Value::Array(
            self.blocks
                .values()
                .filter_map(|b| match b.kind.as_str() {
                    "text" if !b.text.is_empty() => Some(json!({ "type": "text", "text": b.text })),
                    "tool_use" => Some(json!({
                        "type": "tool_use", "id": b.id, "name": b.name,
                        // Replay only a well-formed object; invalid input is reported
                        // to the model through the tool_result instead.
                        "input": serde_json::from_str::<Value>(&b.json).ok().filter(Value::is_object).unwrap_or_else(|| json!({})),
                    })),
                    "thinking" => Some(json!({ "type": "thinking", "thinking": b.text, "signature": b.signature })),
                    "redacted_thinking" => Some(json!({ "type": "redacted_thinking", "data": b.data })),
                    _ => None,
                })
                .collect(),
        )
    }
}

fn map_stop(s: Option<&str>) -> StopReason {
    match s {
        Some("end_turn") | Some("stop_sequence") | None => StopReason::EndTurn,
        Some("tool_use") => StopReason::ToolUse,
        Some("max_tokens") => StopReason::MaxTokens,
        Some("refusal") => StopReason::Refusal,
        Some(other) => StopReason::Other(other.to_string()),
    }
}

async fn error_from(resp: reqwest::Response) -> AppError {
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    let msg = body["error"]["message"]
        .as_str()
        .unwrap_or("request failed");
    match status.as_u16() {
        401 | 403 => AppError::Secret(format!("Anthropic rejected the API key ({status}): {msg}")),
        429 => AppError::Unavailable(format!("Anthropic rate limit ({status}): {msg}")),
        _ => AppError::Unavailable(format!("Anthropic returned {status}: {msg}")),
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn id(&self) -> &'static str {
        "anthropic"
    }

    async fn list_models(&self) -> AppResult<Vec<String>> {
        let mut ids = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let mut req = self
                .request(reqwest::Method::GET, "/models")
                .query(&[("limit", "1000")])
                .timeout(Duration::from_secs(15));
            if let Some(a) = &after {
                req = req.query(&[("after_id", a.as_str())]);
            }
            let resp = req.send().await?;
            if !resp.status().is_success() {
                return Err(error_from(resp).await);
            }
            let page: Value = resp.json().await?;
            if let Some(data) = page["data"].as_array() {
                ids.extend(
                    data.iter()
                        .filter_map(|m| m["id"].as_str().map(str::to_string)),
                );
            }
            match (page["has_more"].as_bool(), page["last_id"].as_str()) {
                (Some(true), Some(last)) => after = Some(last.to_string()),
                _ => break,
            }
        }
        Ok(ids)
    }

    async fn health_check(&self) -> AppResult<String> {
        let models = self.list_models().await?;
        Ok(format!(
            "Connected to Anthropic ({} models available)",
            models.len()
        ))
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSpec],
        opts: &ChatOptions,
    ) -> AppResult<ChatStream> {
        let (system, msgs) = to_anthropic(messages);
        let mut body = json!({
            "model": opts.model,
            "max_tokens": opts.max_tokens,
            "stream": true,
            "messages": msgs,
        });
        if !system.is_empty() {
            body["system"] = Value::String(system);
        }
        if !tools.is_empty() {
            body["tools"] = tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters,
                        "eager_input_streaming": true,
                    })
                })
                .collect();
        }
        let resp = self
            .request(reqwest::Method::POST, "/messages")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Unavailable(format!("Anthropic is not reachable: {e}")))?;
        if !resp.status().is_success() {
            return Err(error_from(resp).await);
        }

        let (tx, rx) = tokio::sync::mpsc::channel(64);
        tokio::spawn(async move {
            let mut bytes = resp.bytes_stream();
            let mut sse = SseParser::default();
            let mut state = AnthropicStreamState::default();
            while let Some(chunk) = bytes.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(Err(AppError::Http(e))).await;
                        return;
                    }
                };
                for ev in sse.push(&chunk) {
                    match state.handle(&ev.event, &ev.data) {
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
                .send(Err(AppError::Unavailable(
                    "Anthropic stream ended unexpectedly".into(),
                )))
                .await;
        });
        Ok(receiver_stream(rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::provider::INVALID_ARGS;

    fn run(events: &[(&str, &str)]) -> Vec<ChatEvent> {
        let mut s = AnthropicStreamState::default();
        let mut out = Vec::new();
        for (e, d) in events {
            out.extend(s.handle(e, d).expect("event").0);
        }
        out
    }

    #[test]
    fn streams_text_and_tool_use_with_thinking_replay() {
        let out = run(&[
            (
                "message_start",
                r#"{"type":"message_start","message":{"id":"msg_1"}}"#,
            ),
            (
                "content_block_start",
                r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}"#,
            ),
            (
                "content_block_delta",
                r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hmm"}}"#,
            ),
            (
                "content_block_delta",
                r#"{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"sig"}}"#,
            ),
            (
                "content_block_stop",
                r#"{"type":"content_block_stop","index":0}"#,
            ),
            (
                "content_block_start",
                r#"{"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}"#,
            ),
            (
                "content_block_delta",
                r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Checking."}}"#,
            ),
            (
                "content_block_stop",
                r#"{"type":"content_block_stop","index":1}"#,
            ),
            (
                "content_block_start",
                r#"{"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"toolu_1","name":"list_directory","input":{}}}"#,
            ),
            (
                "content_block_delta",
                r#"{"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"path\": \"/tm"}}"#,
            ),
            (
                "content_block_delta",
                r#"{"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"p\"}"}}"#,
            ),
            (
                "content_block_stop",
                r#"{"type":"content_block_stop","index":2}"#,
            ),
            ("ping", r#"{"type":"ping"}"#),
            (
                "message_delta",
                r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":9}}"#,
            ),
            ("message_stop", r#"{"type":"message_stop"}"#),
        ]);
        assert_eq!(out[0], ChatEvent::Token("Checking.".into()));
        match &out[1] {
            ChatEvent::ToolCall(c) => {
                assert_eq!(c.id, "toolu_1");
                assert_eq!(c.arguments["path"], "/tmp");
            }
            o => panic!("unexpected {o:?}"),
        }
        match &out[2] {
            ChatEvent::Done {
                stop,
                raw: Some(raw),
            } => {
                assert_eq!(*stop, StopReason::ToolUse);
                assert_eq!(raw[0]["type"], "thinking");
                assert_eq!(raw[0]["signature"], "sig");
                assert_eq!(raw[2]["input"]["path"], "/tmp");
            }
            o => panic!("unexpected {o:?}"),
        }
    }

    #[test]
    fn invalid_tool_json_is_flagged_and_refusal_mapped() {
        let out = run(&[
            (
                "content_block_start",
                r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t","name":"run_command","input":{}}}"#,
            ),
            (
                "content_block_delta",
                r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"command\": \"ls"}}"#,
            ),
            (
                "content_block_stop",
                r#"{"type":"content_block_stop","index":0}"#,
            ),
            (
                "message_delta",
                r#"{"type":"message_delta","delta":{"stop_reason":"max_tokens"}}"#,
            ),
            ("message_stop", r#"{"type":"message_stop"}"#),
        ]);
        match &out[0] {
            ChatEvent::ToolCall(c) => assert_eq!(c.arguments[INVALID_ARGS], true),
            o => panic!("unexpected {o:?}"),
        }
        assert!(matches!(
            &out[1],
            ChatEvent::Done {
                stop: StopReason::MaxTokens,
                ..
            }
        ));
        assert_eq!(map_stop(Some("refusal")), StopReason::Refusal);
    }

    #[test]
    fn error_event_is_an_error() {
        let mut s = AnthropicStreamState::default();
        assert!(s
            .handle(
                "error",
                r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#
            )
            .is_err());
    }

    #[test]
    fn history_merges_parallel_tool_results_and_extracts_system() {
        let c1 = ToolCall {
            id: "a".into(),
            name: "x".into(),
            arguments: json!({}),
        };
        let c2 = ToolCall {
            id: "b".into(),
            name: "y".into(),
            arguments: json!({}),
        };
        let mut asst = ChatMessage::text(Role::Assistant, "");
        asst.tool_calls = vec![c1.clone(), c2.clone()];
        let (system, msgs) = to_anthropic(&[
            ChatMessage::text(Role::System, "rules"),
            ChatMessage::text(Role::User, "hi"),
            asst,
            ChatMessage::tool_result(&c1, "r1".into(), false),
            ChatMessage::tool_result(&c2, "r2".into(), true),
        ]);
        assert_eq!(system, "rules");
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[1]["content"][0]["type"], "tool_use");
        let results = msgs[2]["content"].as_array().expect("array");
        assert_eq!(results.len(), 2);
        assert_eq!(results[1]["is_error"], true);
    }
}
