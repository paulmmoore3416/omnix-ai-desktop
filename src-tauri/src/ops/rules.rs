//! Create, toggle and delete rules, with validation and approvals.

use super::approval;
use super::cron::Schedule;
use super::model::{
    Action, Alert, AlertState, Automation, Condition, Metric, RunState, ScheduledTask, Trigger,
};
use super::MAX_RULES;
use crate::error::{AppError, AppResult};
use crate::security::audit::{AuditRecord, Confirmation, Decision};
use crate::security::confirm::{self, ConfirmRequest};
use crate::security::executor;
use crate::security::policy::{self, RiskTier, Source};
use crate::state::AppState;
use serde::Deserialize;
use tauri::{AppHandle, Runtime};

/// New alert.
#[derive(Debug, Clone, Deserialize)]
pub struct AlertInput {
    /// Name.
    pub name: String,
    /// Condition.
    pub condition: Condition,
    /// Desktop notification (default on).
    #[serde(default)]
    pub notify: Option<bool>,
    /// Also text or call the owner's phone.
    #[serde(default)]
    pub phone: Option<crate::phone::PhoneChannel>,
    /// Cooldown seconds.
    #[serde(default)]
    pub cooldown_secs: Option<u64>,
}

/// New automation.
#[derive(Debug, Clone, Deserialize)]
pub struct AutomationInput {
    /// Name.
    pub name: String,
    /// Trigger.
    pub trigger: Trigger,
    /// Action.
    pub action: Action,
    /// Cooldown seconds.
    #[serde(default)]
    pub cooldown_secs: Option<u64>,
}

/// New scheduled task.
#[derive(Debug, Clone, Deserialize)]
pub struct TaskInput {
    /// Name.
    pub name: String,
    /// Schedule expression.
    pub schedule: String,
    /// Action.
    pub action: Action,
}

fn bad(msg: impl Into<String>) -> AppError {
    AppError::InvalidInput(msg.into())
}

fn check_name(n: &str) -> AppResult<String> {
    let n = n.trim();
    if n.is_empty() || n.chars().count() > 80 {
        return Err(bad("name must be 1–80 characters"));
    }
    Ok(n.to_string())
}

/// Process names as sysinfo reports them.
fn check_process_name(n: &str) -> AppResult<()> {
    if !n.is_empty()
        && n.len() <= 64
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+' | ':'))
    {
        Ok(())
    } else {
        Err(bad(format!("`{n}` is not a valid process name")))
    }
}

/// Validate a condition (targets end up in probes and descriptions).
pub fn check_condition(c: &Condition) -> AppResult<()> {
    if c.sustain_secs > 86_400 {
        return Err(bad("sustain must be at most 24 hours"));
    }
    if !c.threshold.is_finite() || !(-100.0..=1000.0).contains(&c.threshold) {
        return Err(bad("threshold is out of range"));
    }
    let t = c.target.as_deref().map(str::trim).filter(|t| !t.is_empty());
    if c.metric.needs_target() && t.is_none() {
        return Err(bad(
            "this metric needs a target (process, service or container name)",
        ));
    }
    if let Some(t) = t {
        match c.metric {
            Metric::GpuUtil | Metric::GpuMemory | Metric::GpuTemp => {
                t.parse::<u8>()
                    .map_err(|_| bad("GPU target must be a GPU index (0, 1, …)"))?;
            }
            Metric::ProcessMissing => check_process_name(t)?,
            Metric::ServiceDown => {
                let (scope, unit) = t.split_once('/').unwrap_or(("system", t));
                if scope != "system" && scope != "user" {
                    return Err(bad(
                        "service target is `unit`, `system/unit` or `user/unit`",
                    ));
                }
                crate::system::services::validate_unit(unit)?;
            }
            Metric::ContainerDown => crate::system::docker::validate_name(t)?,
            _ => return Err(bad("this metric takes no target")),
        }
    }
    Ok(())
}

fn check_trigger(t: &Trigger, alerts: &[Alert]) -> AppResult<()> {
    match t {
        Trigger::Alert { alert_id } => {
            if !alerts.iter().any(|a| &a.id == alert_id) {
                return Err(bad(format!("no alert with id {alert_id}")));
            }
        }
        Trigger::Condition { condition } => check_condition(condition)?,
        Trigger::ProcessStart { name } | Trigger::ProcessStop { name } => check_process_name(name)?,
        Trigger::FileChange { path } => {
            let p = std::path::Path::new(path);
            if !p.is_absolute() || path.len() > 1024 {
                return Err(bad("file path must be absolute"));
            }
        }
        Trigger::Idle { minutes, cpu_below } => {
            if !(1..=1440).contains(minutes) || !(1.0..=100.0).contains(cpu_below) {
                return Err(bad(
                    "idle needs 1–1440 minutes and a CPU threshold of 1–100%",
                ));
            }
        }
    }
    Ok(())
}

