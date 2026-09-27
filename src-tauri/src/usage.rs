//! Local usage ledger: what OMNIX did for you, per day, on this machine only.
//!
//! Each finished chat turn or side task adds to a daily total in
//! `<app_data_dir>/usage.json` (mode 600): turn counts, token counts, tool
//! calls, time spent generating, and which model answered. **No prompt, reply,
//! tool argument or file name is ever stored**, and the file is never sent
//! anywhere (there is no network code in this module). Settings → Usage shows
//! it so users can see their own return on investment, e.g. how many tokens
//! local models served that a cloud API would have billed.
//!
//! Writes are best effort: a failure is logged and never interrupts a turn.

use crate::ai::metrics::TurnRecord;
use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Days of history kept.
const KEEP_DAYS: usize = 400;
/// Longest window a summary covers.
const MAX_WINDOW: u32 = 366;

/// Totals for one day (or a window of days).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Totals {
    /// Finished turns (chat and side tasks).
    pub turns: u64,
    /// Turns answered by a local model.
    pub local_turns: u64,
    /// Turns answered by a cloud provider.
    pub cloud_turns: u64,
    /// Turns that ended in an error.
    pub errors: u64,
    /// Turns the user cancelled.
    pub cancelled: u64,
    /// Prompt tokens (as reported by the backend).
    pub prompt_tokens: u64,
    /// Output tokens (reported or estimated).
    pub output_tokens: u64,
    /// Prompt tokens served locally.
    pub local_prompt_tokens: u64,
    /// Output tokens served locally.
    pub local_output_tokens: u64,
    /// Tool calls made by the agent.
    pub tool_calls: u64,
    /// Memories recalled into context.
    pub recalled: u64,
    /// Wall-clock time spent in turns, ms.
    pub active_ms: u64,
    /// Turns per model id.
    pub models: BTreeMap<String, u64>,
}

impl Totals {
    fn add_turn(&mut self, r: &TurnRecord) {
        self.turns += 1;
        if r.local {
            self.local_turns += 1;
            self.local_prompt_tokens += r.prompt_tokens;
            self.local_output_tokens += r.output_tokens;
        } else {
            self.cloud_turns += 1;
        }
        match r.outcome.as_str() {
            "error" => self.errors += 1,
            "cancelled" => self.cancelled += 1,
            _ => {}
        }
        self.prompt_tokens += r.prompt_tokens;
        self.output_tokens += r.output_tokens;
        self.tool_calls += u64::from(r.tools);
        self.recalled += u64::from(r.recalled);
        self.active_ms += r.duration_ms;
        *self.models.entry(r.model.clone()).or_default() += 1;
    }

