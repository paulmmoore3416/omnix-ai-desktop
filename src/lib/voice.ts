/**
 * Push-to-talk recording and Piper playback.
 *
 * Audio is only ever sent to the backend (`voice_transcribe`), which forwards
 * it to the configured speech-to-text endpoint under `local_only` rules. The
 * CSP prevents the webview from sending it anywhere else.
 *
 * Capture uses the Web Audio API and encodes 16 kHz mono PCM WAV in-process.
 * That sidesteps MediaRecorder codec gaps in WebKitGTK (missing GStreamer
 * plugins) and is the format Whisper servers decode fastest. MediaRecorder is
 * only used as a fallback when Web Audio is unavailable.
 */
import { invoke } from '@tauri-apps/api/core';

/** Sample rate Whisper models are trained on. */
export const TARGET_SAMPLE_RATE = 16_000;

/** An in-progress recording. */
export interface Recording {
  /** Stop recording and resolve with the captured audio. */
  stop(): Promise<{ bytes: Uint8Array; mime: string; durationMs: number }>;
  /** Stop recording and discard the audio. */
  cancel(): void;
}

export interface RecordingOptions {
  /** Called ~30×/s with the input level, 0..1 (for meters and the avatar). */
  onLevel?: (level: number) => void;
}

type AudioContextCtor = typeof AudioContext;

function audioContextCtor(): AudioContextCtor | undefined {
  const w = globalThis as unknown as { AudioContext?: AudioContextCtor; webkitAudioContext?: AudioContextCtor };
  return w.AudioContext ?? w.webkitAudioContext;
}

/** Turn a getUserMedia failure into an actionable message. */
export function describeMicError(e: unknown): string {
  const name = (e as { name?: string })?.name;
  switch (name) {
    case 'NotAllowedError':
    case 'SecurityError':
      return 'microphone access was denied. Enable voice in Settings → Voice and check your OS privacy settings';
    case 'NotFoundError':
    case 'OverconstrainedError':
      return 'no microphone was found. Plug one in or pick a default input device in your sound settings';
    case 'NotReadableError':
    case 'AbortError':
      return 'the microphone is busy or unavailable (another app may be using it)';
    default:
      return e instanceof Error ? e.message : String(e);
  }
}

/** Start recording from the default microphone. */
export async function startRecording(opts: RecordingOptions = {}): Promise<Recording> {
  if (!navigator.mediaDevices?.getUserMedia) {
    throw new Error('Microphone capture is not supported by this webview');
  }
  let stream: MediaStream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({
      audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true },
      video: false
    });
  } catch (e) {
    throw new Error(describeMicError(e));
  }
  const Ctx = audioContextCtor();
  if (Ctx) {
    try {
      return await startPcmRecording(stream, Ctx, opts);
    } catch {
      // Fall through to MediaRecorder.
    }
  }
  if (typeof MediaRecorder !== 'undefined') return startMediaRecorder(stream);
  stream.getTracks().forEach((t) => t.stop());
  throw new Error('No audio recording API is available in this webview');
}

/** Web Audio capture → WAV. Also drives the level meter. */
async function startPcmRecording(
  stream: MediaStream,
  Ctx: AudioContextCtor,
  opts: RecordingOptions
): Promise<Recording> {
  const ctx = new Ctx();
  // A context created outside a user gesture (global shortcut) may start suspended.
  if (ctx.state === 'suspended') await ctx.resume();
  const source = ctx.createMediaStreamSource(stream);
  const analyser = ctx.createAnalyser();
  analyser.fftSize = 512;
  analyser.smoothingTimeConstant = 0.6;
  // ScriptProcessor is deprecated but universally supported, and unlike an
  // AudioWorklet it needs no separately loaded module (CSP: script-src 'self').
  const processor = ctx.createScriptProcessor(4096, 1, 1);
  const mute = ctx.createGain();
  mute.gain.value = 0; // processors only run when connected to the destination
  const chunks: Float32Array[] = [];
  processor.onaudioprocess = (e) => {
    chunks.push(new Float32Array(e.inputBuffer.getChannelData(0)));
  };
  source.connect(analyser);
  source.connect(processor);
  processor.connect(mute);
  mute.connect(ctx.destination);

  const started = performance.now();
  let raf = 0;
  const levelBuf = new Float32Array(analyser.fftSize);
  const tick = () => {
    analyser.getFloatTimeDomainData(levelBuf);
    let sum = 0;
    for (let i = 0; i < levelBuf.length; i++) sum += levelBuf[i] * levelBuf[i];
    // RMS of speech sits around 0.02–0.2; scale into a useful 0..1 range.
    opts.onLevel?.(Math.min(1, Math.sqrt(sum / levelBuf.length) * 6));
    raf = requestAnimationFrame(tick);
  };
  if (opts.onLevel) raf = requestAnimationFrame(tick);

  let closed = false;
  const teardown = () => {
    if (closed) return;
    closed = true;
    cancelAnimationFrame(raf);
    opts.onLevel?.(0);
    processor.onaudioprocess = null;
    source.disconnect();
    processor.disconnect();
    analyser.disconnect();
    mute.disconnect();
    stream.getTracks().forEach((t) => t.stop());
    void ctx.close().catch(() => {});
  };

  return {
    async stop() {
      const durationMs = performance.now() - started;
      const rate = ctx.sampleRate;
      teardown();
      const pcm = concat(chunks);
      const bytes = encodeWav(downsample(pcm, rate, TARGET_SAMPLE_RATE), TARGET_SAMPLE_RATE);
      return { bytes, mime: 'audio/wav', durationMs };
    },
    cancel: teardown
  };
}

