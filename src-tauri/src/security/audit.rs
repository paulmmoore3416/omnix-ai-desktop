//! Append-only, hash-chained JSONL audit log.
//!
//! Every security-relevant action (command execution, file access, process
//! kills, security-setting changes) is recorded as one JSON object per line in
//! `<app_log_dir>/audit.jsonl`.
//!
//! **Hash chain.** Each entry stores `prev_hash`, the lowercase hex SHA-256 of
//! the previous line's exact bytes (without the trailing newline). The first
//! entry uses 64 zeros. [`verify_file`] recomputes the chain, so editing or
//! deleting any line except the last one is detected. Truncating the tail is
//! *not* detectable from the file alone. Shipping lines to an external sink
//! (Phase 4 Loki option) anchors the chain elsewhere.
//!
//! **Redaction.** Commands and details are passed through [`redact`] before
//! they are written, so API keys and bearer tokens typed into a command never
//! reach the disk.

use crate::error::{AppError, AppResult};
use crate::security::policy::{RiskTier, Source};
use chrono::{SecondsFormat, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// `prev_hash` of the first entry in a new log.
pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Outcome of the policy + confirmation steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Allowed and executed (see `exit_code` for the result).
    Allowed,
    /// Refused by the policy engine.
    Denied,
    /// Refused because the user declined or the dialog timed out.
    NotApproved,
    /// Allowed but failed to start or timed out.
    Failed,
}

/// Result of the native confirmation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confirmation {
    /// No dialog was required for this tier.
    NotRequired,
    /// The user pressed the approve button.
    Approved,
    /// The user pressed cancel or closed the dialog.
    Declined,
    /// Nobody answered before the timeout (treated as deny).
    TimedOut,
    /// No dialog was shown because the request was denied first.
    Skipped,
}

/// One audit record. The hash chain covers the exact serialized line bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// UTC timestamp, RFC 3339 with milliseconds.
    pub ts: String,
    /// Request id (UUID v4) correlating dialog, execution and log.
    pub id: String,
    /// `user` or `llm_tool`.
    pub source: Source,
    /// Kind of action: `exec`, `read_file`, `write_file`, `list_directory`,
    /// `kill_process`, `settings_change`, ...
    pub action: String,
    /// The (redacted) command or a description of the target.
    pub command: String,
    /// Working directory, when relevant.
    pub cwd: Option<String>,
    /// Policy tier.
    pub tier: RiskTier,
    /// Final decision.
    pub decision: Decision,
    /// Confirmation dialog outcome.
    pub confirmation: Confirmation,
    /// Process exit code, when a process ran to completion.
    pub exit_code: Option<i32>,
    /// Wall-clock duration of the action.
    pub duration_ms: Option<u64>,
    /// Free-form (redacted) detail: denial reasons, errors, etc.
    pub detail: Option<String>,
    /// SHA-256 of the previous line (hex), or [`GENESIS_HASH`].
    pub prev_hash: String,
}

/// Builder-style input for [`AuditLog::record`] (everything except `ts`,
/// `prev_hash`, which the log fills in).
#[derive(Debug, Clone)]
pub struct AuditRecord {
    /// See [`AuditEntry::id`].
    pub id: String,
    /// See [`AuditEntry::source`].
    pub source: Source,
    /// See [`AuditEntry::action`].
    pub action: String,
    /// See [`AuditEntry::command`]. Redacted before writing.
    pub command: String,
    /// See [`AuditEntry::cwd`].
    pub cwd: Option<String>,
    /// See [`AuditEntry::tier`].
    pub tier: RiskTier,
    /// See [`AuditEntry::decision`].
    pub decision: Decision,
    /// See [`AuditEntry::confirmation`].
    pub confirmation: Confirmation,
    /// See [`AuditEntry::exit_code`].
    pub exit_code: Option<i32>,
    /// See [`AuditEntry::duration_ms`].
    pub duration_ms: Option<u64>,
    /// See [`AuditEntry::detail`]. Redacted before writing.
    pub detail: Option<String>,
}

/// Result of [`verify_file`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyReport {
    /// True when every line parsed and every `prev_hash` matched.
    pub valid: bool,
    /// Number of entries checked.
    pub entries: usize,
    /// 1-based line number of the first problem, if any.
    pub first_bad_line: Option<usize>,
    /// Description of the first problem, if any.
    pub reason: Option<String>,
}

/// Handle to the audit log file. Cheap to share behind `Arc`/managed state.
#[derive(Debug)]
pub struct AuditLog {
    path: PathBuf,
    /// Serializes appends and remembers the hash of the last line so every
    /// write does not have to re-read the file.
    last_hash: tokio::sync::Mutex<String>,
    /// Optional external sink (Loki shipper). Receives the exact, already
    /// redacted line after it is durably written locally.
    sink: std::sync::Mutex<Option<tokio::sync::mpsc::Sender<String>>>,
}

