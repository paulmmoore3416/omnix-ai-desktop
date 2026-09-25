//! Incremental parsers for streamed HTTP bodies: newline-delimited JSON
//! (Ollama) and Server-Sent Events (Anthropic, OpenAI-compatible), plus a
//! helper that turns an mpsc receiver into a [`ChatStream`].

use crate::ai::provider::{ChatEvent, ChatStream};
use crate::error::AppResult;
use futures_util::StreamExt;

/// Splits a byte stream into complete lines, buffering partial ones.
#[derive(Debug, Default)]
pub struct LineBuffer {
    buf: Vec<u8>,
}

impl LineBuffer {
    /// Feed bytes; returns every complete line (without `\n` / `\r\n`).
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|b| *b == b'\n') {
            let mut line: Vec<u8> = self.buf.drain(..=pos).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            out.push(String::from_utf8_lossy(&line).into_owned());
        }
        out
    }

    /// Whatever remains after the stream ends (a final unterminated line).
    pub fn finish(&mut self) -> Option<String> {
        let rest = std::mem::take(&mut self.buf);
        let s = String::from_utf8_lossy(&rest).trim().to_string();
        (!s.is_empty()).then_some(s)
    }
}

/// One Server-Sent Event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// `event:` field (empty when absent).
    pub event: String,
    /// Concatenated `data:` lines.
    pub data: String,
}

/// Incremental SSE parser (WHATWG event-stream subset: `event`, `data`,
/// comments; `id`/`retry` are ignored).
#[derive(Debug, Default)]
pub struct SseParser {
    lines: LineBuffer,
    event: String,
    data: Vec<String>,
}

impl SseParser {
    /// Feed bytes; returns every event completed by a blank line.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<SseEvent> {
        let mut out = Vec::new();
        for line in self.lines.push(bytes) {
            if line.is_empty() {
                if !self.data.is_empty() || !self.event.is_empty() {
                    out.push(SseEvent {
                        event: std::mem::take(&mut self.event),
                        data: std::mem::take(&mut self.data).join("\n"),
                    });
                }
            } else if line.starts_with(':') {
                // comment / keep-alive
            } else {
                let (field, value) = line.split_once(':').unwrap_or((line.as_str(), ""));
                let value = value.strip_prefix(' ').unwrap_or(value);
                match field {
                    "event" => self.event = value.to_string(),
                    "data" => self.data.push(value.to_string()),
                    _ => {}
                }
            }
        }
        out
    }
}

/// Adapt an mpsc receiver into a boxed [`ChatStream`].
pub fn receiver_stream(rx: tokio::sync::mpsc::Receiver<AppResult<ChatEvent>>) -> ChatStream {
    futures_util::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|e| (e, rx)) })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_buffer_handles_split_chunks() {
        let mut b = LineBuffer::default();
        assert!(b.push(b"{\"a\":").is_empty());
        assert_eq!(b.push(b"1}\n{\"b\"").as_slice(), ["{\"a\":1}"]);
        assert_eq!(b.push(b":2}\r\n").as_slice(), ["{\"b\":2}"]);
        assert_eq!(b.push(b"tail"), Vec::<String>::new());
        assert_eq!(b.finish().as_deref(), Some("tail"));
    }

    #[test]
    fn sse_parses_events_across_chunks() {
        let mut p = SseParser::default();
        let mut ev = p.push(b"event: message_start\ndata: {\"x\":");
        assert!(ev.is_empty());
        ev = p.push(b"1}\n\n: ping\n\ndata: [DONE]\n\n");
        assert_eq!(
            ev,
            vec![
                SseEvent {
                    event: "message_start".into(),
                    data: "{\"x\":1}".into()
                },
                SseEvent {
                    event: String::new(),
                    data: "[DONE]".into()
                },
            ]
        );
    }

    #[test]
    fn sse_joins_multiline_data() {
        let mut p = SseParser::default();
        let ev = p.push(b"data: a\ndata: b\n\n");
        assert_eq!(ev[0].data, "a\nb");
    }
}
