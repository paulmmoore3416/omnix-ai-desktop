//! Outbound texts and calls to the owner's phone through Twilio.
//!
//! Outbound only: OMNIX never accepts instructions by SMS or phone (caller
//! ID can be spoofed), so there is no listener and no new way in. The only
//! recipient is `phone.to_number`; nothing (the model, a rule, the webview)
//! can choose another one, and changing it is a natively confirmed settings
//! change.
//!
//! Security decisions:
//! * The API host is a constant, never a setting, so a settings change
//!   cannot redirect message text (and the auth token) elsewhere.
//! * The auth token is read from the keychain (`twilio`) per send and is
//!   never returned over IPC or logged.
//! * Message text leaves the machine. That is the one exception to
//!   `local_only`, taken only when the user has enabled the phone (see
//!   [`crate::ai::endpoint::ensure_phone_allowed`]).
//! * Sends are capped per rolling hour (`phone.max_per_hour`) and every
//!   attempt, sent or refused, is audited.

use crate::error::{AppError, AppResult};
use crate::security::audit::{AuditRecord, Confirmation, Decision};
use crate::security::policy::{RiskTier, Source};
use crate::settings::{is_e164, is_twilio_sid, PhoneSettings};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Twilio REST API base (fixed on purpose, see the module docs).
const TWILIO_API: &str = "https://api.twilio.com/2010-04-01";
/// Longest SMS body sent (Twilio accepts 1600 characters).
const MAX_SMS_CHARS: usize = 1_500;
/// Longest text read out on a call.
const MAX_CALL_CHARS: usize = 800;

/// How to reach the phone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhoneChannel {
    /// A text message.
    Sms,
    /// A voice call that reads the message aloud.
    Call,
}

impl PhoneChannel {
    fn action(self) -> &'static str {
        match self {
            PhoneChannel::Sms => "phone_sms",
            PhoneChannel::Call => "phone_call",
        }
    }
}

/// Timestamps of recent sends, for `phone.max_per_hour`.
#[derive(Default)]
pub struct RateLimiter(std::sync::Mutex<VecDeque<Instant>>);

impl RateLimiter {
    /// Record a send if fewer than `max` happened in the last hour.
    fn try_take(&self, max: u32, now: Instant) -> bool {
        let mut q = self.0.lock().unwrap_or_else(|e| e.into_inner());
        while q
            .front()
            .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(3600))
        {
            q.pop_front();
        }
        if q.len() >= max as usize {
            return false;
        }
        q.push_back(now);
        true
    }
}

/// Send `text` to the owner's phone. `reason` says what triggered it (for
/// the audit log). Returns Twilio's message or call SID.
pub async fn send(
    state: &AppState,
    channel: PhoneChannel,
    text: &str,
    reason: &str,
    source: Source,
) -> AppResult<String> {
    let cfg = state.settings.read().await.phone.clone();
    let started = Instant::now();
    let result = deliver(state, &cfg, channel, text).await;
    let audit = state
        .audit
        .record(AuditRecord {
            id: uuid::Uuid::new_v4().to_string(),
            source,
            action: channel.action().into(),
            command: format!("{reason}: {}", text.chars().take(160).collect::<String>()),
            cwd: None,
            // Leaves the machine and costs money, but only ever reaches the
            // owner's own number.
            tier: RiskTier::Mutating,
            decision: if result.is_ok() {
                Decision::Allowed
            } else {
                Decision::Failed
            },
            confirmation: Confirmation::NotRequired,
            exit_code: None,
            duration_ms: Some(started.elapsed().as_millis() as u64),
            detail: result.as_ref().err().map(ToString::to_string),
        })
        .await;
    if let Err(e) = audit {
        tracing::warn!(error = %e, "could not audit phone message");
    }
    result
}

