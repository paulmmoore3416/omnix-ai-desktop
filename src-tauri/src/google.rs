//! Google services: Gmail, Drive and the Developer Knowledge API.
//!
//! Off by default (`google.enabled`, a natively confirmed settings change),
//! and **not** an exception to `local_only`: every request goes through
//! [`crate::ai::endpoint::ensure_endpoint_allowed`], so nothing reaches Google
//! while local-only mode is on. Mail, file contents and search queries leave
//! the machine when these tools are used; see `docs/SECURITY.md`.
//!
//! Security decisions:
//! * **OAuth for user data.** Gmail and Drive need the user's consent, not an
//!   API key. [`connect`] runs the RFC 8252 installed-app flow: PKCE (S256),
//!   a random `state`, and a one-shot redirect listener bound to
//!   `127.0.0.1` on an ephemeral port that accepts a single matching request
//!   and closes after it or after [`CONNECT_TIMEOUT`]. It serves nothing
//!   else and is never reachable from the network.
//! * **Least privilege.** Gmail: `gmail.readonly` + `gmail.compose` (drafts;
//!   OMNIX never sends mail). Drive: `drive.readonly` + `drive.file` (writes
//!   only to files OMNIX uploads). Only the services the user ticked are
//!   requested.
//! * **Secrets.** The OAuth client secret (`google_oauth_client`) and the
//!   Developer Knowledge key (`google_devknowledge`) are write-only keychain
//!   entries. The refresh token is `internal.google_refresh`, which the
//!   webview can neither read nor set (a planted token would connect OMNIX
//!   to someone else's account). Access tokens stay in memory.
//! * **Fixed hosts.** API bases are constants, never settings.
//! * **Untrusted content.** Mail and documents are returned to the model as
//!   tool results, which the agent loop wraps as untrusted data. Anything
//!   that writes (a draft, an upload) is natively confirmed and audited;
//!   reads are audited and follow `security.require_confirmation`.

use crate::ai::endpoint::ensure_endpoint_allowed;
use crate::error::{AppError, AppResult};
use crate::security::audit::{AuditRecord, Confirmation, Decision};
use crate::security::confirm::{self, ConfirmRequest};
use crate::security::policy::{RiskTier, Source};
use crate::settings::GoogleSettings;
use crate::state::AppState;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
const GMAIL_API: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
const DRIVE_API: &str = "https://www.googleapis.com/drive/v3";
const DRIVE_UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/files";
const DEV_KNOWLEDGE_API: &str = "https://developerknowledge.googleapis.com/v1";

/// Keychain id of the refresh token (Rust-only, see the module docs).
pub const REFRESH_TOKEN_ID: &str = "internal.google_refresh";
/// Keychain id of the OAuth client secret.
pub const CLIENT_SECRET_ID: &str = "google_oauth_client";
/// Keychain id of the Developer Knowledge API key.
pub const DEV_KEY_ID: &str = "google_devknowledge";

/// How long [`connect`] waits for the browser consent.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(300);
/// Largest mail body or document returned to the model (characters).
const MAX_TEXT_CHARS: usize = 20_000;
/// Largest Drive download.
const MAX_DOWNLOAD_BYTES: usize = 5 * 1024 * 1024;
/// Largest upload.
const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;

/// Which Google service a call needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    /// Gmail (OAuth).
    Gmail,
    /// Drive (OAuth).
    Drive,
    /// Developer Knowledge (API key).
    DevDocs,
}

impl Service {
    fn label(self) -> &'static str {
        match self {
            Service::Gmail => "Gmail",
            Service::Drive => "Google Drive",
            Service::DevDocs => "Google developer docs",
        }
    }
    fn on(self, g: &GoogleSettings) -> bool {
        g.enabled
            && match self {
                Service::Gmail => g.gmail,
                Service::Drive => g.drive,
                Service::DevDocs => g.dev_docs,
            }
    }
}

/// OAuth scopes for the ticked services.
pub fn scopes(g: &GoogleSettings) -> Vec<&'static str> {
    let mut s = Vec::new();
    if g.gmail {
        s.extend([
            "https://www.googleapis.com/auth/gmail.readonly",
            "https://www.googleapis.com/auth/gmail.compose",
        ]);
    }
    if g.drive {
        s.extend([
            "https://www.googleapis.com/auth/drive.readonly",
            "https://www.googleapis.com/auth/drive.file",
        ]);
    }
    s
}

/// In-memory access token.
#[derive(Default)]
pub struct GoogleSession {
    token: tokio::sync::Mutex<Option<(String, Instant)>>,
    connecting: tokio::sync::Mutex<()>,
}

/// What Settings shows.
#[derive(Debug, Clone, Serialize)]
pub struct GoogleStatus {
    /// A refresh token is stored.
    pub connected: bool,
    /// An OAuth client secret is stored.
    pub has_client_secret: bool,
    /// A Developer Knowledge key is stored.
    pub has_dev_key: bool,
    /// Blocked by local-only mode.
    pub local_only: bool,
}

