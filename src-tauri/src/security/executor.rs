//! The only path by which OMNIX runs an external command.
//!
//! ```text
//! request ─► resolve cwd ─► policy::classify ─► (Denied? → audit + error)
//!         ─► (Privileged && !enable_sudo? → audit + error)
//!         ─► native confirmation (Mutating/Privileged always; ReadOnly if configured)
//!         ─► spawn (tokio, cleared env, timeout, output cap, process group)
//!         ─► audit (always) ─► structured result
//! ```

use crate::error::{AppError, AppResult};
use crate::security::audit::{AuditRecord, Confirmation, Decision};
use crate::security::confirm::{self, ConfirmRequest};
use crate::security::elevation;
use crate::security::policy::{self, RiskTier, Source};
use crate::state::AppState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Runtime};
use tokio::io::{AsyncRead, AsyncReadExt};

/// Environment variables passed through to child processes. Everything else
/// (notably any `*_API_KEY` in the parent environment) is cleared so secrets
/// never leak into commands.
const SAFE_ENV: &[&str] = &["PATH", "HOME", "LANG", "TERM"];

/// Extra variables Windows programs need to function at all (`cmd.exe` cannot
/// locate system DLLs or resolve extensions without them). None carry secrets.
#[cfg(windows)]
const SAFE_ENV_WINDOWS: &[&str] = &[
    "SYSTEMROOT",
    "WINDIR",
    "COMSPEC",
    "PATHEXT",
    "USERPROFILE",
    "TEMP",
    "TMP",
];

/// Structured result of an executed command.
#[derive(Debug, Clone, Serialize)]
pub struct ExecResult {
    /// Request id (matches the audit log entry).
    pub request_id: String,
    /// Process exit code; `None` if terminated by a signal.
    pub exit_code: Option<i32>,
    /// Captured stdout (lossy UTF-8, capped).
    pub stdout: String,
    /// Captured stderr (lossy UTF-8, capped).
    pub stderr: String,
    /// True when either stream exceeded the cap and was truncated.
    pub truncated: bool,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Policy tier the command ran under.
    pub tier: RiskTier,
}

/// A request to run a command.
#[derive(Debug, Clone)]
pub struct ExecRequest {
    /// Command line exactly as typed / proposed.
    pub command: String,
    /// Optional working directory (defaults to the user's home).
    pub cwd: Option<String>,
    /// Who asked.
    pub source: Source,
}

/// Run `req` through the full policy pipeline.
pub async fn execute<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    req: ExecRequest,
) -> AppResult<ExecResult> {
    execute_inner(app, state, req, None).await
}

/// Unattended run for the scheduler/automations. The caller must already
/// have verified a signed approval for exactly this command and working
/// directory (`ops::approval::verify`); `label` names the rule for the audit
/// log. The policy is re-evaluated now, so a rule can never do more than the
/// current policy allows:
///
/// * `Denied` → refused (as always);
/// * `Privileged` → refused: elevation needs a person at the password prompt;
/// * `Mutating` (or anything, with `require_confirmation`) → runs, audited as
///   `confirmation: pre_approved` instead of opening a dialog nobody answers.
pub async fn execute_preapproved<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    req: ExecRequest,
    label: &str,
) -> AppResult<ExecResult> {
    execute_inner(app, state, req, Some(label)).await
}

