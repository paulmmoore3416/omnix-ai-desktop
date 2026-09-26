//! Docker containers: list with live CPU/memory, control, logs.
//!
//! Listing uses fixed-argv probes (`docker ps`, `docker stats --no-stream`).
//! Control and logs build a `docker` command from a validated container name
//! and a fixed verb and run it through [`executor::execute`]: state changes
//! are Mutating (native confirmation), logs are read-only; all are audited.

use super::probe;
use crate::error::{AppError, AppResult};
use crate::security::executor::{self, ExecRequest, ExecResult};
use crate::security::policy::Source;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tauri::{AppHandle, Runtime};

/// A container.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Container {
    /// Short id.
    pub id: String,
    /// Name.
    pub name: String,
    /// Image.
    pub image: String,
    /// `running`, `exited`, `created`, `paused`, …
    pub state: String,
    /// Human status (`Up 2 hours`).
    pub status: String,
    /// Published ports.
    pub ports: String,
    /// Compose project, if any.
    pub project: Option<String>,
    /// CPU % (running containers).
    pub cpu: Option<f32>,
    /// Memory usage, human string from docker (`512MiB / 46GiB`).
    pub memory: Option<String>,
    /// Memory %.
    pub memory_percent: Option<f32>,
}

/// Result of a listing: `available: false` with a reason when Docker isn't
/// installed or the user can't reach the daemon.
#[derive(Debug, Clone, Serialize)]
pub struct DockerStatus {
    /// Docker is usable.
    pub available: bool,
    /// Why not.
    pub reason: Option<String>,
    /// Containers (running first).
    pub containers: Vec<Container>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PsRow {
    #[serde(rename = "ID")]
    id: String,
    names: String,
    image: String,
    state: String,
    status: String,
    #[serde(default)]
    ports: String,
    #[serde(default)]
    labels: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StatsRow {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "CPUPerc", default)]
    cpu_perc: String,
    #[serde(default)]
    mem_usage: String,
    #[serde(default)]
    mem_perc: String,
}

/// Allowed control verbs.
pub const ACTIONS: &[&str] = &["start", "stop", "restart", "pause", "unpause"];

/// Container names/ids are interpolated into a command line.
pub fn validate_name(name: &str) -> AppResult<()> {
    let ok = !name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
    if ok {
        Ok(())
    } else {
        Err(AppError::InvalidInput(format!(
            "`{name}` is not a valid container name"
        )))
    }
}

/// List containers with live stats for running ones.
pub async fn list() -> DockerStatus {
    if !probe::available("docker") {
        return DockerStatus {
            available: false,
            reason: Some("Docker is not installed".into()),
            containers: vec![],
        };
    }
    let Some(ps) = probe::run(
        "docker",
        &["ps", "-a", "--format", "{{json .}}"],
        Duration::from_secs(8),
    )
    .await
    else {
        return DockerStatus {
            available: false,
            reason: Some(
                "cannot reach the Docker daemon (is it running, and is your user in the `docker` group?)".into(),
            ),
            containers: vec![],
        };
    };
    let mut containers = parse_ps(&ps);
    if containers.iter().any(|c| c.state == "running") {
        if let Some(stats) = probe::run(
            "docker",
            &["stats", "--no-stream", "--format", "{{json .}}"],
            Duration::from_secs(10),
        )
        .await
        {
            apply_stats(&mut containers, &stats);
        }
    }
    DockerStatus {
        available: true,
        reason: None,
        containers,
    }
}

/// Parse `docker ps --format '{{json .}}'` (one object per line).
pub fn parse_ps(out: &str) -> Vec<Container> {
    let mut v: Vec<Container> = out
        .lines()
        .filter_map(|l| serde_json::from_str::<PsRow>(l).ok())
        .map(|r| Container {
            project: r
                .labels
                .split(',')
                .find_map(|kv| kv.strip_prefix("com.docker.compose.project="))
                .map(str::to_string),
            id: r.id.chars().take(12).collect(),
            name: r.names,
            image: r.image,
            state: r.state,
            status: r.status,
            ports: r.ports,
            cpu: None,
            memory: None,
            memory_percent: None,
        })
        .collect();
    v.sort_by(|a, b| {
        (a.state != "running")
            .cmp(&(b.state != "running"))
            .then(a.name.cmp(&b.name))
    });
    v
}

