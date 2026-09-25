//! Optional shipping of the audit log to Grafana Loki.
//!
//! Off by default (`observability.loki_url` empty). When enabled, every audit
//! line (already redacted, exactly as written to `audit.jsonl`) is pushed to
//! `POST {loki_url}/loki/api/v1/push` in small batches. Shipping the chain to
//! an external store anchors it: tail truncation of the local file becomes
//! detectable by comparing against Loki. The endpoint obeys `local_only`.

use crate::error::AppResult;
use crate::state::AppState;
use serde_json::json;
use std::time::Duration;

/// Maximum buffered lines while Loki is unreachable (oldest dropped first).
const MAX_BUFFER: usize = 5000;
/// Flush interval.
const FLUSH_EVERY: Duration = Duration::from_secs(2);

/// Build the Loki push payload.
pub fn payload(host: &str, batch: &[(u128, String)]) -> serde_json::Value {
    json!({
        "streams": [{
            "stream": { "app": "omnix", "log": "audit", "host": host },
            "values": batch.iter().map(|(ts, line)| json!([ts.to_string(), line])).collect::<Vec<_>>()
        }]
    })
}

/// (Re)configure shipping from current settings. Safe to call repeatedly.
pub async fn configure(state: &AppState) -> AppResult<()> {
    let settings = state.settings.read().await.clone();
    let url = settings
        .observability
        .loki_url
        .trim()
        .trim_end_matches('/')
        .to_string();
    if url.is_empty() {
        state.audit.set_sink(None);
        return Ok(());
    }
    crate::ai::endpoint::ensure_endpoint_allowed(&url, settings.security.local_only).await?;
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1024);
    let http = state.http.clone();
    let host = sysinfo::System::host_name().unwrap_or_else(|| "unknown".into());
    tauri::async_runtime::spawn(async move {
        let push = format!("{url}/loki/api/v1/push");
        let mut buffer: Vec<(u128, String)> = Vec::new();
        let mut tick = tokio::time::interval(FLUSH_EVERY);
        loop {
            tokio::select! {
                line = rx.recv() => match line {
                    Some(l) => {
                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_nanos())
                            .unwrap_or(0);
                        buffer.push((ts, l));
                        if buffer.len() > MAX_BUFFER {
                            let excess = buffer.len() - MAX_BUFFER;
                            buffer.drain(..excess);
                        }
                    }
                    // Sender replaced/removed: flush what we have and stop.
                    None => {
                        let _ = send(&http, &push, &host, &buffer).await;
                        break;
                    }
                },
                _ = tick.tick() => {
                    if !buffer.is_empty() && send(&http, &push, &host, &buffer).await {
                        buffer.clear();
                    }
                }
            }
        }
    });
    state.audit.set_sink(Some(tx));
    tracing::info!("audit log shipping to Loki enabled");
    Ok(())
}

async fn send(http: &reqwest::Client, url: &str, host: &str, batch: &[(u128, String)]) -> bool {
    if batch.is_empty() {
        return true;
    }
    match http
        .post(url)
        .json(&payload(host, batch))
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => true,
        Ok(r) => {
            tracing::warn!(status = %r.status(), "Loki rejected audit batch; will retry");
            false
        }
        Err(e) => {
            tracing::warn!(error = %e, "Loki unreachable; will retry");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_shape_matches_loki_push_api() {
        let p = payload("box", &[(1_700_000_000_000_000_000, "{\"a\":1}".into())]);
        assert_eq!(p["streams"][0]["stream"]["app"], "omnix");
        assert_eq!(p["streams"][0]["values"][0][0], "1700000000000000000");
        assert_eq!(p["streams"][0]["values"][0][1], "{\"a\":1}");
    }
}