/// Current connection state (never returns a secret).
pub async fn status(state: &AppState) -> AppResult<GoogleStatus> {
    let store = state.secrets.clone();
    let (connected, has_client_secret, has_dev_key) =
        tokio::task::spawn_blocking(move || -> AppResult<(bool, bool, bool)> {
            Ok((
                store.has(REFRESH_TOKEN_ID)?,
                store.has(CLIENT_SECRET_ID)?,
                store.has(DEV_KEY_ID)?,
            ))
        })
        .await??;
    Ok(GoogleStatus {
        connected,
        has_client_secret,
        has_dev_key,
        local_only: state.settings.read().await.security.local_only,
    })
}

/// Refuse unless the service is enabled and local-only mode is off.
pub async fn ensure_allowed(state: &AppState, service: Service) -> AppResult<()> {
    let s = state.settings.read().await;
    if !service.on(&s.google) {
        return Err(AppError::InvalidInput(format!(
            "{} is off: enable it in Settings → Google",
            service.label()
        )));
    }
    if s.security.local_only {
        return Err(AppError::LocalOnly(format!(
            "{} is a cloud service; turn off local-only mode in Settings → Security to use it",
            service.label()
        )));
    }
    Ok(())
}

/// Whether the agent should be offered this service's tools.
pub async fn tools_available(state: &AppState, service: Service) -> bool {
    if ensure_allowed(state, service).await.is_err() {
        return false;
    }
    let id = match service {
        Service::DevDocs => DEV_KEY_ID,
        Service::Gmail | Service::Drive => REFRESH_TOKEN_ID,
    };
    let store = state.secrets.clone();
    tokio::task::spawn_blocking(move || store.has(id))
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(false)
}

async fn secret(state: &AppState, id: &'static str) -> AppResult<Option<String>> {
    let store = state.secrets.clone();
    tokio::task::spawn_blocking(move || store.get(id)).await?
}

// ---------------------------------------------------------------- OAuth ---

/// PKCE verifier (64 hex characters from two random UUIDs, within the
/// RFC 7636 alphabet) and its S256 challenge.
fn pkce() -> (String, String) {
    let verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

/// Run the consent flow in the user's browser and store the refresh token.
/// Called from Settings → Google → Connect (a user click).
pub async fn connect<R: Runtime>(app: &AppHandle<R>, state: &AppState) -> AppResult<String> {
    let _one =
        state.google.connecting.try_lock().map_err(|_| {
            AppError::InvalidInput("a Google sign-in is already in progress".into())
        })?;
    let settings = state.settings.read().await.clone();
    let g = &settings.google;
    if !g.enabled {
        return Err(AppError::InvalidInput(
            "turn on Google in Settings → Google and save first".into(),
        ));
    }
    if settings.security.local_only {
        return Err(AppError::LocalOnly(
            "Google is a cloud service; turn off local-only mode in Settings → Security first"
                .into(),
        ));
    }
    let scopes = scopes(g);
    if scopes.is_empty() {
        return Err(AppError::InvalidInput(
            "tick Gmail and/or Drive before connecting".into(),
        ));
    }
    if g.client_id.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "enter the OAuth client ID (Google Cloud Console → Credentials → Desktop app)".into(),
        ));
    }
    let client_secret = secret(state, CLIENT_SECRET_ID).await?.ok_or_else(|| {
        AppError::InvalidInput("save the OAuth client secret in Settings → Google first".into())
    })?;
    ensure_endpoint_allowed(AUTH_URL, false).await?;

    // One-shot loopback redirect listener (RFC 8252 §7.3).
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
    let redirect = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
    let (verifier, challenge) = pkce();
    let csrf = uuid::Uuid::new_v4().simple().to_string();
    let mut auth =
        reqwest::Url::parse(AUTH_URL).map_err(|e| AppError::Internal(format!("auth URL: {e}")))?;
    auth.query_pairs_mut()
        .append_pair("client_id", g.client_id.trim())
        .append_pair("redirect_uri", &redirect)
        .append_pair("response_type", "code")
        .append_pair("scope", &scopes.join(" "))
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &csrf)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");
    tauri_plugin_opener::open_url(auth.as_str(), None::<&str>)
        .map_err(|e| AppError::Unavailable(format!("cannot open the browser: {e}")))?;
    let _ = app; // the browser, not a webview window, shows Google's consent page

    let code = tokio::time::timeout(CONNECT_TIMEOUT, wait_for_code(&listener, &csrf))
        .await
        .map_err(|_| AppError::Unavailable("Google sign-in timed out after 5 minutes".into()))??;
    drop(listener);

    let form = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", g.client_id.trim())
        .append_pair("client_secret", &client_secret)
        .append_pair("code", &code)
        .append_pair("code_verifier", &verifier)
        .append_pair("grant_type", "authorization_code")
        .append_pair("redirect_uri", &redirect)
        .finish();
    let tok = token_request(state, form).await?;
    let refresh = tok
        .get("refresh_token")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Unavailable("Google returned no refresh token".into()))?
        .to_string();
    remember_access(state, &tok).await;
    let store = state.secrets.clone();
    tokio::task::spawn_blocking(move || store.set(REFRESH_TOKEN_ID, &refresh)).await??;
    let who = account_email(state).await.unwrap_or_default();
    audit(
        state,
        Source::User,
        "google_connect",
        &format!("scopes={}", scopes.join(" ")),
        RiskTier::Mutating,
        Decision::Allowed,
        Confirmation::Approved,
        Some(format!("account {who}")),
    )
    .await?;
    Ok(if who.is_empty() {
        "Connected to Google.".into()
    } else {
        format!("Connected to Google as {who}.")
    })
}