fn check_action_shape(a: &Action) -> AppResult<()> {
    match a {
        Action::Notify { title, message } => {
            if title.trim().is_empty()
                || title.chars().count() > 80
                || message.chars().count() > 500
            {
                return Err(bad(
                    "notification needs a title (≤ 80) and a message (≤ 500 characters)",
                ));
            }
        }
        Action::Command { command, .. } => {
            if command.trim().is_empty() || command.len() > 2000 {
                return Err(bad("command must be 1–2000 characters"));
            }
        }
        Action::AiReport { prompt, .. } => {
            if prompt.trim().is_empty() || prompt.chars().count() > 2000 {
                return Err(bad("report prompt must be 1–2000 characters"));
            }
        }
        Action::Text { message } | Action::Call { message } => {
            if message.trim().is_empty() || message.chars().count() > 500 {
                return Err(bad("a text or call needs a message of 1–500 characters"));
            }
        }
    }
    Ok(())
}

fn audit_rule(
    action: &str,
    what: &str,
    source: Source,
    tier: RiskTier,
    decision: Decision,
    c: Confirmation,
    detail: Option<String>,
) -> AuditRecord {
    AuditRecord {
        id: uuid::Uuid::new_v4().to_string(),
        source,
        action: action.into(),
        command: what.chars().take(500).collect(),
        cwd: None,
        tier,
        decision,
        confirmation: c,
        exit_code: None,
        duration_ms: None,
        detail,
    }
}

/// Classify a command action and, if it would need confirmation, get the
/// one-time native approval and sign it. Also serves as the creation
/// confirmation for model-proposed rules. Returns whether a dialog was shown.
async fn approve_action<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    rule_id: &str,
    rule_desc: &str,
    action: &mut Action,
    source: Source,
) -> AppResult<bool> {
    let Action::Command {
        command,
        cwd,
        approval: slot,
    } = action
    else {
        return Ok(false);
    };
    *command = command.trim().to_string();
    let settings = state.settings.read().await.clone();
    let cwd_path = executor::resolve_cwd(cwd.as_deref(), state.home.as_deref())?;
    let cwd_str = cwd_path.to_string_lossy().to_string();
    *cwd = Some(cwd_str.clone());
    let class = policy::classify(command, &state.policy_config(&settings, Some(&cwd_path)));
    let rec = |d, c, detail| audit_rule("ops_approve", command, source, class.tier, d, c, detail);
    match class.tier {
        RiskTier::Denied => {
            state
                .audit
                .record(rec(
                    Decision::Denied,
                    Confirmation::Skipped,
                    Some(class.reasons.join("; ")),
                ))
                .await?;
            return Err(AppError::PolicyDenied(class.reasons.join("; ")));
        }
        RiskTier::Privileged => {
            let msg = "privileged (sudo) commands can't run unattended: elevation needs you at the password prompt";
            state
                .audit
                .record(rec(
                    Decision::Denied,
                    Confirmation::Skipped,
                    Some(msg.into()),
                ))
                .await?;
            return Err(AppError::PolicyDenied(msg.into()));
        }
        _ => {}
    }
    let needs = class.tier >= RiskTier::Mutating || settings.security.require_confirmation;
    if !needs {
        // Read-only: nothing to pre-approve (re-classified at every run).
        *slot = None;
        return Ok(false);
    }
    let c = confirm::require(
        app,
        state,
        &ConfirmRequest {
            title: "Allow unattended command?".into(),
            subject: format!("Command:\n{command}"),
            details: vec![
                format!("Runs automatically: {rule_desc}"),
                format!("Working directory: {cwd_str}"),
                "OMNIX will run it without asking each time. Changing the command requires approval again.".into(),
            ],
            tier: class.tier,
            source,
            reasons: class.reasons.clone(),
            approve_label: "Allow".into(),
        },
    )
    .await;
    match c {
        Ok(c) => {
            *slot = Some(approval::sign(
                state.secrets.as_ref(),
                rule_id,
                command,
                Some(&cwd_str),
            )?);
            state
                .audit
                .record(rec(
                    Decision::Allowed,
                    c,
                    Some(format!("approved for {rule_desc}")),
                ))
                .await?;
            Ok(true)
        }
        Err(c) => {
            state
                .audit
                .record(rec(Decision::NotApproved, c, None))
                .await?;
            Err(AppError::NotApproved(
                "the unattended command was not approved".into(),
            ))
        }
    }
}

