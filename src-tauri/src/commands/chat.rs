//! Chat input router.
//!
//! Slash commands are handled locally; everything else goes to the configured
//! LLM. Every privileged action goes through the same policy/confirm/audit
//! pipeline as the dedicated commands. There is no canned "fallback" answer:
//! if the model is unreachable, the user gets an error.

use super::not_implemented;
use crate::ai::{endpoint, ollama};
use crate::error::{AppError, AppResult};
use crate::security::executor::{self, ExecRequest, ExecResult};
use crate::security::files;
use crate::security::policy::Source;
use crate::state::AppState;
use crate::system::metrics;
use tauri::{AppHandle, State};

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
    if input.starts_with('/') {
        let word = input.split_whitespace().next().unwrap_or(input).to_string();
        return Err(AppError::InvalidInput(format!(
            "unknown command {word}. Available: /execute, /file, /monitor"
        )));
    }
    ask_llm(&state, input).await
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
    let mut sys = state.system()?;
    let s = metrics::status(&mut sys)?;
    let total = sys.total_memory() as f64 / 1_073_741_824.0;
    let used = sys.used_memory() as f64 / 1_073_741_824.0;
    let top = crate::system::processes::list(&mut sys);
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

async fn ask_llm(state: &AppState, prompt: &str) -> AppResult<String> {
    let settings = state.settings.read().await.clone();
    let ai = &settings.ai;
    endpoint::ensure_provider_allowed(&ai.provider, settings.security.local_only)?;
    if ai.provider != "ollama" {
        return not_implemented("cloud chat providers");
    }
    if ai.ollama_model.is_empty() {
        return Err(AppError::InvalidInput(
            "no Ollama model selected: choose one in Settings → AI Models".into(),
        ));
    }
    endpoint::ensure_endpoint_allowed(&ai.ollama_host, settings.security.local_only).await?;
    ollama::generate(
        &state.http,
        &ai.ollama_host,
        &ai.ollama_model,
        prompt,
        ai.temperature,
        ai.max_tokens,
    )
    .await
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
        s.push_str(&r.stdout);
    }
    if !r.stderr.is_empty() {
        s.push_str("\n[stderr]\n");
        s.push_str(&r.stderr);
    }
    s
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
}