/// Accept connections until one carries our `state`; answer each with a
/// tiny static page. Requests with another `state` (or none) are refused
/// and the wait continues.
async fn wait_for_code(listener: &tokio::net::TcpListener, csrf: &str) -> AppResult<String> {
    loop {
        let (mut sock, _) = listener.accept().await?;
        let mut buf = vec![0u8; 8192];
        let n = tokio::time::timeout(Duration::from_secs(10), sock.read(&mut buf))
            .await
            .unwrap_or(Ok(0))
            .unwrap_or(0);
        let head = String::from_utf8_lossy(&buf[..n]);
        let outcome = parse_redirect(&head, csrf);
        let (status, body) = match &outcome {
            Some(Ok(_)) => (
                "200 OK",
                "Connected. You can close this tab and return to OMNIX.",
            ),
            Some(Err(_)) => (
                "400 Bad Request",
                "Google sign-in failed. Return to OMNIX for details.",
            ),
            None => ("400 Bad Request", "Unexpected request."),
        };
        let page = format!(
            "<!doctype html><meta charset=utf-8><title>OMNIX</title><p style=\"font:16px system-ui;margin:3em\">{body}</p>"
        );
        let resp = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{page}",
            page.len()
        );
        let _ = sock.write_all(resp.as_bytes()).await;
        let _ = sock.shutdown().await;
        if let Some(result) = outcome {
            return result;
        }
    }
}

/// `Some(Ok(code))` for our redirect, `Some(Err)` for our redirect carrying
/// an error, `None` for anything else (wrong state, favicon, garbage).
fn parse_redirect(head: &str, csrf: &str) -> Option<AppResult<String>> {
    let target = head
        .lines()
        .next()?
        .strip_prefix("GET ")?
        .split(' ')
        .next()?;
    let url = reqwest::Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    let q: std::collections::HashMap<String, String> = url.query_pairs().into_owned().collect();
    if q.get("state").map(String::as_str) != Some(csrf) {
        return None;
    }
    if let Some(e) = q.get("error") {
        return Some(Err(AppError::NotApproved(format!(
            "Google sign-in was not completed ({e})"
        ))));
    }
    q.get("code")
        .filter(|c| !c.is_empty())
        .map(|c| Ok(c.clone()))
}

async fn token_request(state: &AppState, form: String) -> AppResult<Value> {
    ensure_endpoint_allowed(TOKEN_URL, false).await?;
    let resp = state
        .http
        .post(TOKEN_URL)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form)
        .send()
        .await?;
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let err = v.get("error").and_then(Value::as_str).unwrap_or("error");
        return Err(if err == "invalid_grant" {
            AppError::NotApproved(
                "Google access was revoked or expired: reconnect in Settings → Google".into(),
            )
        } else {
            AppError::Unavailable(format!("Google token request failed: {status} {err}"))
        });
    }
    Ok(v)
}

async fn remember_access(state: &AppState, tok: &Value) {
    if let Some(at) = tok.get("access_token").and_then(Value::as_str) {
        let secs = tok
            .get("expires_in")
            .and_then(Value::as_u64)
            .unwrap_or(3600);
        // Refresh a minute early.
        let until = Instant::now() + Duration::from_secs(secs.saturating_sub(60));
        *state.google.token.lock().await = Some((at.to_string(), until));
    }
}

