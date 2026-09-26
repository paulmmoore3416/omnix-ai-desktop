/**
 * OMNIX avatar colour language.
 *
 * Three independent layers, so a mood and a problem can be shown at once:
 * - Mood: what OMNIX is doing. Colours the core and inner rings.
 * - Condition: a standing problem. Colours the outer halo and raises a banner.
 * - Signal: a one-off event. Fires an expanding ripple from the core.
 *
 * The in-app colour key, the docs, and the tests all read these tables, so a
 * colour change here is a colour change everywhere.
 */

export type Emotion =
  | 'idle' | 'thinking' | 'speaking' | 'working' | 'happy' | 'excited' | 'focused'
  | 'confused' | 'success' | 'error' | 'listening' | 'processing' | 'searching' | 'remembering';

/** Moods the avatar can display: every Emotion plus states it enters on its own. */
export type Mood = Emotion | 'dormant';

export type Condition = 'nominal' | 'offline' | 'strain' | 'security' | 'network';

export type SignalKind = 'tool' | 'ok' | 'fail' | 'notice';
/**
 * `id` must change for every event so repeats of the same kind still fire.
 * `ref` ties a tool call to its result (the agent's tool-call id) and `label`
 * names it; together they drive the orbiting tool satellites.
 */
export type Signal = { kind: SignalKind; id: number; label?: string; ref?: string };

export interface Swatch {
  color: string;
  label: string;
  meaning: string;
  effect: string;
}

export const MOODS: Record<Mood, Swatch> = {
  idle: { color: '#29d8ff', label: 'Standby', meaning: 'Ready and waiting', effect: 'Slow ring drift, calm core breathing' },
  listening: { color: '#4f8bff', label: 'Listening', meaning: 'Recording your voice', effect: 'Waveform and equalizer follow your mic; sound particles drift in' },
  thinking: { color: '#8b7bff', label: 'Thinking', meaning: 'Working out an answer', effect: 'Gyroscope rings turn in 3D, linked data motes orbit' },
  processing: { color: '#c36bff', label: 'Processing', meaning: 'Transcribing or computing', effect: 'Counter-rotating rings, fast radar sweep' },
  searching: { color: '#00c2a8', label: 'Searching', meaning: 'Looking through memory, files or the system', effect: 'Wide radar sweep that lights up contacts' },
  remembering: { color: '#f0a8ff', label: 'Remembering', meaning: 'Saving something to long-term memory', effect: 'Particles spiral into the core, which flashes as each lands' },
  working: { color: '#ffb02e', label: 'Executing', meaning: 'Running a command or tool', effect: 'Rings step like a gear, arcs crackle from the core' },
  speaking: { color: '#4ff5d2', label: 'Speaking', meaning: 'Replying to you', effect: 'Equalizer pulses round the core, sparks stream toward the chat' },
  focused: { color: '#d6ecff', label: 'Focused', meaning: 'Concentrating on a task', effect: 'Rings tighten, sweep narrows' },
  happy: { color: '#ffd84a', label: 'Happy', meaning: 'Task finished, all good', effect: 'Warm glow, happy bounce' },
  excited: { color: '#ff5ea8', label: 'Excited', meaning: 'Something went really well', effect: 'Fast spin with sparks' },
  success: { color: '#3dff95', label: 'Success', meaning: 'Request completed', effect: 'Double shockwave and a burst of confetti' },
  confused: { color: '#9fb4c8', label: 'Confused', meaning: "Didn't catch that / unclear input", effect: 'Rings wobble back and forth' },
  error: { color: '#ff3d4f', label: 'Error', meaning: 'Request failed (details in the pop-up)', effect: 'Glitch shake and flicker' },
  dormant: { color: '#3553b8', label: 'Dormant', meaning: 'Idle for a while; click to wake', effect: 'Dimmed, very slow breathing' }
};

export const CONDITIONS: Record<Exclude<Condition, 'nominal'>, Swatch> = {
  offline: { color: '#7c8594', label: 'Backend offline', meaning: 'The OMNIX core stopped responding', effect: 'Grey halo, stuttering flicker' },
  strain: { color: '#ff7a1a', label: 'High system load', meaning: 'CPU above 85% or memory above 90%', effect: 'Orange halo with fast warning ticks' },
  security: { color: '#ff2bd6', label: 'Blocked by policy', meaning: 'A command was denied, not approved, or kept local-only', effect: 'Magenta halo with hazard chevrons' },
  network: { color: '#d4ff3a', label: 'Connection issue', meaning: 'AI provider or network unreachable', effect: 'Chartreuse halo, broken dashed ring' }
};

export const SIGNALS: Record<SignalKind, Swatch> = {
  tool: { color: '#ffb02e', label: 'Tool dispatched', meaning: 'OMNIX asked to use a tool', effect: 'Amber ripple' },
  ok: { color: '#3dff95', label: 'Tool succeeded', meaning: 'A tool call returned OK', effect: 'Green ripple' },
  fail: { color: '#ff3d4f', label: 'Tool failed', meaning: 'A tool call returned an error', effect: 'Red ripple' },
  notice: { color: '#ffffff', label: 'Notice', meaning: 'Informational message from the agent', effect: 'White ripple' }
};

/** Maps a backend `AppError.kind` to the condition it should raise, if any. */
export function conditionForError(kind: string | undefined): Condition | null {
  switch (kind) {
    case 'policy_denied':
    case 'not_approved':
    case 'local_only':
      return 'security';
    case 'http':
    case 'unavailable':
      return 'network';
    default:
      return null;
  }
}

/** `#rrggbb` → `rgba(r, g, b, a)`. */
export function rgba(hex: string, a: number): string {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${a})`;
}

/** Linear mix of two `#rrggbb` colours; `t = 0` is `a`, `t = 1` is `b`. */
export function mix(a: string, b: string, t: number): string {
  const pa = parseInt(a.slice(1), 16);
  const pb = parseInt(b.slice(1), 16);
  const ch = (s: number) => Math.round(((pa >> s) & 255) * (1 - t) + ((pb >> s) & 255) * t);
  return `#${((ch(16) << 16) | (ch(8) << 8) | ch(0)).toString(16).padStart(6, '0')}`;
}
