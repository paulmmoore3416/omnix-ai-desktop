//! Chat input router.
//!
//! Slash commands are handled locally; everything else goes to the configured
//! LLM. Every privileged action goes through the same policy/confirm/audit
//! pipeline as the dedicated commands. There is no canned "fallback" answer:
//! if the model is unreachable, the user gets an error.

use super::not_implemented;
use crate::ai::agent::{self, UiEvent};
use crate::error::{AppError, AppResult};
use crate::security::executor::{self, ExecRequest, ExecResult};
use crate::security::files;
use crate::security::policy::Source;
use crate::state::AppState;
use std::sync::atomic::Ordering;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

/// Send a chat message to the configured model. Tokens, tool calls/results
/// and notices stream over `on_event`; the returned future resolves when the
/// turn is complete. Errors are both returned and streamed as `error`.
#[tauri::command]
pub async fn chat_send(
    app: AppHandle,
    state: State<'_, AppState>,
    message: String,
    on_event: Channel<UiEvent>,
) -> AppResult<()> {
    let message = message.trim().to_string();
    if message.is_empty() {
        return Err(AppError::InvalidInput("message is empty".into()));
    }
    if message.len() > 100_000 {
        return Err(AppError::InvalidInput("message is too long".into()));
    }
    let emit = |ev: UiEvent| {
        // A closed channel (window reloaded) is not an error for the turn.
        if let Err(e) = on_event.send(ev) {
            tracing::debug!(error = %e, "chat channel closed");
        }
    };
    let result = agent::run_turn(&app, &state, message.clone(), &emit).await;
    if let Err(e) = &result {
        emit(UiEvent::Error {
            message: e.to_string(),
        });
    }
    let (capture, archive) = {
        let s = state.settings.read().await;
        let on = !s.memory.backend_url.trim().is_empty();
        (
            s.memory.auto_capture && on,
            s.memory.archive_conversations && on,
        )
    };
    if result.is_ok() && archive {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            if let Err(e) = agent::archive_conversation(&state).await {
                tracing::debug!(error = %e, "conversation archive failed");
            }
        });
    }
    if result.is_ok() && capture && agent::worth_capturing(&message) {
        // Runs after the reply is complete so it never delays it; results
        // are reported as a note on the same reply.
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let note = match agent::capture_facts(&state, &message).await {
                Ok(facts) => {
                    state.agent_metrics.capture(
                        facts
                            .iter()
                            .filter(|m| m.get("status").and_then(|s| s.as_str()) == Some("created"))
                            .count(),
                    );
                    let new: Vec<&str> = facts
                        .iter()
                        .filter(|m| m.get("status").and_then(|s| s.as_str()) == Some("created"))
                        .filter_map(|m| m.get("content").and_then(|c| c.as_str()))
                        .collect();
                    (!new.is_empty()).then(|| format!("🧠 Remembered: {}", new.join(" · ")))
                }
                Err(e) => {
                    tracing::warn!(error = %e, "fact capture failed");
                    None
                }
            };
            if let Some(message) = note {
                let _ = on_event.send(UiEvent::Notice { message });
            }
        });
    }
    result
}

/// Stop the current response (between stream events / before the next tool).
#[tauri::command]
pub async fn chat_cancel(state: State<'_, AppState>) -> AppResult<()> {
    state.chat_cancel.store(true, Ordering::SeqCst);
    Ok(())
}

/// Clear the conversation history.
#[tauri::command]
pub async fn chat_reset(state: State<'_, AppState>) -> AppResult<()> {
    state
        .conversation
        .try_lock()
        .map_err(|_| AppError::InvalidInput("a response is in progress; stop it first".into()))?
        .clear();
    // The next conversation gets its own archive document.
    if let Ok(mut doc) = state.conversation_doc.lock() {
        *doc = None;
    }
    Ok(())
}