fn pct(s: &str) -> Option<f32> {
    s.trim().trim_end_matches('%').parse().ok()
}

/// Merge `docker stats --no-stream --format '{{json .}}'` into `containers`.
pub fn apply_stats(containers: &mut [Container], out: &str) {
    let by_id: HashMap<String, StatsRow> = out
        .lines()
        .filter_map(|l| serde_json::from_str::<StatsRow>(l).ok())
        .map(|s| (s.id.chars().take(12).collect(), s))
        .collect();
    for c in containers.iter_mut() {
        if let Some(s) = by_id.get(&c.id) {
            c.cpu = pct(&s.cpu_perc);
            c.memory = Some(s.mem_usage.clone());
            c.memory_percent = pct(&s.mem_perc);
        }
    }
}

/// Start/stop/… a container through the full policy pipeline.
pub async fn control<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    name: &str,
    action: &str,
    source: Source,
) -> AppResult<ExecResult> {
    validate_name(name)?;
    if !ACTIONS.contains(&action) {
        return Err(AppError::InvalidInput(format!(
            "unknown action `{action}` (use {})",
            ACTIONS.join(", ")
        )));
    }
    executor::execute(
        app,
        state,
        ExecRequest {
            command: format!("docker {action} {name}"),
            cwd: None,
            source,
        },
    )
    .await
}

/// Last `lines` log lines of a container (read-only; audited).
pub async fn logs<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    name: &str,
    lines: u32,
    source: Source,
) -> AppResult<String> {
    validate_name(name)?;
    let r = executor::execute(
        app,
        state,
        ExecRequest {
            command: format!(
                "docker logs --tail {} --timestamps {name}",
                lines.clamp(10, 2000)
            ),
            cwd: None,
            source,
        },
    )
    .await?;
    // Containers often log to stderr; show both.
    Ok(format!("{}{}", r.stdout, r.stderr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_validated() {
        assert!(validate_name("omnix-speaches").is_ok());
        assert!(validate_name("67fbef6783e1").is_ok());
        for bad in ["", "-x", "a;b", "$(id)", "a b", "../x"] {
            assert!(validate_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn parses_ps_and_stats() {
        let ps = r#"{"ID":"88663223fa39","Names":"nginx","Image":"nginx:stable-alpine","State":"created","Status":"Created","Ports":"","Labels":"com.docker.compose.project=mediaship,x=y"}
{"ID":"67fbef6783e1","Names":"omnix-speaches","Image":"ghcr.io/speaches-ai/speaches:latest-cpu","State":"running","Status":"Up 2 hours","Ports":"127.0.0.1:8000->8000/tcp","Labels":""}"#;
        let mut c = parse_ps(ps);
        assert_eq!(c[0].name, "omnix-speaches", "running first");
        assert_eq!(c[1].project.as_deref(), Some("mediaship"));
        apply_stats(
            &mut c,
            r#"{"ID":"67fbef6783e1","CPUPerc":"0.52%","MemUsage":"512MiB / 46GiB","MemPerc":"1.09%"}"#,
        );
        assert_eq!(c[0].cpu, Some(0.52));
        assert_eq!(c[0].memory_percent, Some(1.09));
        assert_eq!(c[1].cpu, None);
    }

    #[test]
    fn docker_commands_classify_as_expected() {
        use crate::security::policy::{classify, PolicyConfig, RiskTier};
        let cfg = PolicyConfig::default();
        assert_eq!(
            classify("docker restart omnix-speaches", &cfg).tier,
            RiskTier::Mutating
        );
        assert_eq!(
            classify("docker logs --tail 200 --timestamps omnix-speaches", &cfg).tier,
            RiskTier::ReadOnly
        );
    }
}
