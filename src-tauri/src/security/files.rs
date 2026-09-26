//! Guarded file access used by `/file` chat commands (and, later, LLM tools).
//!
//! These are **not** exposed as raw Tauri commands: the webview cannot read or
//! write arbitrary paths directly. Every call:
//!
//! 1. resolves the path (expand `~`, require absolute, canonicalize to follow
//!    symlinks) so `~/notes -> ~/.ssh` style links cannot bypass checks;
//! 2. refuses credential locations (`policy::is_sensitive_path`) and, for
//!    writes, OMNIX-protected files (audit log, settings);
//! 3. enforces size limits;
//! 4. asks for native confirmation for writes (and for reads when
//!    `require_confirmation` is on);
//! 5. writes an audit entry.

use crate::error::{AppError, AppResult};
use crate::security::audit::{AuditRecord, Confirmation, Decision};
use crate::security::confirm::{self, ConfirmRequest};
use crate::security::executor::expand_home;
use crate::security::policy::{self, RiskTier, Source};
use crate::state::AppState;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Runtime};

/// Largest file `read_file` will return.
pub const MAX_READ_BYTES: u64 = 5 * 1024 * 1024;
/// Largest content `write_file` accepts.
pub const MAX_WRITE_BYTES: usize = 5 * 1024 * 1024;
/// Maximum entries returned by `list_directory`.
pub const MAX_LIST_ENTRIES: usize = 5000;

/// Read a UTF-8 text file.
pub async fn read_file<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    path: &str,
    source: Source,
) -> AppResult<String> {
    let started = Instant::now();
    let resolved = resolve_existing(path, state.home.as_deref());
    let rec = base_record("read_file", path, source);
    let p = match resolved.and_then(|p| guard_read(state, p)) {
        Ok(p) => p,
        Err(e) => return deny(state, rec, e).await,
    };
    let meta = tokio::fs::metadata(&p).await?;
    if !meta.is_file() {
        return Err(AppError::InvalidInput(format!(
            "{} is not a file",
            p.display()
        )));
    }
    if meta.len() > MAX_READ_BYTES {
        return Err(AppError::InvalidInput(format!(
            "{} is larger than {} MiB",
            p.display(),
            MAX_READ_BYTES / 1024 / 1024
        )));
    }
    let confirmation = confirm_read_if_configured(app, state, &p, source, "Read file?").await?;
    let content = tokio::fs::read_to_string(&p).await.map_err(|e| {
        AppError::InvalidInput(format!("cannot read {} as UTF-8 text: {e}", p.display()))
    })?;
    state
        .audit
        .record(AuditRecord {
            command: p.display().to_string(),
            decision: Decision::Allowed,
            confirmation,
            duration_ms: Some(elapsed_ms(started)),
            ..rec
        })
        .await?;
    Ok(content)
}

/// List a directory (names only, sorted, directories suffixed with `/`).
pub async fn list_directory<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    path: &str,
    source: Source,
) -> AppResult<Vec<String>> {
    let started = Instant::now();
    let rec = base_record("list_directory", path, source);
    let p = match resolve_existing(path, state.home.as_deref()).and_then(|p| guard_read(state, p)) {
        Ok(p) => p,
        Err(e) => return deny(state, rec, e).await,
    };
    let confirmation =
        confirm_read_if_configured(app, state, &p, source, "List directory?").await?;
    let mut entries = tokio::fs::read_dir(&p).await?;
    let mut names = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        if names.len() >= MAX_LIST_ENTRIES {
            names.push(format!("…[more than {MAX_LIST_ENTRIES} entries]"));
            break;
        }
        let mut name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
            name.push('/');
        }
        names.push(name);
    }
    names.sort();
    state
        .audit
        .record(AuditRecord {
            command: p.display().to_string(),
            decision: Decision::Allowed,
            confirmation,
            duration_ms: Some(elapsed_ms(started)),
            ..rec
        })
        .await?;
    Ok(names)
}