/// Handle one line of chat input and return the assistant's reply text.
#[tauri::command]
pub async fn process_command(
    app: AppHandle,
    state: State<'_, AppState>,
    command: String,
) -> AppResult<String> {
    let input = command.trim();
    if let Some(rest) = slash(input, "/execute") {
        let r = executor::execute(
            &app,
            &state,
            ExecRequest {
                command: rest.to_string(),
                cwd: None,
                source: Source::User,
            },
        )
        .await?;
        return Ok(format_exec(&r));
    }
    if let Some(rest) = slash(input, "/file") {
        return file_op(&app, &state, rest).await;
    }
    if slash(input, "/search").is_some() {
        return not_implemented("/search");
    }
    if slash(input, "/monitor").is_some() {
        return monitor(&state);
    }
    let word = input.split_whitespace().next().unwrap_or(input).to_string();
    Err(AppError::InvalidInput(format!(
        "unknown command {word}. Available: /execute, /file, /monitor (plain text goes to chat)"
    )))
}

/// Match `/name` or `/name <rest>` (word boundary), returning `<rest>`.
fn slash<'a>(input: &'a str, name: &str) -> Option<&'a str> {
    let rest = input.strip_prefix(name)?;
    if rest.is_empty() {
        Some("")
    } else if rest.starts_with(char::is_whitespace) {
        Some(rest.trim())
    } else {
        None
    }
}

/// `/file read <path>` | `/file list <path>` | `/file write <path> <content>`.
async fn file_op(app: &AppHandle, state: &AppState, rest: &str) -> AppResult<String> {
    let (op, args) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let args = args.trim();
    if args.is_empty() {
        return Err(AppError::InvalidInput(
            "usage: /file read|list <path>  or  /file write <path> <content>".into(),
        ));
    }
    match op {
        "read" => files::read_file(app, state, args, Source::User).await,
        "list" => {
            let names = files::list_directory(app, state, args, Source::User).await?;
            Ok(format!("Contents of {args}:\n{}", names.join("\n")))
        }
        "write" => {
            let (path, content) = args.split_once(char::is_whitespace).ok_or_else(|| {
                AppError::InvalidInput("usage: /file write <path> <content>".into())
            })?;
            files::write_file(app, state, path, content, Source::User).await?;
            Ok(format!("Wrote {} bytes to {path}", content.len()))
        }
        other => Err(AppError::InvalidInput(format!(
            "unknown file operation `{other}` (use read, list or write)"
        ))),
    }
}

fn monitor(state: &AppState) -> AppResult<String> {
    let mut m = state.monitor()?;
    let s = m.status()?;
    let info = m.info()?;
    let total = info.total_memory as f64 / 1_073_741_824.0;
    let used = info.used_memory as f64 / 1_073_741_824.0;
    let top = crate::system::processes::list(&mut m);
    let mut out = format!(
        "System status\n• CPU: {:.1}%\n• Memory: {:.1}% ({used:.2} / {total:.2} GiB)\n• Uptime: {} h\n• Processes: {}\n\nTop processes by CPU:\n",
        s.cpu,
        s.memory,
        s.uptime / 3600,
        s.processes
    );
    for (i, p) in top.iter().take(5).enumerate() {
        out.push_str(&format!(
            "  {}. {} (PID {}) {:.1}%\n",
            i + 1,
            p.name,
            p.pid,
            p.cpu
        ));
    }
    Ok(out)
}

/// Render an execution result for the chat transcript.
fn format_exec(r: &ExecResult) -> String {
    let status = match r.exit_code {
        Some(0) => "✓ exit 0".to_string(),
        Some(c) => format!("⚠ exit {c}"),
        None => "⚠ terminated by signal".to_string(),
    };
    let mut s = format!("{status} · {} ms · {}\n", r.duration_ms, r.tier);
    if !r.stdout.is_empty() {
        s.push_str(&fenced(&r.stdout));
    }
    if !r.stderr.is_empty() {
        s.push_str("\nstderr:\n");
        s.push_str(&fenced(&r.stderr));
    }
    s
}

/// Wrap output in a Markdown code fence longer than any backtick run inside it,
/// so command output can never break out of the block when rendered.
fn fenced(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}\n{}\n{fence}\n", text.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_matches_on_word_boundary() {
        assert_eq!(slash("/execute ls -la", "/execute"), Some("ls -la"));
        assert_eq!(slash("/execute", "/execute"), Some(""));
        assert_eq!(slash("/executes ls", "/execute"), None);
        assert_eq!(slash("hello", "/execute"), None);
    }

    #[test]
    fn fences_cannot_be_escaped() {
        assert_eq!(fenced("hi"), "```\nhi\n```\n");
        let f = fenced("a ```` b");
        assert!(f.starts_with("`````\n"));
    }
}
