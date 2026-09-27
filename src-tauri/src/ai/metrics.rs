//! Agent and model metrics (in memory, since app start).
//!
//! Token counts and timings come from the backend when it reports them
//! (Ollama's final line: `eval_count`, `eval_duration`, …); otherwise output
//! tokens are estimated from characters (≈4 chars/token) and flagged
//! `estimated`. Nothing here stores prompt or reply text.

use crate::ai::provider::Usage;
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;
use std::time::Instant;

/// Recent turns kept for the timeline.
const RECENT: usize = 50;

#[derive(Debug, Default, Clone, Serialize)]
struct ToolStat {
    calls: u64,
    ok: u64,
    failed: u64,
    total_ms: u64,
    max_ms: u64,
}

#[derive(Debug, Default, Clone, Serialize)]
struct ModelStat {
    generations: u64,
    turns: u64,
    prompt_tokens: u64,
    output_tokens: u64,
    generation_ms: u64,
    prompt_ms: u64,
    load_ms: u64,
    cold_starts: u64,
    ttft_ms_total: u64,
    ttft_count: u64,
    estimated_tokens: bool,
}

/// One finished turn (no content).
#[derive(Debug, Clone, Serialize)]
pub struct TurnRecord {
    /// RFC 3339 end time.
    pub ts: String,
    /// Model id.
    pub model: String,
    /// Wall-clock duration, ms.
    pub duration_ms: u64,
    /// Time to first token, ms.
    pub ttft_ms: Option<u64>,
    /// Output tokens (reported or estimated).
    pub output_tokens: u64,
    /// Prompt tokens reported by the backend (0 when it reports none).
    pub prompt_tokens: u64,
    /// True when a local model (Ollama) answered.
    pub local: bool,
    /// Tool calls made.
    pub tools: u32,
    /// Memory entries auto-recalled.
    pub recalled: u32,
    /// `ok`, `error` or `cancelled`.
    pub outcome: String,
}

#[derive(Debug, Default)]
struct Inner {
    turns: u64,
    errors: u64,
    cancelled: u64,
    tools: BTreeMap<String, ToolStat>,
    models: BTreeMap<String, ModelStat>,
    recall_runs: u64,
    recall_hits: u64,
    recall_empty: u64,
    captures: u64,
    captured: u64,
    recent: VecDeque<TurnRecord>,
}

/// Thread-safe collector.
pub struct AgentMetrics {
    started: Instant,
    inner: Mutex<Inner>,
}

impl Default for AgentMetrics {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            inner: Mutex::new(Inner::default()),
        }
    }
}

/// Per-turn accumulator filled by the agent loop.
#[derive(Debug)]
pub struct TurnTrace {
    /// Model id.
    pub model: String,
    started: Instant,
    /// Time to first token.
    pub ttft_ms: Option<u64>,
    output_chars: u64,
    reported_tokens: Option<u64>,
    /// Tool calls.
    pub tools: u32,
    /// Recalled entries.
    pub recalled: u32,
    /// Prompt tokens reported across the turn's generations.
    prompt_tokens: u64,
    /// True when the model runs locally (for the usage ledger).
    pub local: bool,
}

impl TurnTrace {
    /// Start timing a turn.
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
            started: Instant::now(),
            ttft_ms: None,
            output_chars: 0,
            reported_tokens: None,
            tools: 0,
            recalled: 0,
            prompt_tokens: 0,
            local: true,
        }
    }

    /// Mark whether a local model serves this turn (default: local).
    pub fn with_local(mut self, local: bool) -> Self {
        self.local = local;
        self
    }

    /// A text fragment arrived.
    pub fn token(&mut self, text: &str) {
        if self.ttft_ms.is_none() && !text.is_empty() {
            self.ttft_ms = Some(self.started.elapsed().as_millis() as u64);
        }
        self.output_chars += text.chars().count() as u64;
    }

    fn output_tokens(&self) -> (u64, bool) {
        match self.reported_tokens {
            Some(t) => (t, false),
            None => (self.output_chars.div_ceil(4), true),
        }
    }
}