/// A valid access token, refreshed when needed.
async fn access_token(state: &AppState) -> AppResult<String> {
    if let Some((t, until)) = state.google.token.lock().await.clone() {
        if Instant::now() < until {
            return Ok(t);
        }
    }
    let settings = state.settings.read().await.clone();
    let refresh = secret(state, REFRESH_TOKEN_ID).await?.ok_or_else(|| {
        AppError::InvalidInput("Google is not connected: Settings → Google → Connect".into())
    })?;
    let client_secret = secret(state, CLIENT_SECRET_ID).await?.ok_or_else(|| {
        AppError::InvalidInput("the OAuth client secret is missing: Settings → Google".into())
    })?;
    let form = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", settings.google.client_id.trim())
        .append_pair("client_secret", &client_secret)
        .append_pair("refresh_token", &refresh)
        .append_pair("grant_type", "refresh_token")
        .finish();
    let tok = token_request(state, form).await?;
    remember_access(state, &tok).await;
    tok.get("access_token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| AppError::Unavailable("Google returned no access token".into()))
}

/// Forget the connection: revoke at Google (best effort), delete the
/// refresh token, drop the access token.
pub async fn disconnect(state: &AppState) -> AppResult<()> {
    let refresh = secret(state, REFRESH_TOKEN_ID).await?;
    if let Some(t) = refresh {
        if !state.settings.read().await.security.local_only {
            let form = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("token", &t)
                .finish();
            let _ = state
                .http
                .post(REVOKE_URL)
                .header(
                    reqwest::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(form)
                .send()
                .await;
        }
        let store = state.secrets.clone();
        tokio::task::spawn_blocking(move || store.delete(REFRESH_TOKEN_ID)).await??;
    }
    *state.google.token.lock().await = None;
    audit(
        state,
        Source::User,
        "google_disconnect",
        "",
        RiskTier::Mutating,
        Decision::Allowed,
        Confirmation::NotRequired,
        None,
    )
    .await
}

async fn account_email(state: &AppState) -> Option<String> {
    let g = state.settings.read().await.google.clone();
    let v = if g.gmail {
        api_get(state, &format!("{GMAIL_API}/profile"), &[])
            .await
            .ok()?
    } else {
        api_get(
            state,
            &format!("{DRIVE_API}/about"),
            &[("fields", "user(emailAddress)")],
        )
        .await
        .ok()?
    };
    v.pointer("/emailAddress")
        .or_else(|| v.pointer("/user/emailAddress"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

// ------------------------------------------------------------ requests ---

async fn api_get(state: &AppState, url: &str, query: &[(&str, &str)]) -> AppResult<Value> {
    let resp = authed(state, reqwest::Method::GET, url)
        .await?
        .query(query)
        .send()
        .await?;
    json_or_error(resp).await
}

async fn authed(
    state: &AppState,
    method: reqwest::Method,
    url: &str,
) -> AppResult<reqwest::RequestBuilder> {
    ensure_endpoint_allowed(url, state.settings.read().await.security.local_only).await?;
    let token = access_token(state).await?;
    Ok(state.http.request(method, url).bearer_auth(token))
}

async fn json_or_error(resp: reqwest::Response) -> AppResult<Value> {
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(v);
    }
    let msg = v
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("request failed");
    Err(AppError::Unavailable(format!(
        "Google API: {status}: {msg}"
    )))
}

/// Audit a Google call (the caller decides the tier and decision).
#[allow(clippy::too_many_arguments)] // mirrors the AuditRecord fields it fills
async fn audit(
    state: &AppState,
    source: Source,
    action: &str,
    command: &str,
    tier: RiskTier,
    decision: Decision,
    confirmation: Confirmation,
    detail: Option<String>,
) -> AppResult<()> {
    state
        .audit
        .record(AuditRecord {
            id: uuid::Uuid::new_v4().to_string(),
            source,
            action: action.into(),
            command: command.chars().take(300).collect(),
            cwd: None,
            tier,
            decision,
            confirmation,
            exit_code: None,
            duration_ms: None,
            detail,
        })
        .await
}

/// Audit an agent read, after the optional read confirmation.
async fn read_gate<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    service: Service,
    action: &str,
    what: &str,
) -> AppResult<()> {
    ensure_allowed(state, service).await?;
    let settings = state.settings.read().await.clone();
    let confirmation = if settings.security.require_confirmation {
        let c = confirm::ask(
            app,
            &ConfirmRequest {
                title: format!("Read from {}?", service.label()),
                subject: what.to_string(),
                details: vec!["The result is sent to the AI model.".into()],
                tier: RiskTier::ReadOnly,
                source: Source::LlmTool,
                reasons: vec!["read confirmation is enabled in Settings → Security".into()],
                approve_label: "Allow".into(),
            },
            Duration::from_secs(settings.security.confirmation_timeout_secs),
        )
        .await;
        if c != Confirmation::Approved {
            audit(
                state,
                Source::LlmTool,
                action,
                what,
                RiskTier::ReadOnly,
                Decision::NotApproved,
                c,
                None,
            )
            .await?;
            return Err(AppError::NotApproved("read was not approved".into()));
        }
        c
    } else {
        Confirmation::NotRequired
    };
    audit(
        state,
        Source::LlmTool,
        action,
        what,
        RiskTier::ReadOnly,
        Decision::Allowed,
        confirmation,
        None,
    )
    .await
}

/// Natively confirm a write to Google; audits a refusal.
async fn write_gate<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    service: Service,
    action: &str,
    req: ConfirmRequest,
    what: &str,
) -> AppResult<Confirmation> {
    ensure_allowed(state, service).await?;
    let timeout = state
        .settings
        .read()
        .await
        .security
        .confirmation_timeout_secs;
    let c = confirm::ask(app, &req, Duration::from_secs(timeout)).await;
    if c != Confirmation::Approved {
        audit(
            state,
            Source::LlmTool,
            action,
            what,
            RiskTier::Mutating,
            Decision::NotApproved,
            c,
            None,
        )
        .await?;
        return Err(AppError::NotApproved(format!(
            "{} was not approved",
            req.title
        )));
    }
    Ok(c)
}

fn clip(s: &str) -> String {
    if s.chars().count() <= MAX_TEXT_CHARS {
        return s.to_string();
    }
    let mut out: String = s.chars().take(MAX_TEXT_CHARS).collect();
    out.push_str("\n…[truncated]");
    out
}

// --------------------------------------------------------------- Gmail ---

/// Search mail with Gmail query syntax; returns sender, subject, date and
/// snippet per message.
pub async fn gmail_search<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    query: &str,
    max: u32,
) -> AppResult<String> {
    read_gate(app, state, Service::Gmail, "gmail_search", query).await?;
    let max = max.clamp(1, 25).to_string();
    let list = api_get(
        state,
        &format!("{GMAIL_API}/messages"),
        &[("q", query), ("maxResults", &max)],
    )
    .await?;
    let ids: Vec<String> = list
        .get("messages")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|m| m.get("id").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if ids.is_empty() {
        return Ok("No messages match.".into());
    }
    let mut out = Vec::new();
    for id in ids {
        let url = format!("{GMAIL_API}/messages/{id}");
        let resp = authed(state, reqwest::Method::GET, &url)
            .await?
            .query(&[
                ("format", "metadata"),
                ("metadataHeaders", "From"),
                ("metadataHeaders", "Subject"),
                ("metadataHeaders", "Date"),
            ])
            .send()
            .await?;
        let m = json_or_error(resp).await?;
        let h = |name: &str| header(&m, name).unwrap_or_default();
        out.push(json!({
            "id": id,
            "from": h("From"),
            "subject": h("Subject"),
            "date": h("Date"),
            "snippet": m.get("snippet").and_then(Value::as_str).unwrap_or_default(),
        }));
    }
    Ok(serde_json::to_string_pretty(&out).unwrap_or_default())
}

fn header(msg: &Value, name: &str) -> Option<String> {
    msg.pointer("/payload/headers")?
        .as_array()?
        .iter()
        .find(|h| {
            h.get("name")
                .and_then(Value::as_str)
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })?
        .get("value")?
        .as_str()
        .map(str::to_string)
}

/// Read one message as plain text.
pub async fn gmail_read<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    id: &str,
) -> AppResult<String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(AppError::InvalidInput("invalid message id".into()));
    }
    read_gate(app, state, Service::Gmail, "gmail_read", id).await?;
    let m = api_get(
        state,
        &format!("{GMAIL_API}/messages/{id}"),
        &[("format", "full")],
    )
    .await?;
    let body = m
        .get("payload")
        .map(message_text)
        .filter(|b| !b.trim().is_empty())
        .unwrap_or_else(|| {
            m.get("snippet")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        });
    let h = |name: &str| header(&m, name).unwrap_or_default();
    Ok(clip(&format!(
        "From: {}\nTo: {}\nDate: {}\nSubject: {}\n\n{}",
        h("From"),
        h("To"),
        h("Date"),
        h("Subject"),
        body.trim()
    )))
}