async fn execute_inner<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    req: ExecRequest,
    preapproved: Option<&str>,
) -> AppResult<ExecResult> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let settings = state.settings.read().await.clone();
    let sec = &settings.security;

    let cwd = resolve_cwd(req.cwd.as_deref(), state.home.as_deref())?;
    let cwd_str = cwd.to_string_lossy().to_string();
    let cfg = state.policy_config(&settings, Some(&cwd));
    let class = policy::classify(&req.command, &cfg);

    let base = AuditRecord {
        id: request_id.clone(),
        source: req.source,
        action: "exec".into(),
        command: req.command.clone(),
        cwd: Some(cwd_str.clone()),
        tier: class.tier,
        decision: Decision::Denied,
        confirmation: Confirmation::Skipped,
        exit_code: None,
        duration_ms: None,
        detail: Some(match preapproved {
            Some(label) => format!("unattended ({label}); {}", class.reasons.join("; ")),
            None => class.reasons.join("; "),
        }),
    };

    // 1. Policy denial.
    if class.tier == RiskTier::Denied {
        state.audit.record(base).await?;
        return Err(AppError::PolicyDenied(class.reasons.join("; ")));
    }

    // 2. Elevation gate (checked before bothering the user with a dialog).
    if class.tier == RiskTier::Privileged && preapproved.is_some() {
        let msg =
            "privileged commands never run unattended (elevation needs you at the password prompt)";
        state
            .audit
            .record(AuditRecord {
                detail: Some(msg.into()),
                ..base
            })
            .await?;
        return Err(AppError::PolicyDenied(msg.into()));
    }
    let program_args = if class.tier == RiskTier::Privileged {
        if !sec.enable_sudo {
            let msg = "privileged commands are disabled (Settings → Security → Enable sudo)";
            state
                .audit
                .record(AuditRecord {
                    detail: Some(msg.into()),
                    ..base
                })
                .await?;
            return Err(AppError::PolicyDenied(msg.into()));
        }
        match elevation::elevated_command(&req.command) {
            Ok(pa) => pa,
            Err(e) => {
                state
                    .audit
                    .record(AuditRecord {
                        decision: Decision::Failed,
                        detail: Some(e.to_string()),
                        ..base
                    })
                    .await?;
                return Err(e);
            }
        }
    } else if let (false, Some(argv)) = (class.needs_shell, class.argv.as_ref()) {
        let (p, a) = argv
            .split_first()
            .ok_or_else(|| AppError::InvalidInput("empty command".into()))?;
        (p.clone(), a.to_vec())
    } else {
        shell_command(&req.command)
    };

    // 3. Native confirmation.
    let needs_confirm = class.tier >= RiskTier::Mutating || sec.require_confirmation;
    let confirmation = if needs_confirm && preapproved.is_some() {
        Confirmation::PreApproved
    } else if needs_confirm {
        let c = confirm::ask(
            app,
            &ConfirmRequest {
                title: match class.tier {
                    RiskTier::Privileged => "Run command as administrator?".into(),
                    _ => "Run command?".into(),
                },
                subject: format!("Command:\n{}", req.command),
                details: vec![format!("Working directory: {cwd_str}")],
                tier: class.tier,
                source: req.source,
                reasons: class.reasons.clone(),
                approve_label: "Run".into(),
            },
            Duration::from_secs(sec.confirmation_timeout_secs),
        )
        .await;
        if c != Confirmation::Approved {
            state
                .audit
                .record(AuditRecord {
                    decision: Decision::NotApproved,
                    confirmation: c,
                    ..base
                })
                .await?;
            return Err(AppError::NotApproved(match c {
                Confirmation::TimedOut => "confirmation timed out".into(),
                _ => "you declined to run this command".into(),
            }));
        }
        c
    } else {
        Confirmation::NotRequired
    };

    // 4. Execute.
    let started = Instant::now();
    let run = run_process(
        &program_args.0,
        &program_args.1,
        &cwd,
        Duration::from_secs(sec.command_timeout_secs),
        sec.max_output_bytes,
    )
    .await;
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    // 5. Audit (always) and return.
    match run {
        Ok(out) => {
            state
                .audit
                .record(AuditRecord {
                    decision: Decision::Allowed,
                    confirmation,
                    exit_code: out.exit_code,
                    duration_ms: Some(duration_ms),
                    detail: match (preapproved, out.truncated) {
                        (Some(l), true) => Some(format!("unattended ({l}); output truncated")),
                        (Some(l), false) => Some(format!("unattended ({l})")),
                        (None, true) => Some("output truncated".to_string()),
                        (None, false) => None,
                    },
                    ..base
                })
                .await?;
            Ok(ExecResult {
                request_id,
                exit_code: out.exit_code,
                stdout: out.stdout,
                stderr: out.stderr,
                truncated: out.truncated,
                duration_ms,
                tier: class.tier,
            })
        }
        Err(e) => {
            state
                .audit
                .record(AuditRecord {
                    decision: Decision::Failed,
                    confirmation,
                    duration_ms: Some(duration_ms),
                    detail: Some(e.to_string()),
                    ..base
                })
                .await?;
            Err(e)
        }
    }
}

