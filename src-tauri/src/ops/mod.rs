//! Host operations: alerts, automations and scheduled tasks.
//!
//! * [`model`]: the rule types, persisted in `~/.config/omnix/ops.json`
//!   (mode 600, atomic writes, on the policy's protected-path list so no
//!   executed command can edit it).
//! * [`cron`]: schedule expressions.
//! * [`approval`]: signed one-time approvals for unattended commands.
//! * [`engine`]: the background loop (metrics sampling, alert evaluation,
//!   triggers, scheduler) and action runner.
//! * [`rules`]: create / update / delete with validation and native
//!   confirmation.
//!
//! Security summary (details in `docs/SECURITY.md` §14): rules are created
//! in the UI or proposed by the model; model proposals always need a native
//! confirmation. A command action is policy-classified when the rule is
//! created. `Denied` and `Privileged` commands are refused outright, and
//! anything that would need a confirmation needs a one-time native approval
//! bound by HMAC to the exact command. At run time the command is classified
//! again and executed through `executor::execute_preapproved`, and every run
//! is audited.

pub mod approval;
pub mod cron;
pub mod engine;
pub mod model;
pub mod rules;

use crate::error::{AppError, AppResult};
use model::{Activity, OpsFile};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Activity entries kept.
pub const ACTIVITY_KEEP: usize = 200;
/// Maximum rules of each kind.
pub const MAX_RULES: usize = 100;

/// `~/.config/omnix/ops.json`, next to `settings.json`.
pub fn default_path() -> AppResult<PathBuf> {
    crate::settings::default_path()?
        .parent()
        .map(|p| p.join("ops.json"))
        .ok_or_else(|| AppError::Internal("no config directory".into()))
}

/// Rules plus engine runtime state.
pub struct OpsState {
    path: PathBuf,
    data: Mutex<OpsFile>,
    /// Engine edge-detection state (not persisted).
    pub runtime: Mutex<engine::Runtime>,
    /// Bounds concurrently running actions.
    pub slots: tokio::sync::Semaphore,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl OpsState {
    /// Load `path`; a corrupt file is moved aside and replaced by an empty one.
    pub fn load(path: PathBuf) -> Self {
        let data = match std::fs::read_to_string(&path) {
            Ok(s) => match serde_json::from_str::<OpsFile>(&s) {
                Ok(f) => f,
                Err(e) => {
                    let aside = path.with_extension(format!(
                        "json.corrupt-{}",
                        chrono::Local::now().format("%Y%m%d%H%M%S")
                    ));
                    tracing::error!(error = %e, aside = %aside.display(), "ops.json is invalid; starting empty");
                    let _ = std::fs::rename(&path, &aside);
                    OpsFile::default()
                }
            },
            Err(_) => OpsFile::default(),
        };
        Self {
            path,
            data: Mutex::new(data),
            runtime: Mutex::new(engine::Runtime::default()),
            slots: tokio::sync::Semaphore::new(4),
        }
    }

    /// File location.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Copy of all rules and activity.
    pub fn snapshot(&self) -> OpsFile {
        lock(&self.data).clone()
    }

    /// Mutate and persist atomically. The closure's error aborts the save.
    pub fn update<T>(&self, f: impl FnOnce(&mut OpsFile) -> AppResult<T>) -> AppResult<T> {
        let mut g = lock(&self.data);
        let mut next = g.clone();
        let out = f(&mut next)?;
        next.version = 1;
        write_private(&self.path, &serde_json::to_vec_pretty(&next)?)?;
        *g = next;
        Ok(out)
    }

    /// Append to the activity feed (persisted, bounded).
    pub fn activity(&self, kind: &str, name: &str, ok: bool, summary: &str) -> Activity {
        let a = Activity {
            ts: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            kind: kind.into(),
            name: name.chars().take(120).collect(),
            ok,
            summary: summary.chars().take(500).collect(),
        };
        let entry = a.clone();
        if let Err(e) = self.update(|f| {
            f.activity.push(entry);
            let n = f.activity.len();
            if n > ACTIVITY_KEEP {
                f.activity.drain(..n - ACTIVITY_KEEP);
            }
            Ok(())
        }) {
            tracing::warn!(error = %e, "could not persist ops activity");
        }
        a
    }
}

/// Write `bytes` to `path` via a temp file + rename, mode 600.
fn write_private(path: &Path, bytes: &[u8]) -> AppResult<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_privately_and_survives_corruption() {
        let dir = tempfile::tempdir().expect("tmp");
        let p = dir.path().join("ops.json");
        let s = OpsState::load(p.clone());
        s.activity("test", "x", true, "hello");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&p).expect("meta").permissions().mode() & 0o777,
                0o600
            );
        }
        assert_eq!(OpsState::load(p.clone()).snapshot().activity.len(), 1);
        std::fs::write(&p, "{not json").expect("write");
        assert!(OpsState::load(p.clone()).snapshot().activity.is_empty());
        assert!(std::fs::read_dir(dir.path()).expect("dir").any(|e| e
            .expect("e")
            .file_name()
            .to_string_lossy()
            .contains("corrupt")));
    }

    #[test]
    fn activity_is_bounded() {
        let dir = tempfile::tempdir().expect("tmp");
        let s = OpsState::load(dir.path().join("ops.json"));
        for i in 0..(ACTIVITY_KEEP + 5) {
            s.activity("k", &format!("n{i}"), true, "");
        }
        let a = s.snapshot().activity;
        assert_eq!(a.len(), ACTIVITY_KEEP);
        assert_eq!(
            a.last().expect("last").name,
            format!("n{}", ACTIVITY_KEEP + 4)
        );
    }
}
