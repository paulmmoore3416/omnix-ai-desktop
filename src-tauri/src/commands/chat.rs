//! Chat input router.
//!
//! Slash commands are handled locally; everything else goes to the configured
//! LLM. Every privileged action goes through the same policy/confirm/audit
//! pipeline as the dedicated commands. There is no canned "fallback" answer:
//! if the model is unreachable, the user gets an error.

use crate::ai::agent::{self, UiEvent};
use crate::error::{AppError, AppResult};
use crate::memory::{self, NewMemory, SearchHit};
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
    if let Some(rest) = slash(input, "/remember") {
        return remember(&state, rest).await;
    }
    if let Some(rest) = slash(input, "/recall") {
        return recall(&state, rest).await;
    }
    if let Some(rest) = slash(input, "/search") {
        return search(&state, rest).await;
    }
    if slash(input, "/monitor").is_some() {
        return monitor(&state);
    }
    let word = input.split_whitespace().next().unwrap_or(input).to_string();
    Err(AppError::InvalidInput(format!(
        "unknown command {word}. Available: /execute, /file, /monitor, /remember, /recall, /search (plain text goes to chat)"
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

/// Results shown by `/recall` and `/search`.
const LOOKUP_LIMIT: u32 = 8;
/// Hits scoring below this are dropped. The kb-core contract calibrates
/// `score` so that unrelated text scores ≤ 0.25; a deliberate lookup shows
/// anything above that (auto-recall uses the stricter `recall_min_score`).
const LOOKUP_MIN_SCORE: f32 = 0.25;
/// Characters of each hit shown in the chat transcript.
const LOOKUP_PREVIEW_CHARS: usize = 300;

/// `/remember <text> [#tag …]`: save a user memory. Trailing `#tag` words
/// become tags. Memories are inert data (no confirmation), but the write is
/// audited like the assistant's `remember` tool.
async fn remember(state: &AppState, rest: &str) -> AppResult<String> {
    let (content, tags) = split_tags(rest);
    if content.is_empty() {
        return Err(AppError::InvalidInput(
            "usage: /remember <what to remember> [#tag …]".into(),
        ));
    }
    if content.chars().count() > 20_000 {
        return Err(AppError::InvalidInput(
            "a memory must be at most 20000 characters".into(),
        ));
    }
    let v = agent::save_memory_audited(
        state,
        NewMemory {
            content: content.clone(),
            tags,
            // Stated deliberately by the user, so ranked above the
            // assistant's default of 6.
            importance: 7,
            category: "general".into(),
            source: Some("user".into()),
            collection: None,
        },
        Source::User,
    )
    .await?;
    let id = v.get("id").and_then(|i| i.as_str()).unwrap_or("?");
    Ok(match v.get("status").and_then(|s| s.as_str()) {
        Some("reinforced") => {
            format!("🧠 Already known; the existing memory was reinforced (id `{id}`).")
        }
        Some("updated") => format!("🧠 Updated an existing memory with this wording (id `{id}`)."),
        _ => format!("💾 Remembered (id `{id}`). Manage it in the Knowledge view."),
    })
}

/// `/recall [query]`: memories matching the query, or the most recent
/// memories when no query is given. Document chunks are left out (`/search`).
async fn recall(state: &AppState, query: &str) -> AppResult<String> {
    let store = memory::require(state).await?;
    if query.is_empty() {
        let recent = store.list(LOOKUP_LIMIT).await?;
        if recent.is_empty() {
            return Ok("No memories yet. Save one with `/remember <fact>`.".into());
        }
        let mut out = String::from("Most recent memories:\n");
        for m in &recent {
            let when = m.created_at.as_deref().and_then(|t| t.get(..10));
            out.push_str(&format!(
                "- {}{} `{}`\n",
                preview(&m.content),
                when.map(|d| format!(" (saved {d})")).unwrap_or_default(),
                m.id
            ));
        }
        return Ok(out);
    }
    let hits = relevant(store.search_kind(query, LOOKUP_LIMIT, "memory").await?);
    if hits.is_empty() {
        return Ok(format!("No memories match “{query}”."));
    }
    Ok(format!(
        "Memories matching “{query}”:\n{}",
        format_lookup(&hits)
    ))
}

/// `/search <query>`: semantic + keyword search over everything in the
/// memory service: memories and indexed documents (notes, watched folders).
async fn search(state: &AppState, query: &str) -> AppResult<String> {
    if query.is_empty() {
        return Err(AppError::InvalidInput("usage: /search <query>".into()));
    }
    let store = memory::require(state).await?;
    let hits = relevant(store.search(query, LOOKUP_LIMIT).await?);
    if hits.is_empty() {
        return Ok(format!(
            "Nothing in memory or indexed documents matches “{query}”."
        ));
    }
    Ok(format!("Results for “{query}”:\n{}", format_lookup(&hits)))
}

/// Split trailing `#tag` words off `/remember` input.
fn split_tags(input: &str) -> (String, Vec<String>) {
    let mut words: Vec<&str> = input.split_whitespace().collect();
    let mut tags = Vec::new();
    while let Some(w) = words.last() {
        match w.strip_prefix('#') {
            Some(t) if !t.is_empty() && t.len() <= 64 && !t.contains('#') => {
                tags.push(t.to_lowercase());
                words.pop();
            }
            _ => break,
        }
    }
    tags.reverse();
    tags.dedup();
    tags.truncate(10);
    (words.join(" "), tags)
}

fn relevant(mut hits: Vec<SearchHit>) -> Vec<SearchHit> {
    hits.retain(|h| h.score >= LOOKUP_MIN_SCORE);
    hits
}

/// One Markdown list line per hit. Stored text is untrusted data: it is
/// flattened to one line and clipped, and the transcript renders it through
/// the sanitizing Markdown renderer like any other reply.
fn format_lookup(hits: &[SearchHit]) -> String {
    let mut out = String::new();
    for h in hits {
        let label = if h.kind.as_deref() == Some("document") {
            format!("📄 {}", h.source.as_deref().unwrap_or("document"))
        } else {
            "🧠 memory".to_string()
        };
        let when = h
            .created_at
            .as_deref()
            .and_then(|t| t.get(..10))
            .map(|d| format!(", {d}"))
            .unwrap_or_default();
        out.push_str(&format!(
            "- **{label}**{when} · relevance {:.2}: {}\n",
            h.score,
            preview(&h.content)
        ));
    }
    out
}

fn preview(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut p: String = flat.chars().take(LOOKUP_PREVIEW_CHARS).collect();
    if flat.chars().count() > LOOKUP_PREVIEW_CHARS {
        p.push('…');
    }
    p
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
    fn remember_splits_trailing_tags() {
        assert_eq!(
            split_tags("I prefer tea #Drinks #prefs"),
            ("I prefer tea".into(), vec!["drinks".into(), "prefs".into()])
        );
        // Only trailing tags are split off; a # inside the text stays.
        assert_eq!(
            split_tags("issue #42 is fixed"),
            ("issue #42 is fixed".into(), vec![])
        );
        assert_eq!(split_tags("#only"), (String::new(), vec!["only".into()]));
        assert_eq!(split_tags("  "), (String::new(), vec![]));
    }

    #[test]
    fn lookup_preview_is_one_clipped_line() {
        assert_eq!(preview("a\n\n b\tc"), "a b c");
        let long = "x".repeat(LOOKUP_PREVIEW_CHARS + 5);
        assert_eq!(preview(&long).chars().count(), LOOKUP_PREVIEW_CHARS + 1);
    }

    #[test]
    fn fences_cannot_be_escaped() {
        assert_eq!(fenced("hi"), "```\nhi\n```\n");
        let f = fenced("a ```` b");
        assert!(f.starts_with("`````\n"));
    }
}