/// Resolve and validate the working directory. Defaults to `home`.
pub fn resolve_cwd(cwd: Option<&str>, home: Option<&Path>) -> AppResult<PathBuf> {
    let raw = match cwd.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => expand_home(c, home),
        None => home
            .map(Path::to_path_buf)
            .ok_or_else(|| AppError::Unavailable("no home directory".into()))?,
    };
    if !raw.is_absolute() {
        return Err(AppError::InvalidInput(
            "working directory must be an absolute path".into(),
        ));
    }
    let canon = std::fs::canonicalize(&raw)
        .map_err(|e| AppError::InvalidInput(format!("working directory {}: {e}", raw.display())))?;
    if !canon.is_dir() {
        return Err(AppError::InvalidInput(format!(
            "working directory {} is not a directory",
            canon.display()
        )));
    }
    Ok(canon)
}

/// Expand a leading `~` using `home`.
pub fn expand_home(p: &str, home: Option<&Path>) -> PathBuf {
    match (p, home) {
        ("~", Some(h)) => h.to_path_buf(),
        (s, Some(h)) if s.starts_with("~/") => h.join(&s[2..]),
        (s, _) => PathBuf::from(s),
    }
}

/// `(program, args)` running `raw` through the platform shell.
fn shell_command(raw: &str) -> (String, Vec<String>) {
    if cfg!(windows) {
        ("cmd".into(), vec!["/C".into(), raw.to_string()])
    } else {
        ("/bin/sh".into(), vec!["-c".into(), raw.to_string()])
    }
}

/// Minimal child environment (see [`SAFE_ENV`]).
pub fn safe_env() -> Vec<(String, std::ffi::OsString)> {
    #[cfg(windows)]
    let names = SAFE_ENV.iter().chain(SAFE_ENV_WINDOWS.iter());
    #[cfg(not(windows))]
    let names = SAFE_ENV.iter();
    let mut v: Vec<(String, std::ffi::OsString)> = names
        .filter_map(|k| std::env::var_os(k).map(|val| ((*k).to_string(), val)))
        .collect();
    if !v.iter().any(|(k, _)| k == "PATH") && cfg!(unix) {
        v.push((
            "PATH".into(),
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".into(),
        ));
    }
    if !v.iter().any(|(k, _)| k == "TERM") {
        v.push(("TERM".into(), "dumb".into()));
    }
    v
}

/// Raw process output.
#[derive(Debug)]
pub struct ProcessOutput {
    /// Exit code (`None` if killed by a signal).
    pub exit_code: Option<i32>,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Either stream was truncated.
    pub truncated: bool,
}

/// Spawn `program args` with the hardened settings and wait for it.
///
/// * `kill_on_drop(true)` so a dropped future never leaks a child.
/// * On Unix the child leads a new process group; on timeout the whole group
///   is SIGKILLed so background jobs (`cmd &`) do not survive.
/// * stdout/stderr are drained concurrently (so a chatty process never blocks
///   on a full pipe) but only the first `cap` bytes are kept.
pub async fn run_process(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
    cap: usize,
) -> AppResult<ProcessOutput> {
    run_process_with_input(program, args, cwd, timeout, cap, None).await
}

/// [`run_process`] that also writes `input` to the child's stdin (then
/// closes it). Used for fixed-argv internal tools such as Piper TTS.
pub async fn run_process_with_input(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
    cap: usize,
    input: Option<Vec<u8>>,
) -> AppResult<ProcessOutput> {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(safe_env())
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    cmd.process_group(0);

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::Execution(format!("failed to start `{program}`: {e}")))?;
    let pid = child.id();
    if let (Some(data), Some(mut stdin)) = (input, child.stdin.take()) {
        // Write concurrently so a child that fills its stdout pipe before
        // reading all input cannot deadlock us. Dropping `stdin` closes it.
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let _ = stdin.write_all(&data).await;
        });
    }
    let out = child.stdout.take();
    let err = child.stderr.take();

    let work = async {
        let (o, e, status) =
            tokio::join!(read_capped(out, cap), read_capped(err, cap), child.wait());
        (o, e, status)
    };

    match tokio::time::timeout(timeout, work).await {
        Ok((o, e, status)) => {
            let (stdout, t1) = o?;
            let (stderr, t2) = e?;
            let status = status?;
            Ok(ProcessOutput {
                exit_code: status.code(),
                stdout: render(stdout, t1, cap),
                stderr: render(stderr, t2, cap),
                truncated: t1 || t2,
            })
        }
        Err(_) => {
            kill_group(pid);
            // Best effort: also kill the direct child (kill_on_drop covers the rest).
            let _ = child.start_kill();
            Err(AppError::Execution(format!(
                "timed out after {}s; the process was killed",
                timeout.as_secs()
            )))
        }
    }
}