/** Fallback: compressed capture with MediaRecorder (no level meter). */
function startMediaRecorder(stream: MediaStream): Recording {
  const recorder = new MediaRecorder(stream);
  const chunks: Blob[] = [];
  recorder.ondataavailable = (e) => {
    if (e.data.size > 0) chunks.push(e.data);
  };
  const started = performance.now();
  recorder.start();
  const release = () => stream.getTracks().forEach((t) => t.stop());
  return {
    stop: () =>
      new Promise((resolve, reject) => {
        const durationMs = performance.now() - started;
        recorder.onstop = async () => {
          release();
          const mime = recorder.mimeType || 'audio/webm';
          try {
            const blob = new Blob(chunks, { type: mime });
            resolve({ bytes: new Uint8Array(await blob.arrayBuffer()), mime, durationMs });
          } catch (e) {
            reject(e);
          }
        };
        recorder.stop();
      }),
    cancel() {
      recorder.onstop = null;
      if (recorder.state !== 'inactive') recorder.stop();
      release();
    }
  };
}

function concat(chunks: Float32Array[]): Float32Array {
  const out = new Float32Array(chunks.reduce((n, c) => n + c.length, 0));
  let offset = 0;
  for (const c of chunks) {
    out.set(c, offset);
    offset += c.length;
  }
  return out;
}

/** Downsample by box-averaging each output sample's source window (cheap anti-aliasing). */
export function downsample(input: Float32Array, fromRate: number, toRate: number): Float32Array {
  if (toRate >= fromRate) return input;
  const ratio = fromRate / toRate;
  const out = new Float32Array(Math.floor(input.length / ratio));
  for (let i = 0; i < out.length; i++) {
    const start = Math.floor(i * ratio);
    const end = Math.min(input.length, Math.floor((i + 1) * ratio));
    let sum = 0;
    for (let j = start; j < end; j++) sum += input[j];
    out[i] = end > start ? sum / (end - start) : 0;
  }
  return out;
}

/** Encode mono float samples as 16-bit PCM WAV. */
export function encodeWav(samples: Float32Array, sampleRate: number): Uint8Array {
  const buf = new ArrayBuffer(44 + samples.length * 2);
  const v = new DataView(buf);
  const str = (o: number, s: string) => {
    for (let i = 0; i < s.length; i++) v.setUint8(o + i, s.charCodeAt(i));
  };
  str(0, 'RIFF');
  v.setUint32(4, 36 + samples.length * 2, true);
  str(8, 'WAVE');
  str(12, 'fmt ');
  v.setUint32(16, 16, true); // fmt chunk size
  v.setUint16(20, 1, true); // PCM
  v.setUint16(22, 1, true); // mono
  v.setUint32(24, sampleRate, true);
  v.setUint32(28, sampleRate * 2, true); // byte rate
  v.setUint16(32, 2, true); // block align
  v.setUint16(34, 16, true); // bits per sample
  str(36, 'data');
  v.setUint32(40, samples.length * 2, true);
  for (let i = 0; i < samples.length; i++) {
    const s = Math.max(-1, Math.min(1, samples[i]));
    v.setInt16(44 + i * 2, s < 0 ? s * 0x8000 : s * 0x7fff, true);
  }
  return new Uint8Array(buf);
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
  await new Promise<void>((resolve, reject) => {
    audio.onended = () => resolve();
    audio.onerror = () => reject(new Error('could not play synthesized audio'));
    audio.play().catch(reject);
  }).finally(() => URL.revokeObjectURL(url));
}
