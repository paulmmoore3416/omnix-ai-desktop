//! Disk cleanup: find reclaimable space and free it safely.
//!
//! Two kinds of category:
//!
//! * **Cache directories** under the user's home that applications rebuild
//!   on demand (trash, thumbnails, pip/npm/yarn/cargo download caches). OMNIX
//!   deletes their *contents* itself, only inside this fixed allowlist,
//!   removing symlinks as links (never following them), after one native
//!   confirmation listing every category and size. Each category is audited.
//! * **Tool commands** (`docker image prune -f`, `docker builder prune -f`,
//!   `journalctl --user --vacuum-time=14d`): run through the executor, so they
//!   get the normal policy classification, confirmation and audit.

use super::probe;
use crate::error::{AppError, AppResult};
use crate::security::audit::{AuditRecord, Decision};
use crate::security::confirm::{self, ConfirmRequest};
use crate::security::executor::{self, ExecRequest};
use crate::security::policy::{RiskTier, Source};
use crate::state::AppState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Runtime};

/// Stop sizing a directory after this many entries (keeps scans fast).
const MAX_WALK: usize = 400_000;

/// A cleanup category.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CleanupItem {
    /// Stable id.
    pub id: String,
    /// Label.
    pub label: String,
    /// What it is and what happens after cleaning.
    pub description: String,
    /// Reclaimable bytes (estimate for commands).
    pub bytes: u64,
    /// `files` or `command`.
    pub kind: String,
    /// The command that will run (`command` kind).
    pub command: Option<String>,
    /// Size is a lower bound (scan was capped).
    pub partial: bool,
}

struct DirCategory {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    rel: &'static [&'static str],
}

const DIRS: &[DirCategory] = &[
    DirCategory {
        id: "trash",
        label: "Trash",
        description: "Files you already deleted to the Trash. Emptying is permanent.",
        rel: &[".local/share/Trash/files", ".local/share/Trash/info"],
    },
    DirCategory {
        id: "thumbnails",
        label: "Thumbnail cache",
        description: "Image previews; regenerated when you browse folders.",
        rel: &[".cache/thumbnails"],
    },
    DirCategory {
        id: "pip",
        label: "pip download cache",
        description: "Downloaded Python packages; re-downloaded when needed.",
        rel: &[".cache/pip"],
    },
    DirCategory {
        id: "npm",
        label: "npm cache",
        description: "Downloaded npm packages; re-downloaded when needed.",
        rel: &[".npm/_cacache"],
    },
    DirCategory {
        id: "yarn",
        label: "Yarn cache",
        description: "Downloaded Yarn packages; re-downloaded when needed.",
        rel: &[".cache/yarn"],
    },
    DirCategory {
        id: "cargo",
        label: "Cargo download cache",
        description: "Downloaded Rust crate archives; re-downloaded when needed.",
        rel: &[".cargo/registry/cache"],
    },
];

fn dir_size(p: &Path) -> (u64, bool) {
    let mut total = 0u64;
    let mut n = 0usize;
    let mut stack = vec![p.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            n += 1;
            if n > MAX_WALK {
                return (total, true);
            }
            let Ok(m) = e.path().symlink_metadata() else {
                continue;
            };
            if m.is_dir() {
                stack.push(e.path());
            } else {
                total += m.len();
            }
        }
    }
    (total, false)
}

fn dirs_for(home: &Path, c: &DirCategory) -> Vec<PathBuf> {
    c.rel
        .iter()
        .map(|r| home.join(r))
        .filter(|p| p.is_dir())
        .collect()
}

/// Scan all categories.
pub async fn scan(state: &AppState) -> AppResult<Vec<CleanupItem>> {
    let home = state
        .home
        .clone()
        .ok_or_else(|| AppError::Unavailable("no home directory".into()))?;
    let mut items = tokio::task::spawn_blocking(move || {
        DIRS.iter()
            .filter_map(|c| {
                let dirs = dirs_for(&home, c);
                if dirs.is_empty() {
                    return None;
                }
                let (mut bytes, mut partial) = (0, false);
                for d in &dirs {
                    let (b, p) = dir_size(d);
                    bytes += b;
                    partial |= p;
                }
                Some(CleanupItem {
                    id: c.id.into(),
                    label: c.label.into(),
                    description: c.description.into(),
                    bytes,
                    kind: "files".into(),
                    command: None,
                    partial,
                })
            })
            .collect::<Vec<_>>()
    })
    .await?;

    if probe::available("docker") {
        if let Some(df) = probe::run(
            "docker",
            &["system", "df", "--format", "{{json .}}"],
            Duration::from_secs(15),
        )
        .await
        {
            for (ty, id, label, cmd, desc) in [
                (
                    "Images",
                    "docker_images",
                    "Unused Docker images (dangling)",
                    "docker image prune -f",
                    "Untagged image layers no container uses.",
                ),
                (
                    "Build Cache",
                    "docker_build",
                    "Docker build cache",
                    "docker builder prune -f",
                    "Cached build layers; rebuilt on the next build.",
                ),
            ] {
                if let Some(bytes) = docker_reclaimable(&df, ty) {
                    if bytes > 0 {
                        items.push(CleanupItem {
                            id: id.into(),
                            label: label.into(),
                            description: desc.into(),
                            bytes,
                            kind: "command".into(),
                            command: Some(cmd.into()),
                            partial: ty == "Images", // df counts all unused, prune removes dangling only
                        });
                    }
                }
            }
        }
    }
    if let Some(out) = probe::run(
        "journalctl",
        &["--user", "--disk-usage"],
        Duration::from_secs(10),
    )
    .await
    {
        if let Some(bytes) = parse_journal_usage(&out) {
            if bytes > 64 * 1024 * 1024 {
                items.push(CleanupItem {
                    id: "journal_user".into(),
                    label: "Your system journal (older than 14 days)".into(),
                    description: "Log entries of your user services older than two weeks.".into(),
                    bytes,
                    kind: "command".into(),
                    command: Some("journalctl --user --vacuum-time=14d".into()),
                    partial: true,
                });
            }
        }
    }
    items.sort_by_key(|a| std::cmp::Reverse(a.bytes));
    Ok(items)
}

