//! Voice: push-to-talk speech-to-text and Piper text-to-speech.
//!
//! * **STT** posts the recorded clip to a configurable faster-whisper server
//!   using the OpenAI-compatible `POST {stt_url}/v1/audio/transcriptions`
//!   (multipart `file`, `model`, `language`). The endpoint is subject to
//!   `local_only`, so audio stays on the LAN/tailnet by default.
//! * **TTS** runs Piper as a local subprocess through the executor's hardened
//!   spawn (cleared environment, timeout, output cap, process-group kill) with
//!   a fixed argv; the text goes in on stdin, never through a shell. The Piper
//!   program is chosen in settings, and changing it requires native
//!   confirmation, which is the trust decision for running it.
//! * Push-to-talk only; there is no always-on listening.

use crate::error::{AppError, AppResult};
use crate::security::audit::{AuditRecord, Confirmation, Decision};
use crate::security::elevation::find_in_path;
use crate::security::executor;
use crate::security::policy::{RiskTier, Source};
use crate::state::AppState;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;

/// Largest accepted recording.
pub const MAX_AUDIO_BYTES: usize = 25 * 1024 * 1024;
/// Largest text sent to TTS.
pub const MAX_TTS_CHARS: usize = 5000;

#[derive(Deserialize)]
struct Transcription {
    text: String,
}

/// Map a MIME type to a file name the STT server can sniff.
fn file_name_for(mime: &str) -> &'static str {
    match mime.split(';').next().unwrap_or("").trim() {
        "audio/ogg" => "audio.ogg",
        "audio/wav" | "audio/x-wav" => "audio.wav",
        "audio/mp4" | "audio/m4a" => "audio.m4a",
        "audio/mpeg" => "audio.mp3",
        _ => "audio.webm",
    }
}

/// Transcribe an audio clip.
pub async fn transcribe(state: &AppState, audio: Vec<u8>, mime: &str) -> AppResult<String> {
    let settings = state.settings.read().await.clone();
    let v = &settings.voice;
    if !v.enabled || v.stt_url.trim().is_empty() {
        return Err(AppError::Unavailable(
            "voice input is off: enable it and set a speech-to-text URL in Settings → Voice".into(),
        ));
    }
    if audio.is_empty() || audio.len() > MAX_AUDIO_BYTES {
        return Err(AppError::InvalidInput(
            "recording is empty or larger than 25 MiB".into(),
        ));
    }
    let base = v.stt_url.trim_end_matches('/');
    crate::ai::endpoint::ensure_endpoint_allowed(base, settings.security.local_only).await?;
    let part = reqwest::multipart::Part::bytes(audio)
        .file_name(file_name_for(mime))
        .mime_str(mime.split(';').next().unwrap_or("audio/webm").trim())
        .map_err(|e| AppError::InvalidInput(format!("audio type: {e}")))?;
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", v.whisper_model.clone())
        .text("language", v.language.clone());
    let resp = state
        .http
        .post(format!("{base}/v1/audio/transcriptions"))
        .multipart(form)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| AppError::Unavailable(format!("speech-to-text server at {base}: {e}")))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Unavailable(format!(
            "speech-to-text returned {status}: {}",
            body.chars().take(200).collect::<String>()
        )));
    }
    Ok(resp.json::<Transcription>().await?.text.trim().to_string())
}

/// Resolve the Piper executable from settings.
fn piper_program(path: &str) -> AppResult<PathBuf> {
    let p = PathBuf::from(path);
    let found = if p.is_absolute() {
        p.is_file().then_some(p)
    } else {
        find_in_path(path)
    };
    found.ok_or_else(|| {
        AppError::Unavailable(format!(
            "Piper (`{path}`) was not found; install piper-tts or set its path in Settings → Voice"
        ))
    })
}

/// Speak `text` with Piper and return WAV bytes.
pub async fn speak(state: &AppState, cache_dir: PathBuf, text: &str) -> AppResult<Vec<u8>> {
    let settings = state.settings.read().await.clone();
    let v = &settings.voice;
    if !v.enabled {
        return Err(AppError::Unavailable(
            "voice output is off (Settings → Voice)".into(),
        ));
    }
    let text: String = text.chars().take(MAX_TTS_CHARS).collect();
    if text.trim().is_empty() {
        return Err(AppError::InvalidInput("nothing to speak".into()));
    }
    let program = piper_program(&v.piper_path)?;
    tokio::fs::create_dir_all(&cache_dir).await?;
    let out = cache_dir.join(format!("tts-{}.wav", uuid::Uuid::new_v4().simple()));
    // Fixed argv, no shell. `--model` accepts a voice name or an .onnx path.
    let args = vec![
        "--model".to_string(),
        v.tts_voice.clone(),
        "--output_file".to_string(),
        out.display().to_string(),
    ];
    let cwd = cache_dir.clone();
    let started = std::time::Instant::now();
    let run = executor::run_process_with_input(
        &program.to_string_lossy(),
        &args,
        &cwd,
        Duration::from_secs(settings.security.command_timeout_secs),
        64 * 1024,
        Some(text.into_bytes()),
    )
    .await;
    let exit_code = run.as_ref().ok().and_then(|o| o.exit_code);
    state
        .audit
        .record(AuditRecord {
            id: uuid::Uuid::new_v4().to_string(),
            source: Source::User,
            action: "tts".into(),
            command: format!("{} --model {}", program.display(), v.tts_voice),
            cwd: None,
            tier: RiskTier::ReadOnly,
            decision: if matches!(exit_code, Some(0)) {
                Decision::Allowed
            } else {
                Decision::Failed
            },
            confirmation: Confirmation::NotRequired,
            exit_code,
            duration_ms: u64::try_from(started.elapsed().as_millis()).ok(),
            detail: run.as_ref().err().map(ToString::to_string),
        })
        .await?;
    let output = run?;
    if output.exit_code != Some(0) {
        let _ = tokio::fs::remove_file(&out).await;
        return Err(AppError::Execution(format!(
            "Piper failed: {}",
            output.stderr.chars().take(300).collect::<String>()
        )));
    }
    let bytes = tokio::fs::read(&out).await?;
    let _ = tokio::fs::remove_file(&out).await;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_follow_mime() {
        assert_eq!(file_name_for("audio/webm;codecs=opus"), "audio.webm");
        assert_eq!(file_name_for("audio/ogg"), "audio.ogg");
        assert_eq!(file_name_for("audio/wav"), "audio.wav");
        assert_eq!(file_name_for("weird"), "audio.webm");
    }

    #[test]
    fn missing_piper_is_a_clear_error() {
        assert!(matches!(
            piper_program("definitely-not-piper-omnix"),
            Err(AppError::Unavailable(_))
        ));
    }
}