fn lock(m: &Mutex<Inner>) -> std::sync::MutexGuard<'_, Inner> {
    // Metrics must never take the app down: recover from poisoning.
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl AgentMetrics {
    /// Backend-reported usage for one generation within `trace`'s turn.
    pub fn usage(&self, trace: &mut TurnTrace, u: &Usage) {
        let mut g = lock(&self.inner);
        let m = g.models.entry(trace.model.clone()).or_default();
        m.generations += 1;
        m.prompt_tokens += u.prompt_tokens.unwrap_or(0);
        m.output_tokens += u.output_tokens.unwrap_or(0);
        m.generation_ms += u.generation_ms.unwrap_or(0);
        m.prompt_ms += u.prompt_ms.unwrap_or(0);
        if let Some(l) = u.load_ms {
            m.load_ms += l;
            // Ollama reports a few ms of load time on warm models.
            if l > 500 {
                m.cold_starts += 1;
            }
        }
        trace.prompt_tokens += u.prompt_tokens.unwrap_or(0);
        if let Some(t) = u.output_tokens {
            trace.reported_tokens = Some(trace.reported_tokens.unwrap_or(0) + t);
        }
    }

    /// A tool finished.
    pub fn tool(&self, name: &str, ok: bool, ms: u64) {
        let mut g = lock(&self.inner);
        let t = g.tools.entry(name.to_string()).or_default();
        t.calls += 1;
        if ok {
            t.ok += 1;
        } else {
            t.failed += 1;
        }
        t.total_ms += ms;
        t.max_ms = t.max_ms.max(ms);
    }

    /// Auto-recall ran and returned `hits` entries.
    pub fn recall(&self, hits: usize) {
        let mut g = lock(&self.inner);
        g.recall_runs += 1;
        g.recall_hits += hits as u64;
        if hits == 0 {
            g.recall_empty += 1;
        }
    }

    /// Fact capture ran and stored `saved` new memories.
    pub fn capture(&self, saved: usize) {
        let mut g = lock(&self.inner);
        g.captures += 1;
        g.captured += saved as u64;
    }

    /// The turn ended (`outcome`: `ok`, `error`, `cancelled`). Returns the
    /// content-free record (also fed to the persistent usage ledger).
    pub fn finish(&self, trace: TurnTrace, outcome: &str) -> TurnRecord {
        let (tokens, estimated) = trace.output_tokens();
        let duration_ms = trace.started.elapsed().as_millis() as u64;
        let mut g = lock(&self.inner);
        g.turns += 1;
        match outcome {
            "error" => g.errors += 1,
            "cancelled" => g.cancelled += 1,
            _ => {}
        }
        let m = g.models.entry(trace.model.clone()).or_default();
        m.turns += 1;
        if let Some(t) = trace.ttft_ms {
            m.ttft_ms_total += t;
            m.ttft_count += 1;
        }
        if estimated {
            m.output_tokens += tokens;
            m.estimated_tokens = true;
        }
        if g.recent.len() == RECENT {
            g.recent.pop_front();
        }
        let rec = TurnRecord {
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            model: trace.model,
            duration_ms,
            ttft_ms: trace.ttft_ms,
            output_tokens: tokens,
            prompt_tokens: trace.prompt_tokens,
            local: trace.local,
            tools: trace.tools,
            recalled: trace.recalled,
            outcome: outcome.to_string(),
        };
        g.recent.push_back(rec.clone());
        rec
    }

    /// JSON snapshot with derived rates.
    pub fn snapshot(&self) -> serde_json::Value {
        let g = lock(&self.inner);
        let models: Vec<serde_json::Value> = g
            .models
            .iter()
            .map(|(name, m)| {
                serde_json::json!({
                    "model": name,
                    "turns": m.turns,
                    "generations": m.generations,
                    "prompt_tokens": m.prompt_tokens,
                    "output_tokens": m.output_tokens,
                    "tokens_estimated": m.estimated_tokens,
                    "tokens_per_sec": (m.generation_ms > 0)
                        .then(|| (m.output_tokens as f64) / (m.generation_ms as f64) * 1000.0),
                    "prompt_tokens_per_sec": (m.prompt_ms > 0)
                        .then(|| (m.prompt_tokens as f64) / (m.prompt_ms as f64) * 1000.0),
                    "avg_ttft_ms": (m.ttft_count > 0).then(|| m.ttft_ms_total / m.ttft_count),
                    "cold_starts": m.cold_starts,
                    "load_ms": m.load_ms,
                })
            })
            .collect();
        let tools: Vec<serde_json::Value> = g
            .tools
            .iter()
            .map(|(name, t)| {
                serde_json::json!({
                    "tool": name, "calls": t.calls, "ok": t.ok, "failed": t.failed,
                    "avg_ms": (t.calls > 0).then(|| t.total_ms / t.calls), "max_ms": t.max_ms,
                })
            })
            .collect();
        serde_json::json!({
            "uptime_s": self.started.elapsed().as_secs(),
            "turns": g.turns,
            "errors": g.errors,
            "cancelled": g.cancelled,
            "models": models,
            "tools": tools,
            "recall": { "runs": g.recall_runs, "hits": g.recall_hits, "empty": g.recall_empty },
            "capture": { "runs": g.captures, "saved": g.captured },
            "recent": g.recent.iter().rev().collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_reported_and_estimated_tokens() {
        let m = AgentMetrics::default();
        let mut t = TurnTrace::new("qwen3:8b");
        t.token("Hello");
        assert!(t.ttft_ms.is_some());
        m.usage(
            &mut t,
            &Usage {
                prompt_tokens: Some(100),
                output_tokens: Some(50),
                prompt_ms: Some(200),
                generation_ms: Some(1000),
                load_ms: Some(3000),
            },
        );
        m.tool("run_command", true, 40);
        m.tool("run_command", false, 60);
        m.recall(3);
        m.finish(t, "ok");

        let mut t2 = TurnTrace::new("other");
        t2.token("abcdefgh"); // 8 chars ≈ 2 tokens
        m.finish(t2, "error");

        let s = m.snapshot();
        assert_eq!(s["turns"], 2);
        assert_eq!(s["errors"], 1);
        let q = &s["models"].as_array().expect("models")[1];
        assert_eq!(q["model"], "qwen3:8b");
        assert_eq!(q["tokens_per_sec"], 50.0);
        assert_eq!(q["cold_starts"], 1);
        let o = &s["models"].as_array().expect("models")[0];
        assert_eq!(o["output_tokens"], 2);
        assert_eq!(o["tokens_estimated"], true);
        assert_eq!(s["tools"][0]["avg_ms"], 50);
        assert_eq!(s["recall"]["hits"], 3);
        assert_eq!(s["recent"][0]["outcome"], "error");
    }
}
