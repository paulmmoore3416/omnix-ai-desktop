//! Process listing and (confirmed, audited) termination.

use crate::error::{AppError, AppResult};
use crate::security::audit::{AuditRecord, Confirmation, Decision};
use crate::security::confirm::{self, ConfirmRequest};
use crate::security::policy::{RiskTier, Source};
use crate::state::AppState;
use crate::system::metrics::{percent, Monitor};
use serde::Serialize;
use std::time::Duration;
use sysinfo::{Pid, ProcessesToUpdate, Signal};
use tauri::{AppHandle, Runtime};

/// Maximum processes returned to the UI.
pub const MAX_PROCESSES: usize = 100;

/// One row in the process table.
#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    /// Process id.
    pub pid: u32,
    /// Executable name.
    pub name: String,
    /// CPU usage %.
    pub cpu: f32,
    /// Memory usage % of total RAM.
    pub memory: f32,
    /// Scheduler status.
    pub status: String,
}

/// Top processes by CPU usage.
pub fn list(monitor: &mut Monitor) -> Vec<ProcessInfo> {
    let sys = monitor.sys();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.refresh_memory();
    let total = sys.total_memory();
    let mut v: Vec<ProcessInfo> = sys
        .processes()
        .iter()
        .map(|(pid, p)| ProcessInfo {
            pid: pid.as_u32(),
            name: p.name().to_string_lossy().into_owned(),
            cpu: p.cpu_usage(),
            memory: percent(p.memory(), total),
            status: format!("{:?}", p.status()),
        })
        .collect();
    // total_cmp gives a total order even with NaN (no unwrap needed).
    v.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
    v.truncate(MAX_PROCESSES);
    v
}

/// Terminate `pid` after native confirmation. Tier `Mutating`.
///
/// Refuses PID 0/1 and OMNIX itself. Sends SIGTERM first and falls back to
/// SIGKILL where SIGTERM is unsupported.
pub async fn kill<R: Runtime>(app: &AppHandle<R>, state: &AppState, pid: u32) -> AppResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    let base = AuditRecord {
        id,
        source: Source::User,
        action: "kill_process".into(),
        command: format!("kill {pid}"),
        cwd: None,
        tier: RiskTier::Mutating,
        decision: Decision::Denied,
        confirmation: Confirmation::Skipped,
        exit_code: None,
        duration_ms: None,
        detail: None,
    };
    if pid <= 1 || pid == std::process::id() {
        let msg = "refusing to kill init, the kernel or OMNIX itself";
        state
            .audit
            .record(AuditRecord {
                tier: RiskTier::Denied,
                detail: Some(msg.into()),
                ..base
            })
            .await?;
        return Err(AppError::PolicyDenied(msg.into()));
    }
    let name = {
        let mut monitor = state.monitor()?;
        let sys = monitor.sys();
        sys.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
        sys.process(Pid::from_u32(pid))
            .map(|p| p.name().to_string_lossy().into_owned())
            .ok_or_else(|| AppError::InvalidInput(format!("no process with PID {pid}")))?
    };
    let timeout = Duration::from_secs(
        state
            .settings
            .read()
            .await
            .security
            .confirmation_timeout_secs,
    );
    let c = confirm::ask(
        app,
        &ConfirmRequest {
            title: "End process?".into(),
            subject: format!("Process: {name} (PID {pid})"),
            details: vec!["Unsaved work in that program will be lost.".into()],
            tier: RiskTier::Mutating,
            source: Source::User,
            reasons: vec!["terminates a running process".into()],
            approve_label: "End process".into(),
        },
        timeout,
    )
    .await;
    let base = AuditRecord {
        command: format!("kill {pid} ({name})"),
        confirmation: c,
        ..base
    };
    if c != Confirmation::Approved {
        state
            .audit
            .record(AuditRecord {
                decision: Decision::NotApproved,
                ..base
            })
            .await?;
        return Err(AppError::NotApproved("process was not terminated".into()));
    }
    let killed = {
        let mut monitor = state.monitor()?;
        let sys = monitor.sys();
        sys.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
        match sys.process(Pid::from_u32(pid)) {
            Some(p) => p.kill_with(Signal::Term).unwrap_or_else(|| p.kill()),
            None => false,
        }
    };
    state
        .audit
        .record(AuditRecord {
            decision: if killed {
                Decision::Allowed
            } else {
                Decision::Failed
            },
            ..base
        })
        .await?;
    if killed {
        Ok(())
    } else {
        Err(AppError::Execution(format!(
            "could not signal PID {pid} (it may have exited or belong to another user)"
        )))
    }
}