/// The best text of a MIME tree: the first `text/plain` part, else the
/// first `text/html` part with tags stripped.
fn message_text(payload: &Value) -> String {
    fn find(p: &Value, mime: &str) -> Option<String> {
        if p.get("mimeType").and_then(Value::as_str) == Some(mime) {
            if let Some(d) = p.pointer("/body/data").and_then(Value::as_str) {
                return decode_b64url(d);
            }
        }
        p.get("parts")?
            .as_array()?
            .iter()
            .find_map(|c| find(c, mime))
    }
    find(payload, "text/plain")
        .or_else(|| find(payload, "text/html").map(|h| strip_html(&h)))
        .unwrap_or_default()
}

fn decode_b64url(s: &str) -> Option<String> {
    let bytes = URL_SAFE_NO_PAD.decode(s.trim_end_matches('=')).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Rough HTML → text for mail bodies (the model reads it; nothing renders it).
fn strip_html(html: &str) -> String {
    let no_blocks = regex::Regex::new(r"(?is)<(script|style|head)\b.*?</(script|style|head)>")
        .map(|re| re.replace_all(html, " ").into_owned())
        .unwrap_or_else(|_| html.to_string());
    let breaks = regex::Regex::new(r"(?i)<br\s*/?>|</p>|</div>|</tr>|</li>")
        .map(|re| re.replace_all(&no_blocks, "\n").into_owned())
        .unwrap_or(no_blocks);
    let text = regex::Regex::new(r"<[^>]*>")
        .map(|re| re.replace_all(&breaks, "").into_owned())
        .unwrap_or(breaks);
    let text = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    regex::Regex::new(r"\n\s*\n\s*\n+")
        .map(|re| re.replace_all(&text, "\n\n").into_owned())
        .unwrap_or(text)
}

/// Create a draft (never sent by OMNIX). Natively confirmed.
pub async fn gmail_create_draft<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    to: &str,
    subject: &str,
    body: &str,
) -> AppResult<String> {
    // Header injection: no line breaks in header fields.
    if [to, subject].iter().any(|f| f.contains(['\r', '\n'])) {
        return Err(AppError::InvalidInput(
            "`to` and `subject` must be single lines".into(),
        ));
    }
    if to.trim().is_empty() || !to.contains('@') {
        return Err(AppError::InvalidInput(
            "`to` must be an email address".into(),
        ));
    }
    let preview: String = body.chars().take(400).collect();
    let what = format!("to={to} subject={subject}");
    let confirmation = write_gate(
        app,
        state,
        Service::Gmail,
        "gmail_draft",
        ConfirmRequest {
            title: "Create Gmail draft?".into(),
            subject: format!("To: {to}\nSubject: {subject}"),
            details: vec![format!(
                "Body:\n{preview}{}",
                if body.chars().count() > 400 {
                    "…"
                } else {
                    ""
                }
            )],
            tier: RiskTier::Mutating,
            source: Source::LlmTool,
            reasons: vec!["saves a draft in your Gmail (OMNIX never sends mail)".into()],
            approve_label: "Create draft".into(),
        },
        &what,
    )
    .await?;
    let raw = format!(
        "To: {to}\r\nSubject: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{}",
        encode_subject(subject),
        body.replace("\r\n", "\n").replace('\n', "\r\n")
    );
    let resp = authed(state, reqwest::Method::POST, &format!("{GMAIL_API}/drafts"))
        .await?
        .json(&json!({ "message": { "raw": URL_SAFE_NO_PAD.encode(raw.as_bytes()) } }))
        .send()
        .await?;
    let result = json_or_error(resp).await;
    audit(
        state,
        Source::LlmTool,
        "gmail_draft",
        &what,
        RiskTier::Mutating,
        if result.is_ok() {
            Decision::Allowed
        } else {
            Decision::Failed
        },
        confirmation,
        result.as_ref().err().map(ToString::to_string),
    )
    .await?;
    let v = result?;
    Ok(format!(
        "Draft saved in Gmail (id {}). Review and send it from Gmail.",
        v.get("id").and_then(Value::as_str).unwrap_or("?")
    ))
}