/// SIGKILL the process group led by `pid` (Unix only).
fn kill_group(pid: Option<u32>) {
    #[cfg(unix)]
    if let Some(pid) = pid.and_then(|p| i32::try_from(p).ok()) {
        // SAFETY: killpg only sends a signal; `pid` is the group id we created
        // with `process_group(0)`, so we cannot signal unrelated groups
        // (pid > 0 is guaranteed by the conversion from a spawned child id).
        unsafe {
            libc::killpg(pid, libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    let _ = pid;
}

/// Read a stream to EOF, keeping at most `cap` bytes.
async fn read_capped<T: AsyncRead + Unpin>(
    src: Option<T>,
    cap: usize,
) -> std::io::Result<(Vec<u8>, bool)> {
    let Some(mut src) = src else {
        return Ok((Vec::new(), false));
    };
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut truncated = false;
    loop {
        let n = src.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        let room = cap.saturating_sub(buf.len());
        if room >= n {
            buf.extend_from_slice(&chunk[..n]);
        } else {
            buf.extend_from_slice(&chunk[..room]);
            truncated = true;
        }
    }
    Ok((buf, truncated))
}

fn render(bytes: Vec<u8>, truncated: bool, cap: usize) -> String {
    let mut s = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        s.push_str(&format!("\n…[output truncated at {cap} bytes]"));
    }
    s
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        std::env::temp_dir()
    }

    #[tokio::test]
    async fn runs_and_captures() {
        let o = run_process(
            "/bin/sh",
            &["-c".into(), "echo out; echo err >&2; exit 3".into()],
            &tmp(),
            Duration::from_secs(5),
            1024,
        )
        .await
        .expect("run");
        assert_eq!(o.exit_code, Some(3));
        assert_eq!(o.stdout.trim(), "out");
        assert_eq!(o.stderr.trim(), "err");
        assert!(!o.truncated);
    }

    #[tokio::test]
    async fn truncates_large_output() {
        let o = run_process(
            "/bin/sh",
            &["-c".into(), "head -c 100000 /dev/zero | tr '\\0' a".into()],
            &tmp(),
            Duration::from_secs(5),
            1000,
        )
        .await
        .expect("run");
        assert!(o.truncated);
        assert!(o.stdout.starts_with(&"a".repeat(1000)));
        assert!(o.stdout.contains("truncated"));
    }

    #[tokio::test]
    async fn times_out_and_kills_background_jobs() {
        let start = Instant::now();
        let r = run_process(
            "/bin/sh",
            &["-c".into(), "sleep 30 & sleep 30".into()],
            &tmp(),
            Duration::from_millis(300),
            1024,
        )
        .await;
        assert!(matches!(r, Err(AppError::Execution(_))));
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn stdin_input_is_delivered() {
        let o = run_process_with_input(
            "/bin/cat",
            &[],
            &tmp(),
            Duration::from_secs(5),
            1024,
            Some(b"hello".to_vec()),
        )
        .await
        .expect("run");
        assert_eq!(o.stdout, "hello");
    }

    #[tokio::test]
    async fn environment_is_cleared() {
        std::env::set_var("OMNIX_TEST_SECRET_API_KEY", "sk-should-not-leak");
        let o = run_process(
            "/bin/sh",
            &["-c".into(), "env".into()],
            &tmp(),
            Duration::from_secs(5),
            65536,
        )
        .await
        .expect("run");
        assert!(!o.stdout.contains("sk-should-not-leak"));
        assert!(o.stdout.contains("PATH="));
    }

    #[test]
    fn cwd_resolution() {
        let home = tmp();
        assert_eq!(
            resolve_cwd(None, Some(&home)).expect("home"),
            std::fs::canonicalize(&home).expect("canon")
        );
        assert!(resolve_cwd(Some("relative/dir"), Some(&home)).is_err());
        assert!(resolve_cwd(Some("/definitely/not/here"), Some(&home)).is_err());
    }
}
