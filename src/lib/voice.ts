/**
 * Push-to-talk recording and Piper playback.
 *
 * Audio is only ever sent to the backend (`voice_transcribe`), which forwards
 * it to the configured speech-to-text endpoint under `local_only` rules. The
 * CSP prevents the webview from sending it anywhere else.
 */
import { invoke } from '@tauri-apps/api/core';

/** An in-progress recording. */
export interface Recording {
  /** Stop recording and resolve with the captured audio. */
  stop(): Promise<{ bytes: Uint8Array; mime: string }>;
}

/** Start recording from the default microphone. */
export async function startRecording(): Promise<Recording> {
  if (!navigator.mediaDevices?.getUserMedia) {
    throw new Error('Microphone capture is not supported by this webview');
  }
  const stream = await navigator.mediaDevices.getUserMedia({ audio: true, video: false });
  const recorder = new MediaRecorder(stream);
  const chunks: Blob[] = [];
  recorder.ondataavailable = (e) => {
    if (e.data.size > 0) chunks.push(e.data);
  };
  recorder.start();
  return {
    stop: () =>
      new Promise((resolve, reject) => {
        recorder.onstop = async () => {
          stream.getTracks().forEach((t) => t.stop());
          const mime = recorder.mimeType || 'audio/webm';
          try {
            const blob = new Blob(chunks, { type: mime });
            resolve({ bytes: new Uint8Array(await blob.arrayBuffer()), mime });
          } catch (e) {
            reject(e);
          }
        };
        recorder.stop();
      })
  };
}

/** Transcribe audio via the backend (raw bytes body, MIME in a header). */
export function transcribe(bytes: Uint8Array, mime: string): Promise<string> {
  return invoke<string>('voice_transcribe', bytes, { headers: { 'x-audio-mime': mime } });
}

/** Remove Markdown syntax so TTS doesn't read symbols aloud. */
export function stripMarkdown(md: string): string {
  return md
    .replace(/```[\s\S]*?```/g, ' (code omitted) ')
    .replace(/`([^`]*)`/g, '$1')
    .replace(/!\[[^\]]*\]\([^)]*\)/g, '')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/^#+\s*/gm, '')
    .replace(/[*_~>]/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

/** Speak text with Piper (backend) and play the returned WAV. */
export async function speak(text: string): Promise<void> {
  const wav = await invoke<ArrayBuffer>('voice_speak', { text: stripMarkdown(text) });
  const url = URL.createObjectURL(new Blob([wav], { type: 'audio/wav' }));
  const audio = new Audio(url);
  audio.onended = () => URL.revokeObjectURL(url);
  await audio.play();
}