impl AuditLog {
    /// Open (or create) the log at `path`, recovering the chain head from the
    /// last line. Creates parent directories with restrictive permissions.
    pub fn open(path: PathBuf) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let last_hash = read_last_line(&path)?
            .map(|l| sha256_hex(l.as_bytes()))
            .unwrap_or_else(|| GENESIS_HASH.to_string());
        Ok(Self {
            path,
            last_hash: tokio::sync::Mutex::new(last_hash),
            sink: std::sync::Mutex::new(None),
        })
    }

    /// Path of the JSONL file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one entry. The write is flushed and fsync'd before returning so
    /// an action is never reported as done without its audit record.
    pub async fn record(&self, rec: AuditRecord) -> AppResult<()> {
        let mut last = self.last_hash.lock().await;
        let entry = AuditEntry {
            ts: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            id: rec.id,
            source: rec.source,
            action: rec.action,
            command: redact(&rec.command),
            cwd: rec.cwd,
            tier: rec.tier,
            decision: rec.decision,
            confirmation: rec.confirmation,
            exit_code: rec.exit_code,
            duration_ms: rec.duration_ms,
            detail: rec.detail.map(|d| redact(&d)),
            prev_hash: last.clone(),
        };
        let line = serde_json::to_string(&entry)?;
        let path = self.path.clone();
        let line_for_write = line.clone();
        // File I/O is blocking; keep it off the async executor threads.
        tokio::task::spawn_blocking(move || append_line(&path, &line_for_write))
            .await
            .map_err(|e| AppError::Internal(format!("audit writer task failed: {e}")))??;
        *last = sha256_hex(line.as_bytes());
        if let Ok(guard) = self.sink.lock() {
            if let Some(tx) = guard.as_ref() {
                // Never block auditing on the network: drop if the queue is full.
                if tx.try_send(line).is_err() {
                    tracing::warn!("audit shipping queue full or closed; line not shipped");
                }
            }
        }
        Ok(())
    }

    /// Install or remove the external sink.
    pub fn set_sink(&self, sink: Option<tokio::sync::mpsc::Sender<String>>) {
        if let Ok(mut guard) = self.sink.lock() {
            *guard = sink;
        }
    }
}

/// Append `line` + `\n`, creating the file with mode 0600 on Unix.
fn append_line(path: &Path, line: &str) -> AppResult<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    f.write_all(line.as_bytes())?;
    f.write_all(b"\n")?;
    f.sync_data()?;
    Ok(())
}

/// Read the last non-empty line of `path`, if the file exists. Reads only the
/// tail (entries are small because commands are length-capped).
fn read_last_line(path: &Path) -> AppResult<Option<String>> {
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let len = f.metadata()?.len();
    let start = len.saturating_sub(256 * 1024);
    f.seek(SeekFrom::Start(start))?;
    let mut buf = String::new();
    f.read_to_string(&mut buf)?;
    Ok(buf
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(str::to_string))
}

/// Recompute the hash chain of the log at `path`.
pub fn verify_file(path: &Path) -> AppResult<VerifyReport> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(VerifyReport {
                valid: true,
                entries: 0,
                first_bad_line: None,
                reason: None,
            })
        }
        Err(e) => return Err(e.into()),
    };
    Ok(verify_str(&content))
}

/// Verify a log given as a string (used by [`verify_file`] and tests).
pub fn verify_str(content: &str) -> VerifyReport {
    let mut expected = GENESIS_HASH.to_string();
    let mut entries = 0;
    for (idx, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let bad = |reason: String| VerifyReport {
            valid: false,
            entries,
            first_bad_line: Some(idx + 1),
            reason: Some(reason),
        };
        let entry: AuditEntry = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(e) => return bad(format!("unparseable entry: {e}")),
        };
        if entry.prev_hash != expected {
            return bad(
                "prev_hash does not match the previous line (entry modified, inserted or removed)"
                    .into(),
            );
        }
        expected = sha256_hex(line.as_bytes());
        entries += 1;
    }
    VerifyReport {
        valid: true,
        entries,
        first_bad_line: None,
        reason: None,
    }
}