/// Create or overwrite a file. Always requires native confirmation.
pub async fn write_file<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    path: &str,
    content: &str,
    source: Source,
) -> AppResult<()> {
    let started = Instant::now();
    let rec = AuditRecord {
        tier: RiskTier::Mutating,
        ..base_record("write_file", path, source)
    };
    if content.len() > MAX_WRITE_BYTES {
        return Err(AppError::InvalidInput(format!(
            "content is larger than {} MiB",
            MAX_WRITE_BYTES / 1024 / 1024
        )));
    }
    let target =
        match resolve_for_write(path, state.home.as_deref()).and_then(|p| guard_write(state, p)) {
            Ok(p) => p,
            Err(e) => return deny(state, rec, e).await,
        };
    let exists = target.exists();
    let preview: String = content
        .chars()
        .take(300)
        .map(|c| if c.is_control() && c != '\n' { '·' } else { c })
        .collect();
    let settings = state.settings.read().await.clone();
    let confirmation = confirm::ask(
        app,
        &ConfirmRequest {
            title: if exists {
                "Overwrite file?"
            } else {
                "Create file?"
            }
            .into(),
            subject: format!("File:\n{}", target.display()),
            details: vec![
                format!("Size: {} bytes", content.len()),
                format!(
                    "Preview:\n{preview}{}",
                    if content.chars().count() > 300 {
                        "…"
                    } else {
                        ""
                    }
                ),
            ],
            tier: RiskTier::Mutating,
            source,
            reasons: vec![if exists {
                "replaces the existing file contents".into()
            } else {
                "creates a new file".into()
            }],
            approve_label: "Write".into(),
        },
        Duration::from_secs(settings.security.confirmation_timeout_secs),
    )
    .await;
    if confirmation != Confirmation::Approved {
        state
            .audit
            .record(AuditRecord {
                command: target.display().to_string(),
                decision: Decision::NotApproved,
                confirmation,
                ..rec
            })
            .await?;
        return Err(AppError::NotApproved("file write was not approved".into()));
    }
    let result = tokio::fs::write(&target, content).await;
    state
        .audit
        .record(AuditRecord {
            command: target.display().to_string(),
            decision: if result.is_ok() {
                Decision::Allowed
            } else {
                Decision::Failed
            },
            confirmation,
            duration_ms: Some(elapsed_ms(started)),
            detail: result.as_ref().err().map(ToString::to_string),
            ..rec
        })
        .await?;
    Ok(result?)
}

/// Write a *private* (mode 600) file at a path the user chose in a native
/// save dialog. The dialog is the confirmation, so none is shown here; the
/// credential/protected-path policy still applies and the write is audited
/// as `action`. Used for memory exports, which contain personal data.
pub async fn write_user_chosen_private(
    state: &AppState,
    path: &Path,
    content: &str,
    action: &str,
) -> AppResult<()> {
    let started = Instant::now();
    let shown = path.display().to_string();
    let rec = AuditRecord {
        tier: RiskTier::Mutating,
        ..base_record(action, &shown, Source::User)
    };
    let target = match resolve_for_write(&shown, state.home.as_deref())
        .and_then(|p| guard_write(state, p))
    {
        Ok(p) => p,
        Err(e) => return deny(state, rec, e).await,
    };
    let data = content.as_bytes().to_vec();
    let dest = target.clone();
    let result = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&dest)?;
        f.write_all(&data)?;
        f.sync_all()
    })
    .await?;
    state
        .audit
        .record(AuditRecord {
            command: target.display().to_string(),
            decision: if result.is_ok() {
                Decision::Allowed
            } else {
                Decision::Failed
            },
            confirmation: Confirmation::Approved,
            duration_ms: Some(elapsed_ms(started)),
            detail: Some(match &result {
                Ok(()) => format!(
                    "{} bytes; path chosen in a native save dialog",
                    content.len()
                ),
                Err(e) => e.to_string(),
            }),
            ..rec
        })
        .await?;
    Ok(result?)
}

/// Read a UTF-8 file the user chose in a native open dialog, up to
/// `max_bytes`, under the credential-path policy; audited as `action`.
pub async fn read_user_chosen(
    state: &AppState,
    path: &Path,
    max_bytes: u64,
    action: &str,
) -> AppResult<String> {
    let started = Instant::now();
    let shown = path.display().to_string();
    let rec = base_record(action, &shown, Source::User);
    let p = match resolve_existing(&shown, state.home.as_deref()).and_then(|p| guard_read(state, p))
    {
        Ok(p) => p,
        Err(e) => return deny(state, rec, e).await,
    };
    let meta = tokio::fs::metadata(&p).await?;
    if !meta.is_file() || meta.len() > max_bytes {
        return Err(AppError::InvalidInput(format!(
            "{} is not a file of at most {} MiB",
            p.display(),
            max_bytes / 1024 / 1024
        )));
    }
    let content = tokio::fs::read_to_string(&p).await.map_err(|e| {
        AppError::InvalidInput(format!("cannot read {} as UTF-8 text: {e}", p.display()))
    })?;
    state
        .audit
        .record(AuditRecord {
            command: p.display().to_string(),
            decision: Decision::Allowed,
            confirmation: Confirmation::Approved,
            duration_ms: Some(elapsed_ms(started)),
            detail: Some("path chosen in a native open dialog".into()),
            ..rec
        })
        .await?;
    Ok(content)
}

fn base_record(action: &str, path: &str, source: Source) -> AuditRecord {
    AuditRecord {
        id: uuid::Uuid::new_v4().to_string(),
        source,
        action: action.into(),
        command: path.into(),
        cwd: None,
        tier: RiskTier::ReadOnly,
        decision: Decision::Denied,
        confirmation: Confirmation::Skipped,
        exit_code: None,
        duration_ms: None,
        detail: None,
    }
}