/// Parse sizes like `1.5GB`, `230.4MB`, `12kB`, `0B`.
pub fn parse_size(s: &str) -> Option<u64> {
    let s = s.split_whitespace().next()?;
    let idx = s.find(|c: char| c.is_ascii_alphabetic())?;
    let (n, unit) = s.split_at(idx);
    let n: f64 = n.parse().ok()?;
    let mult = match unit.to_ascii_uppercase().as_str() {
        "B" => 1.0,
        "KB" | "K" => 1e3,
        "MB" | "M" => 1e6,
        "GB" | "G" => 1e9,
        "TB" | "T" => 1e12,
        "KIB" => 1024.0,
        "MIB" => 1024.0 * 1024.0,
        "GIB" => 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((n * mult) as u64)
}

fn docker_reclaimable(df: &str, ty: &str) -> Option<u64> {
    df.lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .find(|v| v.get("Type").and_then(|t| t.as_str()) == Some(ty))
        .and_then(|v| {
            v.get("Reclaimable")
                .and_then(|r| r.as_str())
                .and_then(parse_size)
        })
}

/// `Archived and active journals take up 1.2G in the file system.`
pub fn parse_journal_usage(s: &str) -> Option<u64> {
    let i = s.find("take up ")? + "take up ".len();
    parse_size(&s[i..])
}

/// Result per category.
#[derive(Debug, Clone, Serialize)]
pub struct CleanupResult {
    /// Category id.
    pub id: String,
    /// Succeeded.
    pub ok: bool,
    /// Bytes freed (measured for files, estimated for commands).
    pub freed: u64,
    /// Message.
    pub message: String,
}

/// Delete the contents of `dir` without following symlinks. Returns bytes freed.
fn empty_dir(dir: &Path) -> (u64, Vec<String>) {
    let mut freed = 0;
    let mut errors = vec![];
    let Ok(rd) = std::fs::read_dir(dir) else {
        return (0, vec![format!("cannot read {}", dir.display())]);
    };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(m) = p.symlink_metadata() else {
            continue;
        };
        let r = if m.is_dir() {
            let (b, _) = dir_size(&p);
            std::fs::remove_dir_all(&p).map(|_| b)
        } else {
            // Files and symlinks (the link itself, never its target).
            std::fs::remove_file(&p).map(|_| m.len())
        };
        match r {
            Ok(b) => freed += b,
            Err(e) => errors.push(format!("{}: {e}", p.display())),
        }
    }
    (freed, errors)
}

