//! systemd services: list (system + user scope) and control.
//!
//! Listing uses fixed-argv probes. Control builds a `systemctl` command from a
//! validated unit name and a fixed verb and runs it through
//! [`executor::execute`], so the policy engine classifies it (state changes
//! are Mutating), the user approves it in a native dialog, and it is audited.
//! For system-scope units OMNIX runs plain `systemctl <verb> <unit>` (no
//! `sudo`): after OMNIX's confirmation, systemd itself asks the desktop's
//! polkit agent for authorisation, so OMNIX never handles a password.

use super::probe;
use crate::error::{AppError, AppResult};
use crate::security::executor::{self, ExecRequest, ExecResult};
use crate::security::policy::Source;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tauri::{AppHandle, Runtime};

/// A systemd service unit.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Service {
    /// Unit name, e.g. `ollama.service`.
    pub unit: String,
    /// `system` or `user`.
    pub scope: String,
    /// Unit description.
    pub description: String,
    /// `loaded`, `not-found`, …
    pub load: String,
    /// `active`, `inactive`, `failed`, …
    pub active: String,
    /// `running`, `exited`, `dead`, …
    pub sub: String,
    /// `enabled`, `disabled`, `static`, … when known.
    pub enabled: Option<String>,
}

#[derive(Deserialize)]
struct UnitRow {
    unit: String,
    #[serde(default)]
    load: String,
    #[serde(default)]
    active: String,
    #[serde(default)]
    sub: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize)]
struct UnitFileRow {
    unit_file: String,
    #[serde(default)]
    state: String,
}

/// Allowed control verbs.
pub const ACTIONS: &[&str] = &["start", "stop", "restart", "reload", "enable", "disable"];

/// Unit names are interpolated into a command line: allow only systemd's
/// own character set and require the `.service` suffix.
pub fn validate_unit(unit: &str) -> AppResult<()> {
    let ok = unit.len() <= 256
        && unit.ends_with(".service")
        && unit.len() > ".service".len()
        && !unit.starts_with('-')
        && unit
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | '-' | ':' | '\\'));
    if ok {
        Ok(())
    } else {
        Err(AppError::InvalidInput(format!(
            "`{unit}` is not a valid service unit name"
        )))
    }
}

/// List services in both scopes (units that are loaded or installed).
pub async fn list() -> Vec<Service> {
    let (sys, sys_files, user, user_files) = tokio::join!(
        units(false),
        unit_files(false),
        units(true),
        unit_files(true)
    );
    let mut out = merge(sys, sys_files, "system");
    out.extend(merge(user, user_files, "user"));
    out.sort_by(|a, b| {
        rank(&a.active)
            .cmp(&rank(&b.active))
            .then(a.scope.cmp(&b.scope))
            .then(a.unit.cmp(&b.unit))
    });
    out
}

/// Failed units first, then active, then the rest.
fn rank(active: &str) -> u8 {
    match active {
        "failed" => 0,
        "active" | "activating" | "reloading" => 1,
        _ => 2,
    }
}