/// Model-proposed rules always get a native confirmation (unless the
/// command approval dialog already covered it).
async fn confirm_llm_rule<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    kind: &str,
    desc: &str,
) -> AppResult<()> {
    let c = confirm::require(
        app,
        state,
        &ConfirmRequest {
            title: format!("Create {kind}?"),
            subject: desc.to_string(),
            details: vec![
                "Proposed by the AI assistant. You can disable or delete it in System Control."
                    .into(),
            ],
            tier: RiskTier::Mutating,
            source: Source::LlmTool,
            reasons: vec!["runs automatically in the background".into()],
            approve_label: "Create".into(),
        },
    )
    .await;
    let (d, conf) = match c {
        Ok(c) => (Decision::Allowed, c),
        Err(c) => (Decision::NotApproved, c),
    };
    state
        .audit
        .record(audit_rule(
            "ops_create",
            desc,
            Source::LlmTool,
            RiskTier::Mutating,
            d,
            conf,
            None,
        ))
        .await?;
    if d == Decision::Allowed {
        Ok(())
    } else {
        Err(AppError::NotApproved(format!("the {kind} was not created")))
    }
}

/// Create an alert.
pub async fn create_alert<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    input: AlertInput,
    source: Source,
) -> AppResult<Alert> {
    let name = check_name(&input.name)?;
    check_condition(&input.condition)?;
    let alert = Alert {
        id: format!("a_{}", uuid::Uuid::new_v4().simple()),
        name,
        condition: input.condition,
        enabled: true,
        notify: input.notify.unwrap_or(true),
        phone: input.phone,
        cooldown_secs: input.cooldown_secs.unwrap_or(900).clamp(0, 86_400),
        state: AlertState::default(),
    };
    let desc = format!(
        "Alert “{}”: {}{}",
        alert.name,
        alert.condition.describe(),
        match alert.phone {
            Some(crate::phone::PhoneChannel::Sms) => " (and text your phone)",
            Some(crate::phone::PhoneChannel::Call) => " (and call your phone)",
            None => "",
        }
    );
    if source == Source::LlmTool {
        confirm_llm_rule(app, state, "alert", &desc).await?;
    }
    let a = alert.clone();
    state.ops.update(|f| {
        if f.alerts.len() >= MAX_RULES {
            return Err(bad("too many alerts"));
        }
        f.alerts.push(a);
        Ok(())
    })?;
    state.ops.activity("rule_created", &alert.name, true, &desc);
    Ok(alert)
}

/// Create an automation.
pub async fn create_automation<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    input: AutomationInput,
    source: Source,
) -> AppResult<Automation> {
    let name = check_name(&input.name)?;
    check_trigger(&input.trigger, &state.ops.snapshot().alerts)?;
    check_action_shape(&input.action)?;
    let id = format!("u_{}", uuid::Uuid::new_v4().simple());
    let desc = format!(
        "Automation “{name}”: {} → {}",
        input.trigger.describe(),
        input.action.describe()
    );
    let mut action = input.action;
    let shown = approve_action(
        app,
        state,
        &id,
        &input.trigger.describe(),
        &mut action,
        source,
    )
    .await?;
    if source == Source::LlmTool && !shown {
        confirm_llm_rule(app, state, "automation", &desc).await?;
    }
    let auto = Automation {
        id,
        name,
        trigger: input.trigger,
        action,
        enabled: true,
        cooldown_secs: input.cooldown_secs.unwrap_or(300).clamp(0, 86_400),
        run: RunState::default(),
    };
    let a = auto.clone();
    state.ops.update(|f| {
        if f.automations.len() >= MAX_RULES {
            return Err(bad("too many automations"));
        }
        f.automations.push(a);
        Ok(())
    })?;
    state.ops.activity("rule_created", &auto.name, true, &desc);
    Ok(auto)
}

/// Create a scheduled task.
pub async fn create_task<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    input: TaskInput,
    source: Source,
) -> AppResult<ScheduledTask> {
    let name = check_name(&input.name)?;
    let sched = Schedule::parse(&input.schedule).map_err(bad)?;
    let next = sched
        .next_after(chrono::Local::now())
        .ok_or_else(|| bad("this schedule never runs"))?;
    check_action_shape(&input.action)?;
    let id = format!("t_{}", uuid::Uuid::new_v4().simple());
    let when = Schedule::describe(&input.schedule);
    let desc = format!(
        "Scheduled task “{name}”: {when} → {}",
        input.action.describe()
    );
    let mut action = input.action;
    let shown = approve_action(app, state, &id, &when, &mut action, source).await?;
    if source == Source::LlmTool && !shown {
        confirm_llm_rule(app, state, "scheduled task", &desc).await?;
    }
    let task = ScheduledTask {
        id,
        name,
        schedule: input.schedule.trim().to_string(),
        action,
        enabled: true,
        next_run: Some(next.to_rfc3339_opts(chrono::SecondsFormat::Secs, false)),
        run: RunState::default(),
    };
    let t = task.clone();
    state.ops.update(|f| {
        if f.tasks.len() >= MAX_RULES {
            return Err(bad("too many scheduled tasks"));
        }
        f.tasks.push(t);
        Ok(())
    })?;
    state.ops.activity("rule_created", &task.name, true, &desc);
    Ok(task)
}