async fn deliver(
    state: &AppState,
    cfg: &PhoneSettings,
    channel: PhoneChannel,
    text: &str,
) -> AppResult<String> {
    crate::ai::endpoint::ensure_phone_allowed(cfg.enabled)?;
    // Settings are validated on save; re-checked because the SID goes into
    // the URL path.
    if !is_twilio_sid(&cfg.account_sid) || !is_e164(&cfg.from_number) || !is_e164(&cfg.to_number) {
        return Err(AppError::InvalidInput(
            "phone is not fully configured (Settings → Phone)".into(),
        ));
    }
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError::InvalidInput("message is empty".into()));
    }
    if !state.phone_limit.try_take(cfg.max_per_hour, Instant::now()) {
        return Err(AppError::InvalidInput(format!(
            "phone limit reached ({} messages/calls in the last hour; Settings → Phone)",
            cfg.max_per_hour
        )));
    }
    let secrets = state.secrets.clone();
    let token = tokio::task::spawn_blocking(move || secrets.get("twilio"))
        .await??
        .ok_or_else(|| {
            AppError::Secret("no Twilio auth token: add it in Settings → Phone".into())
        })?;
    let (endpoint, body_key, body) = match channel {
        PhoneChannel::Sms => ("Messages", "Body", sms_body(text)),
        PhoneChannel::Call => ("Calls", "Twiml", twiml(text)),
    };
    let url = format!("{TWILIO_API}/Accounts/{}/{endpoint}.json", cfg.account_sid);
    let form = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("To", &cfg.to_number)
        .append_pair("From", &cfg.from_number)
        .append_pair(body_key, &body)
        .finish();
    let resp = state
        .http
        .post(url)
        .basic_auth(&cfg.account_sid, Some(token))
        .timeout(Duration::from_secs(20))
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form)
        .send()
        .await
        .map_err(|e| AppError::Unavailable(format!("Twilio: {e}")))?;
    let status = resp.status();
    let v: serde_json::Value = resp.json().await.unwrap_or_default();
    if !status.is_success() {
        let msg = v
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("request failed");
        return Err(match status.as_u16() {
            401 | 403 => AppError::Secret(format!("Twilio rejected the credentials: {msg}")),
            _ => AppError::Unavailable(format!("Twilio returned {status}: {msg}")),
        });
    }
    Ok(v.get("sid")
        .and_then(|s| s.as_str())
        .unwrap_or("?")
        .to_string())
}

/// Plain one-block SMS text, prefixed so the sender is obvious.
fn sms_body(text: &str) -> String {
    let body = format!("OMNIX: {}", plain(text));
    clip(&body, MAX_SMS_CHARS)
}

/// Inline TwiML that reads the message twice. The text is XML-escaped, so
/// it cannot add TwiML verbs (e.g. `<Dial>` to another number).
fn twiml(text: &str) -> String {
    let said = xml_escape(&clip(&plain(text), MAX_CALL_CHARS));
    format!(
        "<Response><Say>This is OMNIX.</Say><Pause length=\"1\"/><Say loop=\"2\">{said}</Say></Response>"
    )
}

/// Drop Markdown markers and collapse whitespace (texts and speech).
fn plain(text: &str) -> String {
    let stripped: String = text
        .chars()
        .filter(|c| !matches!(c, '*' | '`' | '#' | '_'))
        .collect();
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_text_cannot_inject_twiml() {
        let t = twiml("hi</Say><Dial>+15550000000</Dial><Say>");
        assert!(!t.contains("<Dial>"));
        assert!(t.contains("&lt;Dial&gt;"));
    }

    #[test]
    fn sms_is_plain_and_clipped() {
        assert_eq!(
            sms_body("**Disk** at `95%`\n\nnow"),
            "OMNIX: Disk at 95% now"
        );
        let long = "x".repeat(5_000);
        assert_eq!(sms_body(&long).chars().count(), MAX_SMS_CHARS);
    }

    #[test]
    fn rate_limit_rolls_over_after_an_hour() {
        let r = RateLimiter::default();
        let t0 = Instant::now();
        assert!(r.try_take(2, t0));
        assert!(r.try_take(2, t0));
        assert!(!r.try_take(2, t0 + Duration::from_secs(60)));
        assert!(r.try_take(2, t0 + Duration::from_secs(3601)));
    }
}