/// RFC 2047 encoded-word for non-ASCII subjects.
fn encode_subject(s: &str) -> String {
    if s.is_ascii() {
        s.to_string()
    } else {
        format!(
            "=?UTF-8?B?{}?=",
            base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
        )
    }
}

// --------------------------------------------------------------- Drive ---

/// Full-text search over Drive (not trashed).
pub async fn drive_search<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    query: &str,
    max: u32,
) -> AppResult<String> {
    read_gate(app, state, Service::Drive, "drive_search", query).await?;
    let escaped = query.replace('\\', "\\\\").replace('\'', "\\'");
    let q =
        format!("(name contains '{escaped}' or fullText contains '{escaped}') and trashed = false");
    let max = max.clamp(1, 50).to_string();
    let v = api_get(
        state,
        &format!("{DRIVE_API}/files"),
        &[
            ("q", q.as_str()),
            ("pageSize", &max),
            (
                "fields",
                "files(id,name,mimeType,modifiedTime,size,webViewLink)",
            ),
            ("orderBy", "modifiedTime desc"),
        ],
    )
    .await?;
    let files = v.get("files").cloned().unwrap_or(json!([]));
    if files.as_array().is_none_or(Vec::is_empty) {
        return Ok("No files match.".into());
    }
    Ok(serde_json::to_string_pretty(&files).unwrap_or_default())
}

fn valid_drive_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 200
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Read a Drive file as text: Google Docs/Slides export as text, Sheets as
/// CSV (first sheet), text-like files download directly.
pub async fn drive_read<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    id: &str,
) -> AppResult<String> {
    if !valid_drive_id(id) {
        return Err(AppError::InvalidInput("invalid Drive file id".into()));
    }
    read_gate(app, state, Service::Drive, "drive_read", id).await?;
    let meta = api_get(
        state,
        &format!("{DRIVE_API}/files/{id}"),
        &[("fields", "id,name,mimeType,size")],
    )
    .await?;
    let mime = meta.get("mimeType").and_then(Value::as_str).unwrap_or("");
    let name = meta.get("name").and_then(Value::as_str).unwrap_or("");
    let (url, query): (String, Vec<(&str, &str)>) = match mime {
        "application/vnd.google-apps.document" | "application/vnd.google-apps.presentation" => (
            format!("{DRIVE_API}/files/{id}/export"),
            vec![("mimeType", "text/plain")],
        ),
        "application/vnd.google-apps.spreadsheet" => (
            format!("{DRIVE_API}/files/{id}/export"),
            vec![("mimeType", "text/csv")],
        ),
        m if is_text_mime(m) => (format!("{DRIVE_API}/files/{id}"), vec![("alt", "media")]),
        other => {
            return Err(AppError::InvalidInput(format!(
                "`{name}` is {other}, which can't be read as text"
            )))
        }
    };
    let resp = authed(state, reqwest::Method::GET, &url)
        .await?
        .query(&query)
        .send()
        .await?;
    if !resp.status().is_success() {
        return json_or_error(resp).await.map(|_| String::new());
    }
    let bytes = resp.bytes().await?;
    if bytes.len() > MAX_DOWNLOAD_BYTES {
        return Err(AppError::InvalidInput(format!(
            "`{name}` is larger than {} MiB",
            MAX_DOWNLOAD_BYTES / 1024 / 1024
        )));
    }
    Ok(clip(&format!(
        "# {name}\n\n{}",
        String::from_utf8_lossy(&bytes)
    )))
}

