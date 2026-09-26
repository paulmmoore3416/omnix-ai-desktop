//! Alerts, automations and scheduled tasks (persisted in `ops.json`).

use serde::{Deserialize, Serialize};

/// What a condition measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    /// CPU %.
    Cpu,
    /// RAM %.
    Memory,
    /// Swap %.
    Swap,
    /// Disk used % (all local disks).
    Disk,
    /// Hottest sensor °C.
    Temperature,
    /// GPU utilisation % (`target` = GPU index, default: any GPU).
    GpuUtil,
    /// GPU VRAM used %.
    GpuMemory,
    /// GPU °C.
    GpuTemp,
    /// No process named `target` is running.
    ProcessMissing,
    /// systemd unit `target` (`scope/unit` or `unit`) is failed or inactive.
    ServiceDown,
    /// Docker container `target` is not running.
    ContainerDown,
    /// Ollama does not answer.
    OllamaDown,
    /// kb-core does not answer.
    KbCoreDown,
}

impl Metric {
    /// Boolean metrics ignore `op`/`threshold`.
    pub fn is_boolean(self) -> bool {
        matches!(
            self,
            Metric::ProcessMissing
                | Metric::ServiceDown
                | Metric::ContainerDown
                | Metric::OllamaDown
                | Metric::KbCoreDown
        )
    }

    /// Needs `target`.
    pub fn needs_target(self) -> bool {
        matches!(
            self,
            Metric::ProcessMissing | Metric::ServiceDown | Metric::ContainerDown
        )
    }

    /// Unit for display.
    pub fn unit(self) -> &'static str {
        match self {
            Metric::Temperature | Metric::GpuTemp => "°C",
            m if m.is_boolean() => "",
            _ => "%",
        }
    }
}

/// Comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    /// value > threshold
    #[default]
    Above,
    /// value < threshold
    Below,
}

fn default_sustain() -> u64 {
    60
}

/// A condition on host state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    /// Metric.
    pub metric: Metric,
    /// Comparison.
    #[serde(default)]
    pub op: Op,
    /// Threshold (ignored for boolean metrics).
    #[serde(default)]
    pub threshold: f32,
    /// Must hold continuously this long (seconds, 0–86400).
    #[serde(default = "default_sustain")]
    pub sustain_secs: u64,
    /// GPU index, process name, `scope/unit`, or container name.
    #[serde(default)]
    pub target: Option<String>,
}

impl Condition {
    /// Human description, e.g. `CPU above 90% for 2 min`.
    pub fn describe(&self) -> String {
        let name = match self.metric {
            Metric::Cpu => "CPU".to_string(),
            Metric::Memory => "memory".into(),
            Metric::Swap => "swap".into(),
            Metric::Disk => "disk usage".into(),
            Metric::Temperature => "temperature".into(),
            Metric::GpuUtil => format!(
                "GPU{} load",
                self.target
                    .as_deref()
                    .map(|t| format!(" {t}"))
                    .unwrap_or_default()
            ),
            Metric::GpuMemory => format!(
                "GPU{} memory",
                self.target
                    .as_deref()
                    .map(|t| format!(" {t}"))
                    .unwrap_or_default()
            ),
            Metric::GpuTemp => format!(
                "GPU{} temperature",
                self.target
                    .as_deref()
                    .map(|t| format!(" {t}"))
                    .unwrap_or_default()
            ),
            Metric::ProcessMissing => format!(
                "process `{}` not running",
                self.target.as_deref().unwrap_or("?")
            ),
            Metric::ServiceDown => {
                format!("service `{}` down", self.target.as_deref().unwrap_or("?"))
            }
            Metric::ContainerDown => {
                format!("container `{}` down", self.target.as_deref().unwrap_or("?"))
            }
            Metric::OllamaDown => "Ollama unreachable".into(),
            Metric::KbCoreDown => "kb-core unreachable".into(),
        };
        let dur = if self.sustain_secs >= 120 {
            format!(" for {} min", self.sustain_secs / 60)
        } else if self.sustain_secs > 0 {
            format!(" for {} s", self.sustain_secs)
        } else {
            String::new()
        };
        if self.metric.is_boolean() {
            format!("{name}{dur}")
        } else {
            let op = if self.op == Op::Above {
                "above"
            } else {
                "below"
            };
            format!("{name} {op} {}{}{dur}", self.threshold, self.metric.unit())
        }
    }
}

/// Firing state of an alert.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AlertState {
    /// Currently firing.
    pub firing: bool,
    /// When it started firing.
    pub since: Option<String>,
    /// Last time it fired (notified).
    pub last_fired: Option<String>,
    /// Times fired.
    pub fire_count: u64,
    /// Latest measured value.
    pub last_value: Option<f32>,
}

fn default_cooldown() -> u64 {
    900
}
fn yes() -> bool {
    true
}

