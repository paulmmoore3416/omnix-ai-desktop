//! Provider-neutral LLM abstraction.
//!
//! Every backend (Ollama, Anthropic, OpenAI-compatible) implements
//! [`LlmProvider`]. The agent loop only speaks these types, so providers can
//! be swapped without touching tool execution or security logic.

use crate::error::AppResult;
use async_trait::async_trait;
use futures_util::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Conversation role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Operator instructions (system prompt).
    System,
    /// The human user.
    User,
    /// The model.
    Assistant,
    /// Result of a tool call (always untrusted data).
    Tool,
}

/// A tool invocation requested by the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Provider-assigned (or locally generated) call id.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Parsed JSON arguments. [`INVALID_ARGS`] marks unparseable input.
    pub arguments: Value,
}

/// Marker key placed in [`ToolCall::arguments`] when the model's argument
/// JSON could not be parsed; the tool is then not executed.
pub const INVALID_ARGS: &str = "__omnix_invalid_json__";

/// One message in the conversation history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Who wrote it.
    pub role: Role,
    /// Text content (for `Tool`, the wrapped tool output).
    pub content: String,
    /// Tool calls made by an assistant message.
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    /// For `Tool` messages: the call this answers.
    #[serde(default)]
    pub tool_call_id: Option<String>,
    /// For `Tool` messages: the tool's name.
    #[serde(default)]
    pub tool_name: Option<String>,
    /// For `Tool` messages: whether the tool failed.
    #[serde(default)]
    pub is_error: bool,
    /// Provider-native assistant content to replay verbatim on the next
    /// request to the *same* provider (Anthropic thinking blocks must be
    /// passed back unchanged alongside `tool_use`).
    #[serde(default)]
    pub provider_raw: Option<Value>,
}

impl ChatMessage {
    /// Plain text message.
    pub fn text(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: vec![],
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            provider_raw: None,
        }
    }

    /// Tool result message.
    pub fn tool_result(call: &ToolCall, content: String, is_error: bool) -> Self {
        Self {
            role: Role::Tool,
            content,
            tool_calls: vec![],
            tool_call_id: Some(call.id.clone()),
            tool_name: Some(call.name.clone()),
            is_error,
            provider_raw: None,
        }
    }
}

/// A tool the model may call.
#[derive(Debug, Clone, Serialize)]
pub struct ToolSpec {
    /// Tool name.
    pub name: String,
    /// What the tool does (shown to the model).
    pub description: String,
    /// JSON Schema of the arguments.
    pub parameters: Value,
}

/// Per-request generation options.
#[derive(Debug, Clone)]
pub struct ChatOptions {
    /// Model id (discovered at runtime, never hardcoded).
    pub model: String,
    /// Sampling temperature (ignored by providers whose models reject it).
    pub temperature: f32,
    /// Maximum tokens to generate.
    pub max_tokens: u32,
    /// Context window (tokens) for providers that accept it (Ollama `num_ctx`).
    pub context_window: u32,
}

/// Why generation stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Natural end of the answer.
    EndTurn,
    /// The model wants tool results.
    ToolUse,
    /// Output hit `max_tokens` (tool input may be truncated).
    MaxTokens,
    /// The provider's safety system declined the request.
    Refusal,
    /// Anything else, verbatim.
    Other(String),
}

/// Streaming output of [`LlmProvider::chat_stream`].
#[derive(Debug, Clone, PartialEq)]
pub enum ChatEvent {
    /// A fragment of assistant text.
    Token(String),
    /// A complete tool call.
    ToolCall(ToolCall),
    /// Provider-level notice for the user (e.g. tools unavailable for this model).
    Notice(String),
    /// Token counts and timings reported by the backend (Ollama reports them
    /// on its final line). Emitted before `Done` when available.
    Usage(Usage),
    /// Generation finished.
    Done {
        /// Why it stopped.
        stop: StopReason,
        /// Provider-native assistant content for replay (see [`ChatMessage::provider_raw`]).
        raw: Option<Value>,
    },
}

/// Backend-reported usage for one generation. Missing values are `None`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    /// Prompt tokens evaluated.
    pub prompt_tokens: Option<u64>,
    /// Tokens generated.
    pub output_tokens: Option<u64>,
    /// Time spent evaluating the prompt, ms.
    pub prompt_ms: Option<u64>,
    /// Time spent generating, ms.
    pub generation_ms: Option<u64>,
    /// Time spent loading the model, ms (cold start).
    pub load_ms: Option<u64>,
}

/// Stream type returned by providers.
pub type ChatStream = BoxStream<'static, AppResult<ChatEvent>>;

/// An LLM backend.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Stable provider id (`ollama`, `anthropic`, `openai`, ...).
    fn id(&self) -> &'static str;

    /// Models available to this account/server (runtime discovery).
    async fn list_models(&self) -> AppResult<Vec<String>>;

    /// Verify connectivity and credentials; returns a human summary.
    async fn health_check(&self) -> AppResult<String>;

    /// Start a streaming chat completion.
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSpec],
        opts: &ChatOptions,
    ) -> AppResult<ChatStream>;
}

/// Parse tool-argument JSON, returning an [`INVALID_ARGS`] marker instead of
/// failing (the agent reports it back to the model as an error).
pub fn parse_args(raw: &str) -> Value {
    if raw.trim().is_empty() {
        return Value::Object(Default::default());
    }
    match serde_json::from_str::<Value>(raw) {
        Ok(v @ Value::Object(_)) => v,
        _ => serde_json::json!({ INVALID_ARGS: true }),
    }
}
