//! Native confirmation dialogs raised from Rust.
//!
//! JavaScript `confirm()` is not a security control: a compromised or
//! prompt-injected webview can simply skip it. These dialogs are created by
//! the Rust process through `tauri-plugin-dialog`, and their text is built
//! here from the *parsed* request, so the webview cannot alter what is shown.
//!
//! Rules:
//! * **Default deny.** Only an explicit press of the approve button counts as
//!   approval. Cancel, closing the window, a dialog error, or a timeout all deny.
//! * **One at a time.** Dialogs are serialized so a flood of requests cannot
//!   stack dialogs and trick the user into approving the wrong one.

use crate::security::audit::Confirmation;
use crate::security::policy::{RiskTier, Source};
use std::sync::LazyLock;
use std::time::Duration;
use tauri::{AppHandle, Runtime};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

/// Serializes dialogs across the whole app.
static DIALOG_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));

/// Describes what the user is being asked to approve.
#[derive(Debug, Clone)]
pub struct ConfirmRequest {
    /// Dialog title, e.g. "Run command?".
    pub title: String,
    /// What will happen, e.g. the exact command line.
    pub subject: String,
    /// Extra context lines (working directory, file size, ...).
    pub details: Vec<String>,
    /// Policy tier (shown prominently).
    pub tier: RiskTier,
    /// Who asked.
    pub source: Source,
    /// Why the policy engine flagged it.
    pub reasons: Vec<String>,
    /// Label of the approve button (e.g. "Run", "Kill", "Save").
    pub approve_label: String,
}

impl ConfirmRequest {
    /// Render the dialog body. Kept separate for testing.
    pub fn body(&self) -> String {
        let mut s = format!(
            "Risk tier: {}\nRequested by: {}\n\n{}\n",
            self.tier, self.source, self.subject
        );
        for d in &self.details {
            s.push_str(&format!("\n{d}"));
        }
        if !self.reasons.is_empty() {
            s.push_str("\n\nWhy this needs approval:");
            for r in &self.reasons {
                s.push_str(&format!("\n  • {r}"));
            }
        }
        if self.source == Source::LlmTool {
            s.push_str(
                "\n\n⚠ This was proposed by the AI assistant. Only approve if you asked for it.",
            );
        }
        s.push_str("\n\nIf you do not recognise this request, choose Cancel.");
        s
    }
}

/// Show a native OK/Cancel dialog and wait up to `timeout` for an answer.
///
/// Returns [`Confirmation::Approved`] only on an explicit approve click.
pub async fn ask<R: Runtime>(
    app: &AppHandle<R>,
    req: &ConfirmRequest,
    timeout: Duration,
) -> Confirmation {
    let _guard = DIALOG_LOCK.lock().await;
    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();

    let kind = match req.tier {
        RiskTier::ReadOnly => MessageDialogKind::Info,
        _ => MessageDialogKind::Warning,
    };
    app.dialog()
        .message(req.body())
        .title(format!("OMNIX — {}", req.title))
        .kind(kind)
        .buttons(MessageDialogButtons::OkCancelCustom(
            req.approve_label.clone(),
            "Cancel".into(),
        ))
        .show(move |approved| {
            // The receiver may already be gone (timed out); ignoring the send
            // error is correct: a late approval must not count.
            let _ = tx.send(approved);
        });

    match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(true)) => Confirmation::Approved,
        Ok(Ok(false)) | Ok(Err(_)) => Confirmation::Declined,
        Err(_) => {
            tracing::warn!(title = %req.title, "confirmation dialog timed out; treating as deny");
            Confirmation::TimedOut
        }
    }
}

/// Ask with the configured timeout; `Ok(Approved)` only when the user
/// approved, otherwise `Err(NotApproved)` carrying `what` (after the caller's
/// audit, if any). Convenience for actions that aren't shell commands
/// (model deletion, cleanup, unattended-command approval, …).
pub async fn require<R: Runtime>(
    app: &AppHandle<R>,
    state: &crate::state::AppState,
    req: &ConfirmRequest,
) -> Result<Confirmation, Confirmation> {
    let timeout = state
        .settings
        .read()
        .await
        .security
        .confirmation_timeout_secs;
    let c = ask(app, req, Duration::from_secs(timeout)).await;
    if c == Confirmation::Approved {
        Ok(c)
    } else {
        Err(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_shows_command_tier_and_ai_warning() {
        let r = ConfirmRequest {
            title: "Run command?".into(),
            subject: "Command:\nrm notes.txt".into(),
            details: vec!["Working directory: /home/u".into()],
            tier: RiskTier::Mutating,
            source: Source::LlmTool,
            reasons: vec!["`rm` deletes files".into()],
            approve_label: "Run".into(),
        };
        let b = r.body();
        assert!(b.contains("rm notes.txt"));
        assert!(b.contains("Mutating"));
        assert!(b.contains("/home/u"));
        assert!(b.contains("proposed by the AI assistant"));
    }
}