    fn merge(&mut self, o: &Totals) {
        self.turns += o.turns;
        self.local_turns += o.local_turns;
        self.cloud_turns += o.cloud_turns;
        self.errors += o.errors;
        self.cancelled += o.cancelled;
        self.prompt_tokens += o.prompt_tokens;
        self.output_tokens += o.output_tokens;
        self.local_prompt_tokens += o.local_prompt_tokens;
        self.local_output_tokens += o.local_output_tokens;
        self.tool_calls += o.tool_calls;
        self.recalled += o.recalled;
        self.active_ms += o.active_ms;
        for (m, n) in &o.models {
            *self.models.entry(m.clone()).or_default() += n;
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Ledger {
    version: u32,
    /// `YYYY-MM-DD` (local date) → totals.
    days: BTreeMap<String, Totals>,
}

/// One day in a summary.
#[derive(Debug, Clone, Serialize)]
pub struct DayEntry {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// That day's totals.
    #[serde(flatten)]
    pub totals: Totals,
}

/// What Settings → Usage shows.
#[derive(Debug, Clone, Serialize)]
pub struct UsageSummary {
    /// Days with activity inside the window, oldest first.
    pub days: Vec<DayEntry>,
    /// Sum over the window.
    pub window: Totals,
    /// Sum over everything recorded.
    pub all_time: Totals,
    /// First recorded day.
    pub since: Option<String>,
    /// Window length in days.
    pub window_days: u32,
}

/// The ledger file plus a lock serializing read-modify-write.
pub struct UsageLedger {
    path: PathBuf,
    lock: Mutex<()>,
}

impl UsageLedger {
    /// Ledger stored at `path` (created on first write).
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    /// Where the ledger lives.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read(&self) -> Ledger {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Add one finished turn to today's totals. Never fails the caller.
    pub fn record(&self, rec: &TurnRecord) {
        let today = chrono::Local::now().date_naive().to_string();
        if let Err(e) = self.record_on(&today, rec) {
            tracing::warn!(error = %e, "usage ledger not updated");
        }
    }

    fn record_on(&self, day: &str, rec: &TurnRecord) -> AppResult<()> {
        let _g = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut l = self.read();
        l.version = 1;
        l.days.entry(day.to_string()).or_default().add_turn(rec);
        while l.days.len() > KEEP_DAYS {
            let Some(oldest) = l.days.keys().next().cloned() else {
                break;
            };
            l.days.remove(&oldest);
        }
        // write_atomic creates the file with mode 600.
        crate::settings::write_atomic(&self.path, &serde_json::to_string(&l)?)
    }

    /// Summary of the last `window_days` days ending `today`.
    pub fn summary_on(&self, today: chrono::NaiveDate, window_days: u32) -> UsageSummary {
        let window_days = window_days.clamp(1, MAX_WINDOW);
        let from = (today - chrono::Days::new(u64::from(window_days) - 1)).to_string();
        let l = {
            let _g = self.lock.lock().unwrap_or_else(|p| p.into_inner());
            self.read()
        };
        let mut window = Totals::default();
        let mut all_time = Totals::default();
        let mut days = Vec::new();
        for (d, t) in &l.days {
            all_time.merge(t);
            if d.as_str() >= from.as_str() && d.as_str() <= today.to_string().as_str() {
                window.merge(t);
                days.push(DayEntry {
                    date: d.clone(),
                    totals: t.clone(),
                });
            }
        }
        UsageSummary {
            days,
            window,
            all_time,
            since: l.days.keys().next().cloned(),
            window_days,
        }
    }

    /// Summary ending today.
    pub fn summary(&self, window_days: u32) -> UsageSummary {
        self.summary_on(chrono::Local::now().date_naive(), window_days)
    }

    /// Delete all recorded usage.
    pub fn clear(&self) -> AppResult<()> {
        let _g = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(model: &str, local: bool, outcome: &str) -> TurnRecord {
        TurnRecord {
            ts: "2026-09-26T12:00:00Z".into(),
            model: model.into(),
            duration_ms: 1500,
            ttft_ms: Some(200),
            output_tokens: 100,
            prompt_tokens: 400,
            local,
            tools: 2,
            recalled: 1,
            outcome: outcome.into(),
        }
    }

    fn day(s: &str) -> chrono::NaiveDate {
        s.parse().expect("date")
    }

    #[test]
    fn records_and_summarizes_by_day() {
        let d = tempfile::tempdir().expect("tempdir");
        let l = UsageLedger::new(d.path().join("usage.json"));
        l.record_on("2026-09-01", &rec("qwen3:8b", true, "ok"))
            .expect("rec");
        l.record_on("2026-09-25", &rec("qwen3:8b", true, "ok"))
            .expect("rec");
        l.record_on("2026-09-26", &rec("anthropic:x", false, "error"))
            .expect("rec");
        let s = l.summary_on(day("2026-09-26"), 7);
        assert_eq!(s.days.len(), 2);
        assert_eq!(s.window.turns, 2);
        assert_eq!(s.window.local_turns, 1);
        assert_eq!(s.window.cloud_turns, 1);
        assert_eq!(s.window.errors, 1);
        assert_eq!(s.window.local_output_tokens, 100);
        assert_eq!(s.window.tool_calls, 4);
        assert_eq!(s.all_time.turns, 3);
        assert_eq!(s.all_time.models.get("qwen3:8b"), Some(&2));
        assert_eq!(s.since.as_deref(), Some("2026-09-01"));
    }

    #[test]
    fn stores_no_content_and_is_private() {
        let d = tempfile::tempdir().expect("tempdir");
        let p = d.path().join("usage.json");
        let l = UsageLedger::new(p.clone());
        l.record_on("2026-09-26", &rec("qwen3:8b", true, "ok"))
            .expect("rec");
        let raw = std::fs::read_to_string(&p).expect("read");
        // Only counters and the model id: no timestamps finer than a day.
        assert!(!raw.contains("12:00"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&p).expect("meta").permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn keeps_a_bounded_history_and_clears() {
        let d = tempfile::tempdir().expect("tempdir");
        let l = UsageLedger::new(d.path().join("usage.json"));
        let start = day("2025-01-01");
        for i in 0..(KEEP_DAYS as u64 + 5) {
            let dd = (start + chrono::Days::new(i)).to_string();
            l.record_on(&dd, &rec("m", true, "ok")).expect("rec");
        }
        let s = l.summary_on(day("2030-01-01"), 1);
        assert_eq!(s.all_time.turns, KEEP_DAYS as u64);
        assert_eq!(s.since.as_deref(), Some("2025-01-06"));
        l.clear().expect("clear");
        assert_eq!(l.summary_on(day("2030-01-01"), 30).all_time.turns, 0);
        l.clear().expect("clear twice");
    }

    #[test]
    fn corrupt_file_reads_as_empty() {
        let d = tempfile::tempdir().expect("tempdir");
        let p = d.path().join("usage.json");
        std::fs::write(&p, "{not json").expect("write");
        let l = UsageLedger::new(p);
        assert_eq!(l.summary_on(day("2026-01-01"), 30).all_time.turns, 0);
        l.record_on("2026-01-01", &rec("m", true, "ok"))
            .expect("rec");
        assert_eq!(l.summary_on(day("2026-01-01"), 30).all_time.turns, 1);
    }
}