fn is_text_mime(m: &str) -> bool {
    m.starts_with("text/")
        || matches!(
            m,
            "application/json"
                | "application/xml"
                | "application/x-yaml"
                | "application/javascript"
        )
}

/// MIME type for an upload, from the extension.
fn mime_for(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("html" | "htm") => "text/html",
        Some("md") => "text/markdown",
        Some("csv") => "text/csv",
        Some("txt") => "text/plain",
        Some("json") => "application/json",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    }
}

/// Upload a local file to Drive (optionally converting a spreadsheet,
/// document or CSV to its Google format). Natively confirmed.
pub async fn drive_upload<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    path: &str,
    folder_id: Option<&str>,
    convert: bool,
) -> AppResult<String> {
    if let Some(f) = folder_id {
        if !valid_drive_id(f) {
            return Err(AppError::InvalidInput("invalid Drive folder id".into()));
        }
    }
    ensure_allowed(state, Service::Drive).await?;
    let (p, data) =
        crate::security::files::read_bytes_checked(state, path, MAX_UPLOAD_BYTES as u64).await?;
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "upload".into());
    let mime = mime_for(&p);
    let target_mime = convert
        .then(|| match mime {
            m if m.contains("spreadsheetml") || m == "text/csv" => {
                Some("application/vnd.google-apps.spreadsheet")
            }
            m if m.contains("wordprocessingml") || m == "text/markdown" || m == "text/plain" => {
                Some("application/vnd.google-apps.document")
            }
            _ => None,
        })
        .flatten();
    let what = p.display().to_string();
    let confirmation = write_gate(
        app,
        state,
        Service::Drive,
        "drive_upload",
        ConfirmRequest {
            title: "Upload file to Google Drive?".into(),
            subject: format!("File:\n{what}"),
            details: vec![
                format!("Size: {} bytes", data.len()),
                format!("Destination: {}", folder_id.unwrap_or("My Drive")),
                if target_mime.is_some() {
                    "Converted to a Google Docs/Sheets file".into()
                } else {
                    "Uploaded as is".into()
                },
            ],
            tier: RiskTier::Mutating,
            source: Source::LlmTool,
            reasons: vec!["the file's contents leave this computer".into()],
            approve_label: "Upload".into(),
        },
        &what,
    )
    .await?;
    let mut meta = json!({ "name": name });
    if let Some(f) = folder_id {
        meta["parents"] = json!([f]);
    }
    if let Some(t) = target_mime {
        meta["mimeType"] = json!(t);
    }
    let form = reqwest::multipart::Form::new()
        .part(
            "metadata",
            reqwest::multipart::Part::text(meta.to_string())
                .mime_str("application/json; charset=UTF-8")?,
        )
        .part(
            "file",
            reqwest::multipart::Part::bytes(data).mime_str(mime)?,
        );
    let resp = authed(state, reqwest::Method::POST, DRIVE_UPLOAD)
        .await?
        .query(&[
            ("uploadType", "multipart"),
            ("fields", "id,name,webViewLink"),
        ])
        .multipart(form)
        .send()
        .await?;
    let result = json_or_error(resp).await;
    audit(
        state,
        Source::LlmTool,
        "drive_upload",
        &what,
        RiskTier::Mutating,
        if result.is_ok() {
            Decision::Allowed
        } else {
            Decision::Failed
        },
        confirmation,
        result.as_ref().err().map(ToString::to_string),
    )
    .await?;
    let v = result?;
    Ok(format!(
        "Uploaded `{}` to Google Drive: {}",
        v.get("name").and_then(Value::as_str).unwrap_or(&name),
        v.get("webViewLink").and_then(Value::as_str).unwrap_or("")
    ))
}

// ---------------------------------------------------- Developer docs ---

async fn dev_get(state: &AppState, url: &str, query: &[(&str, &str)]) -> AppResult<Value> {
    ensure_endpoint_allowed(url, state.settings.read().await.security.local_only).await?;
    let key = secret(state, DEV_KEY_ID).await?.ok_or_else(|| {
        AppError::InvalidInput(
            "save a Developer Knowledge API key in Settings → Google first".into(),
        )
    })?;
    // Key in a header, not the URL, so it never lands in logs or errors.
    let resp = state
        .http
        .get(url)
        .header("X-Goog-Api-Key", key)
        .query(query)
        .send()
        .await?;
    json_or_error(resp).await
}