async fn units(user: bool) -> Vec<UnitRow> {
    let mut args = vec![];
    if user {
        args.push("--user");
    }
    args.extend([
        "list-units",
        "--type=service",
        "--all",
        "--output=json",
        "--no-pager",
    ]);
    probe::run("systemctl", &args, Duration::from_secs(8))
        .await
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

async fn unit_files(user: bool) -> Vec<UnitFileRow> {
    let mut args = vec![];
    if user {
        args.push("--user");
    }
    args.extend([
        "list-unit-files",
        "--type=service",
        "--output=json",
        "--no-pager",
    ]);
    probe::run("systemctl", &args, Duration::from_secs(8))
        .await
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn merge(units: Vec<UnitRow>, files: Vec<UnitFileRow>, scope: &str) -> Vec<Service> {
    let states: HashMap<String, String> = files
        .into_iter()
        .map(|f| {
            let name = f
                .unit_file
                .rsplit('/')
                .next()
                .unwrap_or(&f.unit_file)
                .to_string();
            (name, f.state)
        })
        .collect();
    units
        .into_iter()
        .filter(|u| u.load != "not-found")
        .map(|u| Service {
            enabled: states.get(&u.unit).cloned(),
            scope: scope.into(),
            unit: u.unit,
            description: u.description,
            load: u.load,
            active: u.active,
            sub: u.sub,
        })
        .collect()
}

/// The command line for a control action (after validation).
pub fn control_command(scope: &str, unit: &str, action: &str) -> AppResult<String> {
    validate_unit(unit)?;
    if !ACTIONS.contains(&action) {
        return Err(AppError::InvalidInput(format!(
            "unknown action `{action}` (use {})",
            ACTIONS.join(", ")
        )));
    }
    match scope {
        "user" => Ok(format!("systemctl --user {action} {unit}")),
        "system" => Ok(format!("systemctl {action} {unit}")),
        _ => Err(AppError::InvalidInput(
            "scope must be `system` or `user`".into(),
        )),
    }
}

/// Start/stop/… a unit through the full policy pipeline.
pub async fn control<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    scope: &str,
    unit: &str,
    action: &str,
    source: Source,
) -> AppResult<ExecResult> {
    let command = control_command(scope, unit, action)?;
    executor::execute(
        app,
        state,
        ExecRequest {
            command,
            cwd: None,
            source,
        },
    )
    .await
}

/// Recent journal lines for a unit (read-only; audited by the executor).
pub async fn logs<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    scope: &str,
    unit: &str,
    lines: u32,
    source: Source,
) -> AppResult<String> {
    validate_unit(unit)?;
    let n = lines.clamp(10, 1000);
    let command = match scope {
        "user" => format!("journalctl --user -u {unit} -n {n} --no-pager -o short-iso"),
        "system" => format!("journalctl -u {unit} -n {n} --no-pager -o short-iso"),
        _ => {
            return Err(AppError::InvalidInput(
                "scope must be `system` or `user`".into(),
            ))
        }
    };
    let r = executor::execute(
        app,
        state,
        ExecRequest {
            command,
            cwd: None,
            source,
        },
    )
    .await?;
    Ok(if r.stdout.trim().is_empty() {
        r.stderr
    } else {
        r.stdout
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_names_are_validated() {
        assert!(validate_unit("ollama.service").is_ok());
        assert!(validate_unit("getty@tty1.service").is_ok());
        assert!(validate_unit("omnix-kb-core.service").is_ok());
        for bad in [
            "ollama",
            "x.service; rm -rf /",
            "$(id).service",
            "-evil.service",
            ".service",
            "a b.service",
            "../x.service",
        ] {
            assert!(validate_unit(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn control_commands() {
        assert_eq!(
            control_command("user", "omnix-kb-core.service", "restart").expect("ok"),
            "systemctl --user restart omnix-kb-core.service"
        );
        assert_eq!(
            control_command("system", "ollama.service", "stop").expect("ok"),
            "systemctl stop ollama.service"
        );
        assert!(control_command("system", "ollama.service", "mask").is_err());
        assert!(control_command("root", "ollama.service", "stop").is_err());
    }

    #[test]
    fn control_commands_are_mutating_not_denied() {
        use crate::security::policy::{classify, PolicyConfig, RiskTier};
        let cfg = PolicyConfig::default();
        let c = classify(
            &control_command("user", "a.service", "restart").expect("ok"),
            &cfg,
        );
        assert_eq!(c.tier, RiskTier::Mutating);
        let c = classify(
            "journalctl --user -u a.service -n 50 --no-pager -o short-iso",
            &cfg,
        );
        assert_eq!(c.tier, RiskTier::ReadOnly);
    }

    #[test]
    fn merges_unit_files_and_ranks_failed_first() {
        let units: Vec<UnitRow> = serde_json::from_str(
            r#"[{"unit":"b.service","load":"loaded","active":"active","sub":"running","description":"B"},
                {"unit":"a.service","load":"loaded","active":"failed","sub":"failed","description":"A"},
                {"unit":"gone.service","load":"not-found","active":"inactive","sub":"dead","description":""}]"#,
        )
        .expect("json");
        let files: Vec<UnitFileRow> = serde_json::from_str(
            r#"[{"unit_file":"/usr/lib/systemd/system/b.service","state":"enabled"}]"#,
        )
        .expect("json");
        let mut v = merge(units, files, "system");
        v.sort_by_key(|a| rank(&a.active));
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].unit, "a.service");
        assert_eq!(v[1].enabled.as_deref(), Some("enabled"));
    }
}
