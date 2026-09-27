//! Phone (Twilio) test command. Sending itself lives in `crate::phone`.

use crate::error::AppResult;
use crate::phone::{self, PhoneChannel};
use crate::security::policy::Source;
use crate::state::AppState;
use tauri::State;

/// Send a fixed test text or call to the configured number. The webview
/// chooses only the channel, never the recipient or the text.
#[tauri::command]
pub async fn phone_test(state: State<'_, AppState>, channel: PhoneChannel) -> AppResult<String> {
    let text = match channel {
        PhoneChannel::Sms => "Test message. Texts from OMNIX are working.",
        PhoneChannel::Call => "This is a test call. Calls from OMNIX are working. Goodbye.",
    };
    let sid = phone::send(&state, channel, text, "test from Settings", Source::User).await?;
    Ok(format!("Sent (Twilio id {sid})"))
}
