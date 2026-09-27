//! Side tasks: tool-less AI jobs that run next to the chat (see `ai::tasks`).

use crate::ai::agent::UiEvent;
use crate::ai::tasks;
use crate::error::AppResult;
use crate::state::AppState;
use tauri::ipc::Channel;
use tauri::State;

/// Run side task `id`. Tokens and notices stream over `on_event`; the
/// returned future resolves when the task ends. Errors are both returned and
/// streamed as `error`.
#[tauri::command]
pub async fn task_run(
    state: State<'_, AppState>,
    id: String,
    instruction: String,
    context: Option<String>,
    on_event: Channel<UiEvent>,
) -> AppResult<()> {
    let emit = |ev: UiEvent| {
        // A closed channel (window reloaded) is not an error for the task.
        if let Err(e) = on_event.send(ev) {
            tracing::debug!(error = %e, "task channel closed");
        }
    };
    let result = tasks::run(&state, &id, &instruction, context.as_deref(), &emit).await;
    if let Err(e) = &result {
        emit(UiEvent::Error {
            message: e.to_string(),
        });
    }
    result
}

/// Stop side task `id` (between stream events). Unknown ids are ignored:
/// the task may have just finished.
#[tauri::command]
pub async fn task_cancel(state: State<'_, AppState>, id: String) -> AppResult<()> {
    tasks::check_id(&id)?;
    state.tasks.cancel(&id);
    Ok(())
}