/// A watched condition that notifies (and can trigger automations).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alert {
    /// `a_…`
    pub id: String,
    /// Name.
    pub name: String,
    /// Condition.
    pub condition: Condition,
    /// Active.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Desktop notification when it fires.
    #[serde(default = "yes")]
    pub notify: bool,
    /// Minimum seconds between two firings.
    #[serde(default = "default_cooldown")]
    pub cooldown_secs: u64,
    /// Runtime state.
    #[serde(default)]
    pub state: AlertState,
}

/// What starts an automation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    /// An alert starts firing.
    Alert {
        /// Alert id.
        alert_id: String,
    },
    /// An inline condition becomes true.
    Condition {
        /// Condition.
        condition: Condition,
    },
    /// A process with this name appears.
    ProcessStart {
        /// Process name.
        name: String,
    },
    /// A process with this name disappears.
    ProcessStop {
        /// Process name.
        name: String,
    },
    /// A file's modification time changes.
    FileChange {
        /// Absolute path.
        path: String,
    },
    /// CPU below `cpu_below` % for `minutes`.
    Idle {
        /// Minutes.
        minutes: u32,
        /// CPU threshold.
        cpu_below: f32,
    },
}

impl Trigger {
    /// Human description.
    pub fn describe(&self) -> String {
        match self {
            Trigger::Alert { alert_id } => format!("when alert {alert_id} fires"),
            Trigger::Condition { condition } => format!("when {}", condition.describe()),
            Trigger::ProcessStart { name } => format!("when `{name}` starts"),
            Trigger::ProcessStop { name } => format!("when `{name}` stops"),
            Trigger::FileChange { path } => format!("when {path} changes"),
            Trigger::Idle { minutes, cpu_below } => {
                format!("when the computer is idle (CPU < {cpu_below}% for {minutes} min)")
            }
        }
    }
}

fn default_report_save() -> bool {
    false
}

/// What a rule does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    /// Desktop notification (plus an in-app toast).
    Notify {
        /// Title.
        title: String,
        /// Body.
        message: String,
    },
    /// Run a command through the policy engine. Commands that need
    /// confirmation carry a signed approval (`approval`).
    Command {
        /// Command line.
        command: String,
        /// Working directory.
        #[serde(default)]
        cwd: Option<String>,
        /// HMAC approval (hex), set by OMNIX after the native dialog.
        #[serde(default)]
        approval: Option<String>,
    },
    /// Ask the local model to write a report on the current host state.
    AiReport {
        /// What to report on.
        prompt: String,
        /// Also index the report in long-term memory.
        #[serde(default = "default_report_save")]
        save_to_memory: bool,
    },
}

impl Action {
    /// Human description.
    pub fn describe(&self) -> String {
        match self {
            Action::Notify { title, .. } => format!("notify “{title}”"),
            Action::Command { command, .. } => format!("run `{command}`"),
            Action::AiReport { prompt, .. } => {
                format!("AI report: {}", prompt.chars().take(80).collect::<String>())
            }
        }
    }
}

/// Last outcome of a rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RunState {
    /// Last start time.
    pub last_run: Option<String>,
    /// Runs so far.
    pub run_count: u64,
    /// Last run succeeded.
    pub last_ok: Option<bool>,
    /// Short result (≤ 500 chars).
    pub last_result: Option<String>,
}

fn default_auto_cooldown() -> u64 {
    300
}

/// Trigger → action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Automation {
    /// `u_…`
    pub id: String,
    /// Name.
    pub name: String,
    /// Trigger.
    pub trigger: Trigger,
    /// Action.
    pub action: Action,
    /// Active.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Minimum seconds between runs.
    #[serde(default = "default_auto_cooldown")]
    pub cooldown_secs: u64,
    /// Last outcome.
    #[serde(default)]
    pub run: RunState,
}

/// Time → action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduledTask {
    /// `t_…`
    pub id: String,
    /// Name.
    pub name: String,
    /// Cron / macro / `@every` expression.
    pub schedule: String,
    /// Action.
    pub action: Action,
    /// Active.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Next planned run (RFC 3339, local offset).
    #[serde(default)]
    pub next_run: Option<String>,
    /// Last outcome.
    #[serde(default)]
    pub run: RunState,
}

/// One line of the activity feed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    /// RFC 3339.
    pub ts: String,
    /// `alert_fired`, `alert_resolved`, `automation`, `schedule`, `rule_created`, …
    pub kind: String,
    /// Rule name.
    pub name: String,
    /// Success.
    pub ok: bool,
    /// Short summary.
    pub summary: String,
}

/// The whole file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OpsFile {
    /// Format version.
    #[serde(default)]
    pub version: u32,
    /// Alerts.
    #[serde(default)]
    pub alerts: Vec<Alert>,
    /// Automations.
    #[serde(default)]
    pub automations: Vec<Automation>,
    /// Scheduled tasks.
    #[serde(default)]
    pub tasks: Vec<ScheduledTask>,
    /// Recent activity (newest last, bounded).
    #[serde(default)]
    pub activity: Vec<Activity>,
}
