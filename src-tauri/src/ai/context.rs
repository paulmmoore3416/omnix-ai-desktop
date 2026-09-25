//! Context-window-aware history truncation.
//!
//! Token counts are estimated (≈4 characters per token plus per-message
//! overhead): provider tokenizers differ and exact counting would require a
//! network round-trip. The estimate errs on the generous side.

use crate::ai::provider::{ChatMessage, Role};

/// Rough token estimate for one message.
pub fn estimate_tokens(m: &ChatMessage) -> usize {
    let tool_chars: usize = m
        .tool_calls
        .iter()
        .map(|c| c.name.len() + c.arguments.to_string().len())
        .sum();
    (m.content.len() + tool_chars) / 4 + 8
}

/// Return the newest suffix of `history` that fits in
/// `context_window - max_tokens - system_tokens`.
///
/// The result never starts with an orphaned tool result or with an assistant
/// tool call whose results were cut, because providers reject such
/// sequences: leading messages are dropped until the first `User` message.
pub fn truncate(
    history: &[ChatMessage],
    system_tokens: usize,
    context_window: u32,
    max_tokens: u32,
) -> Vec<ChatMessage> {
    let budget = (context_window as usize)
        .saturating_sub(max_tokens as usize)
        .saturating_sub(system_tokens)
        .max(256);
    let mut used = 0usize;
    let mut start = history.len();
    for (i, m) in history.iter().enumerate().rev() {
        let t = estimate_tokens(m);
        if used + t > budget && start < history.len() {
            break;
        }
        used += t;
        start = i;
    }
    let kept = &history[start..];
    let first_user = kept
        .iter()
        .position(|m| m.role == Role::User)
        .unwrap_or(kept.len());
    kept[first_user..].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::provider::ToolCall;

    fn user(n: usize) -> ChatMessage {
        ChatMessage::text(Role::User, "x".repeat(n))
    }

    #[test]
    fn keeps_everything_when_it_fits() {
        let h = vec![user(10), ChatMessage::text(Role::Assistant, "ok"), user(10)];
        assert_eq!(truncate(&h, 100, 8192, 1024).len(), 3);
    }

    #[test]
    fn drops_oldest_first() {
        let h = vec![
            user(40_000),
            ChatMessage::text(Role::Assistant, "a"),
            user(400),
        ];
        let t = truncate(&h, 0, 4096, 1024);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].content.len(), 400);
    }

    #[test]
    fn never_starts_with_orphaned_tool_messages() {
        let call = ToolCall {
            id: "1".into(),
            name: "n".into(),
            arguments: serde_json::json!({}),
        };
        let mut asst = ChatMessage::text(Role::Assistant, "");
        asst.tool_calls.push(call.clone());
        let h = vec![
            user(30_000),
            asst,
            ChatMessage::tool_result(&call, "r".repeat(100), false),
            ChatMessage::text(Role::Assistant, "done"),
            user(50),
        ];
        let t = truncate(&h, 0, 4096, 1024);
        assert_eq!(t.first().map(|m| m.role), Some(Role::User));
    }

    #[test]
    fn always_keeps_the_latest_message_even_if_huge() {
        let h = vec![user(100_000)];
        assert_eq!(truncate(&h, 0, 4096, 1024).len(), 1);
    }
}