/// Audit a refusal and return the error.
async fn deny<T>(state: &AppState, rec: AuditRecord, e: AppError) -> AppResult<T> {
    let tier = if matches!(e, AppError::PolicyDenied(_)) {
        RiskTier::Denied
    } else {
        rec.tier
    };
    state
        .audit
        .record(AuditRecord {
            tier,
            decision: Decision::Denied,
            detail: Some(e.to_string()),
            ..rec
        })
        .await?;
    Err(e)
}

async fn confirm_read_if_configured<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    p: &Path,
    source: Source,
    title: &str,
) -> AppResult<Confirmation> {
    let settings = state.settings.read().await.clone();
    if !settings.security.require_confirmation {
        return Ok(Confirmation::NotRequired);
    }
    let c = confirm::ask(
        app,
        &ConfirmRequest {
            title: title.into(),
            subject: format!("Path:\n{}", p.display()),
            details: vec![],
            tier: RiskTier::ReadOnly,
            source,
            reasons: vec!["read confirmation is enabled in Settings → Security".into()],
            approve_label: "Allow".into(),
        },
        Duration::from_secs(settings.security.confirmation_timeout_secs),
    )
    .await;
    if c == Confirmation::Approved {
        Ok(c)
    } else {
        Err(AppError::NotApproved("read was not approved".into()))
    }
}

/// Expand `~`, require an absolute path and canonicalize (must exist).
fn resolve_existing(path: &str, home: Option<&Path>) -> AppResult<PathBuf> {
    let p = expand_home(path.trim(), home);
    if !p.is_absolute() {
        return Err(AppError::InvalidInput(
            "path must be absolute (or start with ~/)".into(),
        ));
    }
    std::fs::canonicalize(&p).map_err(|e| AppError::InvalidInput(format!("{}: {e}", p.display())))
}

/// Resolve a write target whose file may not exist yet: canonicalize the
/// parent (must exist) and, if the file exists, the file itself (follows a
/// symlink so the real target is checked).
fn resolve_for_write(path: &str, home: Option<&Path>) -> AppResult<PathBuf> {
    let p = expand_home(path.trim(), home);
    if !p.is_absolute() {
        return Err(AppError::InvalidInput(
            "path must be absolute (or start with ~/)".into(),
        ));
    }
    if p.exists() {
        return std::fs::canonicalize(&p)
            .map_err(|e| AppError::InvalidInput(format!("{}: {e}", p.display())));
    }
    let name = p
        .file_name()
        .ok_or_else(|| AppError::InvalidInput("path has no file name".into()))?;
    let parent = p
        .parent()
        .ok_or_else(|| AppError::InvalidInput("path has no parent directory".into()))?;
    let parent = std::fs::canonicalize(parent).map_err(|e| {
        AppError::InvalidInput(format!("parent directory {}: {e}", parent.display()))
    })?;
    Ok(parent.join(name))
}

fn guard_read(state: &AppState, p: PathBuf) -> AppResult<PathBuf> {
    let home = state
        .home
        .as_deref()
        .map(|h| h.to_string_lossy().to_string());
    if policy::is_sensitive_path(&p.to_string_lossy(), home.as_deref()) {
        return Err(AppError::PolicyDenied(format!(
            "{} is a credential location",
            p.display()
        )));
    }
    Ok(p)
}

fn guard_write(state: &AppState, p: PathBuf) -> AppResult<PathBuf> {
    let p = guard_read(state, p)?;
    let home = state
        .home
        .as_deref()
        .map(|h| h.to_string_lossy().to_string());
    if policy::is_protected_path(
        &p.to_string_lossy(),
        &state.protected_paths(),
        home.as_deref(),
    ) {
        return Err(AppError::PolicyDenied(format!(
            "{} is protected by OMNIX (audit log / settings)",
            p.display()
        )));
    }
    Ok(p)
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn write_resolution_follows_symlinks() {
        let d = tempfile::tempdir().expect("tempdir");
        let real = d.path().join("real.txt");
        std::fs::write(&real, "x").expect("write");
        let link = d.path().join("link.txt");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");
        let r = resolve_for_write(&link.to_string_lossy(), None).expect("resolve");
        assert_eq!(r, std::fs::canonicalize(&real).expect("canon"));
    }

    #[test]
    fn relative_paths_rejected() {
        assert!(resolve_existing("notes.txt", None).is_err());
        assert!(resolve_for_write("notes.txt", None).is_err());
    }

    #[test]
    fn new_file_under_existing_parent() {
        let d = tempfile::tempdir().expect("tempdir");
        let target = d.path().join("new.txt");
        let r = resolve_for_write(&target.to_string_lossy(), None).expect("resolve");
        assert_eq!(r.file_name().and_then(|n| n.to_str()), Some("new.txt"));
        assert!(
            resolve_for_write(&d.path().join("missing/new.txt").to_string_lossy(), None).is_err()
        );
    }
}