/// Clean the selected categories.
pub async fn run<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    ids: &[String],
) -> AppResult<Vec<CleanupResult>> {
    let items = scan(state).await?;
    let chosen: Vec<CleanupItem> = items.into_iter().filter(|i| ids.contains(&i.id)).collect();
    if chosen.is_empty() {
        return Err(AppError::InvalidInput("nothing selected to clean".into()));
    }
    let home = state
        .home
        .clone()
        .ok_or_else(|| AppError::Unavailable("no home directory".into()))?;
    let mut results = vec![];

    let files: Vec<&CleanupItem> = chosen.iter().filter(|i| i.kind == "files").collect();
    if !files.is_empty() {
        let total: u64 = files.iter().map(|i| i.bytes).sum();
        let c = confirm::require(
            app,
            state,
            &ConfirmRequest {
                title: "Clean up disk space?".into(),
                subject: files
                    .iter()
                    .map(|i| format!("• {} ({:.1} MB)", i.label, i.bytes as f64 / 1e6))
                    .collect::<Vec<_>>()
                    .join("\n"),
                details: vec![format!(
                    "Frees about {:.2} GB. Deleted files can't be restored.",
                    total as f64 / 1e9
                )],
                tier: RiskTier::Mutating,
                source: Source::User,
                reasons: vec!["permanently deletes cached and trashed files".into()],
                approve_label: "Clean".into(),
            },
        )
        .await;
        match c {
            Err(c) => {
                state
                    .audit
                    .record(AuditRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        source: Source::User,
                        action: "cleanup".into(),
                        command: files
                            .iter()
                            .map(|i| i.id.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                        cwd: None,
                        tier: RiskTier::Mutating,
                        decision: Decision::NotApproved,
                        confirmation: c,
                        exit_code: None,
                        duration_ms: None,
                        detail: None,
                    })
                    .await?;
                return Err(AppError::NotApproved("cleanup was not approved".into()));
            }
            Ok(c) => {
                for item in files {
                    let cat = DIRS
                        .iter()
                        .find(|d| d.id == item.id)
                        .expect("scan only returns known ids");
                    let dirs = dirs_for(&home, cat);
                    let t = Instant::now();
                    let (freed, errors) = tokio::task::spawn_blocking(move || {
                        let mut f = 0;
                        let mut errs = vec![];
                        for d in dirs {
                            let (b, e) = empty_dir(&d);
                            f += b;
                            errs.extend(e);
                        }
                        (f, errs)
                    })
                    .await?;
                    let ok = errors.is_empty();
                    state
                        .audit
                        .record(AuditRecord {
                            id: uuid::Uuid::new_v4().to_string(),
                            source: Source::User,
                            action: "cleanup".into(),
                            command: cat
                                .rel
                                .iter()
                                .map(|r| format!("~/{r}/*"))
                                .collect::<Vec<_>>()
                                .join(" "),
                            cwd: None,
                            tier: RiskTier::Mutating,
                            decision: if ok {
                                Decision::Allowed
                            } else {
                                Decision::Failed
                            },
                            confirmation: c,
                            exit_code: None,
                            duration_ms: Some(t.elapsed().as_millis() as u64),
                            detail: Some(format!(
                                "freed {freed} bytes{}",
                                if ok {
                                    String::new()
                                } else {
                                    format!("; {} errors", errors.len())
                                }
                            )),
                        })
                        .await?;
                    results.push(CleanupResult {
                        id: item.id.clone(),
                        ok,
                        freed,
                        message: if ok {
                            "cleaned".into()
                        } else {
                            errors.into_iter().take(3).collect::<Vec<_>>().join("; ")
                        },
                    });
                }
            }
        }
    }
    for item in chosen.iter().filter(|i| i.kind == "command") {
        let cmd = item.command.clone().unwrap_or_default();
        let r = executor::execute(
            app,
            state,
            ExecRequest {
                command: cmd,
                cwd: None,
                source: Source::User,
            },
        )
        .await;
        results.push(match r {
            Ok(r) => CleanupResult {
                id: item.id.clone(),
                ok: r.exit_code == Some(0),
                freed: if r.exit_code == Some(0) {
                    item.bytes
                } else {
                    0
                },
                message: r.stdout.lines().last().unwrap_or("done").to_string(),
            },
            Err(e) => CleanupResult {
                id: item.id.clone(),
                ok: false,
                freed: 0,
                message: e.to_string(),
            },
        });
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("1.5GB"), Some(1_500_000_000));
        assert_eq!(parse_size("230.4MB (12%)"), Some(230_400_000));
        assert_eq!(parse_size("0B"), Some(0));
        assert_eq!(parse_size("12kB"), Some(12_000));
        assert_eq!(parse_size("2GiB"), Some(2 * 1024 * 1024 * 1024));
        assert_eq!(parse_size("lots"), None);
        assert_eq!(
            parse_journal_usage("Archived and active journals take up 1.2G in the file system."),
            Some(1_200_000_000)
        );
        let df = r#"{"Reclaimable":"2.1GB (80%)","Size":"2.6GB","Type":"Images"}
{"Reclaimable":"512MB","Size":"512MB","Type":"Build Cache"}"#;
        assert_eq!(docker_reclaimable(df, "Build Cache"), Some(512_000_000));
    }

    #[test]
    #[cfg(unix)]
    fn empties_without_following_symlinks() {
        let dir = tempfile::tempdir().expect("tmp");
        let cache = dir.path().join("cache");
        let keep = dir.path().join("keep");
        std::fs::create_dir_all(cache.join("sub")).expect("mkdir");
        std::fs::create_dir_all(&keep).expect("mkdir");
        std::fs::write(cache.join("a"), vec![0u8; 100]).expect("w");
        std::fs::write(cache.join("sub").join("b"), vec![0u8; 50]).expect("w");
        std::fs::write(keep.join("precious"), b"x").expect("w");
        std::os::unix::fs::symlink(&keep, cache.join("link")).expect("link");
        let (freed, errs) = empty_dir(&cache);
        assert!(errs.is_empty(), "{errs:?}");
        assert!(freed >= 150);
        assert!(cache.is_dir(), "the directory itself stays");
        assert_eq!(std::fs::read_dir(&cache).expect("rd").count(), 0);
        assert!(keep.join("precious").exists(), "symlink target untouched");
    }
}