/// Rule kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Alert.
    Alert,
    /// Automation.
    Automation,
    /// Scheduled task.
    Task,
}

/// Enable or disable a rule.
pub fn set_enabled(state: &AppState, kind: Kind, id: &str, enabled: bool) -> AppResult<()> {
    state.ops.update(|f| {
        let found = match kind {
            Kind::Alert => f.alerts.iter_mut().find(|r| r.id == id).map(|r| {
                r.enabled = enabled;
                if !enabled {
                    r.state.firing = false;
                    r.state.since = None;
                }
            }),
            Kind::Automation => f
                .automations
                .iter_mut()
                .find(|r| r.id == id)
                .map(|r| r.enabled = enabled),
            Kind::Task => f.tasks.iter_mut().find(|r| r.id == id).map(|r| {
                r.enabled = enabled;
                r.next_run = Schedule::parse(&r.schedule)
                    .ok()
                    .and_then(|s| s.next_after(chrono::Local::now()))
                    .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, false));
            }),
        };
        found.ok_or_else(|| bad("no such rule"))
    })
}

/// Delete a rule (automations triggered by a deleted alert are disabled).
pub fn delete(state: &AppState, kind: Kind, id: &str) -> AppResult<()> {
    state.ops.update(|f| {
        let before = f.alerts.len() + f.automations.len() + f.tasks.len();
        match kind {
            Kind::Alert => {
                f.alerts.retain(|r| r.id != id);
                for a in f.automations.iter_mut() {
                    if matches!(&a.trigger, Trigger::Alert { alert_id } if alert_id == id) {
                        a.enabled = false;
                    }
                }
            }
            Kind::Automation => f.automations.retain(|r| r.id != id),
            Kind::Task => f.tasks.retain(|r| r.id != id),
        }
        if f.alerts.len() + f.automations.len() + f.tasks.len() == before {
            return Err(bad("no such rule"));
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cond(metric: Metric, target: Option<&str>) -> Condition {
        Condition {
            metric,
            op: Default::default(),
            threshold: 90.0,
            sustain_secs: 60,
            target: target.map(str::to_string),
        }
    }

    #[test]
    fn conditions_are_validated() {
        assert!(check_condition(&cond(Metric::Cpu, None)).is_ok());
        assert!(check_condition(&cond(Metric::GpuTemp, Some("1"))).is_ok());
        assert!(check_condition(&cond(Metric::GpuTemp, Some("x"))).is_err());
        assert!(check_condition(&cond(Metric::ProcessMissing, None)).is_err());
        assert!(check_condition(&cond(Metric::ProcessMissing, Some("ollama"))).is_ok());
        assert!(check_condition(&cond(Metric::ProcessMissing, Some("a;b"))).is_err());
        assert!(check_condition(&cond(
            Metric::ServiceDown,
            Some("user/omnix-kb-core.service")
        ))
        .is_ok());
        assert!(check_condition(&cond(Metric::ServiceDown, Some("root/x.service"))).is_err());
        assert!(check_condition(&cond(Metric::ContainerDown, Some("omnix-speaches"))).is_ok());
        assert!(check_condition(&cond(Metric::Cpu, Some("x"))).is_err());
        let mut c = cond(Metric::Cpu, None);
        c.threshold = f32::NAN;
        assert!(check_condition(&c).is_err());
    }

    #[test]
    fn actions_and_triggers_are_validated() {
        assert!(check_action_shape(&Action::Notify {
            title: "".into(),
            message: "".into()
        })
        .is_err());
        assert!(check_action_shape(&Action::Command {
            command: " ".into(),
            cwd: None,
            approval: None
        })
        .is_err());
        assert!(check_trigger(
            &Trigger::FileChange {
                path: "rel/x".into()
            },
            &[]
        )
        .is_err());
        assert!(check_trigger(
            &Trigger::Alert {
                alert_id: "a_missing".into()
            },
            &[]
        )
        .is_err());
        assert!(check_trigger(
            &Trigger::Idle {
                minutes: 10,
                cpu_below: 5.0
            },
            &[]
        )
        .is_ok());
    }
}