/// Search Google's official developer documentation (Android, Firebase,
/// Google Cloud, Maps, Workspace APIs, web.dev, …).
pub async fn dev_docs_search<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    query: &str,
    max: u32,
) -> AppResult<String> {
    let query: String = query.chars().take(500).collect();
    read_gate(app, state, Service::DevDocs, "dev_docs_search", &query).await?;
    let size = max.clamp(1, 10).to_string();
    let v = dev_get(
        state,
        &format!("{DEV_KNOWLEDGE_API}/documents:searchDocumentChunks"),
        &[("query", query.as_str()), ("pageSize", &size)],
    )
    .await?;
    let results = v
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if results.is_empty() {
        return Ok("No matching documentation.".into());
    }
    let out: Vec<Value> = results
        .iter()
        .map(|r| {
            json!({
                "document": r.get("parent").and_then(Value::as_str).unwrap_or_default(),
                "content": r.get("content").and_then(Value::as_str).unwrap_or_default(),
            })
        })
        .collect();
    Ok(clip(
        &serde_json::to_string_pretty(&out).unwrap_or_default(),
    ))
}

/// Whether `name` is a Developer Knowledge document name that stays under
/// `/v1/documents/` (no traversal, query or fragment).
fn valid_doc_name(name: &str) -> bool {
    name.starts_with("documents/")
        && name.len() <= 500
        && !name.contains("..")
        && !name.contains("//")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/-_.~".contains(c))
}

/// Fetch one documentation page as Markdown.
pub async fn dev_docs_get<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    name: &str,
) -> AppResult<String> {
    if !valid_doc_name(name) {
        return Err(AppError::InvalidInput(
            "name must be a document name from dev_docs_search, e.g. documents/developer.android.com/…"
                .into(),
        ));
    }
    read_gate(app, state, Service::DevDocs, "dev_docs_get", name).await?;
    let v = dev_get(state, &format!("{DEV_KNOWLEDGE_API}/{name}"), &[]).await?;
    Ok(clip(&format!(
        "# {}\n{}\n\n{}",
        v.get("title").and_then(Value::as_str).unwrap_or(name),
        v.get("uri").and_then(Value::as_str).unwrap_or_default(),
        v.get("content").and_then(Value::as_str).unwrap_or_default()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_matches_rfc7636() {
        // RFC 7636 appendix B.
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(
            b"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        ));
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        let (v, c) = pkce();
        assert_eq!(v.len(), 64);
        assert_eq!(c, URL_SAFE_NO_PAD.encode(Sha256::digest(v.as_bytes())));
    }

    #[test]
    fn redirect_requires_matching_state() {
        let ok = parse_redirect(
            "GET /?state=abc&code=4%2F0Ax HTTP/1.1\r\nHost: x\r\n",
            "abc",
        );
        assert_eq!(ok.expect("ours").expect("code"), "4/0Ax");
        assert!(parse_redirect("GET /?state=evil&code=x HTTP/1.1\r\n", "abc").is_none());
        assert!(parse_redirect("GET /favicon.ico HTTP/1.1\r\n", "abc").is_none());
        assert!(parse_redirect("POST /?state=abc&code=x HTTP/1.1\r\n", "abc").is_none());
        assert!(matches!(
            parse_redirect("GET /?state=abc&error=access_denied HTTP/1.1\r\n", "abc"),
            Some(Err(AppError::NotApproved(_)))
        ));
        assert!(parse_redirect("garbage", "abc").is_none());
    }

    #[test]
    fn scopes_follow_ticked_services() {
        let mut g = GoogleSettings::default();
        assert!(scopes(&g).is_empty());
        g.gmail = true;
        assert!(scopes(&g).iter().all(|s| s.contains("gmail")));
        assert!(!scopes(&g).iter().any(|s| s.ends_with("gmail.send")));
        g.drive = true;
        assert_eq!(scopes(&g).len(), 4);
        // Never full Drive or Gmail access.
        assert!(!scopes(&g)
            .iter()
            .any(|s| s.ends_with("/auth/drive") || s.ends_with("mail.google.com/")));
    }

    #[test]
    fn mail_bodies_are_decoded() {
        let payload = json!({
            "mimeType": "multipart/alternative",
            "parts": [
                {"mimeType": "text/html", "body": {"data": URL_SAFE_NO_PAD.encode("<p>Hi&nbsp;<b>Paul</b></p><script>x()</script>")}},
            ]
        });
        assert_eq!(message_text(&payload).trim(), "Hi Paul");
        let plain = json!({"mimeType": "text/plain", "body": {"data": URL_SAFE_NO_PAD.encode("Plain body")}});
        assert_eq!(message_text(&plain), "Plain body");
    }

    #[test]
    fn ids_and_names_are_validated() {
        assert!(valid_drive_id("1AbC-d_e"));
        assert!(!valid_drive_id("../x"));
        assert!(!valid_drive_id("a?b"));
        assert!(valid_doc_name(
            "documents/developer.android.com/develop/ui/compose"
        ));
        assert!(!valid_doc_name("documents/../v1/other"));
        assert!(!valid_doc_name("documents/x?key=y"));
        assert!(!valid_doc_name("other/x"));
        assert!(!valid_doc_name("documents//evil.com"));
    }

    #[test]
    fn subjects_and_mime() {
        assert_eq!(encode_subject("Report"), "Report");
        assert!(encode_subject("Café").starts_with("=?UTF-8?B?"));
        assert_eq!(
            mime_for(std::path::Path::new("/x/Budget.XLSX")),
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        );
    }
}