/// Lowercase hex SHA-256.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Secret patterns replaced before anything is logged. Patterns are static
/// and covered by tests, so a compile failure is a programming error.
static REDACTIONS: LazyLock<Vec<(Regex, &'static str)>> =
    LazyLock::new(|| {
        [
        (r"sk-ant-[A-Za-z0-9_\-]{8,}", "sk-ant-[REDACTED]"),
        (r"sk-(?:proj-)?[A-Za-z0-9_\-]{16,}", "sk-[REDACTED]"),
        (r"xai-[A-Za-z0-9]{16,}", "xai-[REDACTED]"),
        (r"AIza[0-9A-Za-z_\-]{30,}", "AIza[REDACTED]"),
        (r"gh[pousr]_[A-Za-z0-9]{20,}", "gh_[REDACTED]"),
        (r"github_pat_[A-Za-z0-9_]{20,}", "github_pat_[REDACTED]"),
        (r"xox[abprs]-[A-Za-z0-9\-]{10,}", "xox-[REDACTED]"),
        (r"AKIA[0-9A-Z]{16}", "AKIA[REDACTED]"),
        (r"(?i)\bbearer\s+[A-Za-z0-9._~+/=\-]{8,}", "Bearer [REDACTED]"),
        (
            r#"(?i)\b(api[_-]?key|token|secret|password|passwd|authorization)(\s*[=:]\s*)[^\s'"]+"#,
            "${1}${2}[REDACTED]",
        ),
    ]
    .into_iter()
    .map(|(p, r)| (Regex::new(p).expect("static redaction regex must compile"), r))
    .collect()
    });

/// Replace API keys, tokens and `key=value` secrets with `[REDACTED]`.
pub fn redact(input: &str) -> String {
    REDACTIONS.iter().fold(input.to_string(), |acc, (re, rep)| {
        re.replace_all(&acc, *rep).into_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(cmd: &str) -> AuditRecord {
        AuditRecord {
            id: uuid::Uuid::new_v4().to_string(),
            source: Source::User,
            action: "exec".into(),
            command: cmd.into(),
            cwd: Some("/tmp".into()),
            tier: RiskTier::ReadOnly,
            decision: Decision::Allowed,
            confirmation: Confirmation::NotRequired,
            exit_code: Some(0),
            duration_ms: Some(3),
            detail: None,
        }
    }

    async fn log_with(n: usize) -> (tempfile::TempDir, AuditLog) {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = AuditLog::open(dir.path().join("audit.jsonl")).expect("open");
        for i in 0..n {
            log.record(rec(&format!("echo {i}"))).await.expect("record");
        }
        (dir, log)
    }

    #[tokio::test]
    async fn chain_verifies() {
        let (_d, log) = log_with(5).await;
        let r = verify_file(log.path()).expect("verify");
        assert!(r.valid, "{r:?}");
        assert_eq!(r.entries, 5);
    }

    #[tokio::test]
    async fn chain_resumes_after_reopen() {
        let (_d, log) = log_with(2).await;
        let path = log.path().to_path_buf();
        drop(log);
        let log2 = AuditLog::open(path.clone()).expect("reopen");
        log2.record(rec("after reopen")).await.expect("record");
        let r = verify_file(&path).expect("verify");
        assert!(r.valid);
        assert_eq!(r.entries, 3);
    }

    #[tokio::test]
    async fn tampered_line_is_detected() {
        let (_d, log) = log_with(4).await;
        let content = std::fs::read_to_string(log.path()).expect("read");
        let tampered = content.replacen("echo 1", "echo X", 1);
        let r = verify_str(&tampered);
        assert!(!r.valid);
        // Line 2 was edited; the mismatch surfaces on line 3.
        assert_eq!(r.first_bad_line, Some(3));
    }

    #[tokio::test]
    async fn deleted_line_is_detected() {
        let (_d, log) = log_with(4).await;
        let content = std::fs::read_to_string(log.path()).expect("read");
        let kept: Vec<&str> = content
            .lines()
            .enumerate()
            .filter(|(i, _)| *i != 1)
            .map(|(_, l)| l)
            .collect();
        let r = verify_str(&kept.join("\n"));
        assert!(!r.valid);
        assert_eq!(r.first_bad_line, Some(2));
    }

    #[tokio::test]
    async fn garbage_line_is_detected() {
        let r = verify_str("not json\n");
        assert!(!r.valid);
        assert_eq!(r.first_bad_line, Some(1));
    }

    #[test]
    fn empty_or_missing_log_is_valid() {
        let d = tempfile::tempdir().expect("tempdir");
        let r = verify_file(&d.path().join("missing.jsonl")).expect("verify");
        assert!(r.valid);
        assert_eq!(r.entries, 0);
    }

    #[tokio::test]
    async fn secrets_are_redacted_on_disk() {
        let (_d, log) = log_with(0).await;
        log.record(rec(
            "curl -H 'Authorization: Bearer abcdef123456789' https://api.anthropic.com -d key=sk-ant-api03-AAAAAAAAAAAAAAAA",
        ))
        .await
        .expect("record");
        let content = std::fs::read_to_string(log.path()).expect("read");
        assert!(!content.contains("abcdef123456789"));
        assert!(!content.contains("AAAAAAAAAAAAAAAA"));
        assert!(content.contains("[REDACTED]"));
    }

    #[test]
    fn redact_patterns() {
        let cases = [
            (
                "export OPENAI_API_KEY=sk-proj-abcdefghijklmnopqrstuvwxyz",
                "abcdefghijklmnop",
            ),
            (
                "git clone https://ghp_abcdefghijklmnopqrstuvwxyz0123@github.com/x",
                "ghp_abcdefghij",
            ),
            ("token: hunter2", "hunter2"),
            ("password=hunter2", "hunter2"),
            ("AKIAABCDEFGHIJKLMNOP", "ABCDEFGHIJKLMNOP"),
            ("xai-abcdefghijklmnopqrstuvwxyz", "abcdefghijklmnopqrst"),
        ];
        for (input, secret) in cases {
            let out = redact(input);
            assert!(!out.contains(secret), "{input} -> {out}");
        }
        assert_eq!(redact("ls -la"), "ls -la");
    }
}
