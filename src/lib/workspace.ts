/**
 * Workspace dock: side tasks, prompt library, pinboard and the situation
 * brief. Pure helpers live here so they can be unit-tested; the components
 * under `components/workspace/` render them.
 *
 * Prompts and pins are per-user conveniences kept in `localStorage`. Every
 * read and write is guarded: storage can be missing or throw, and the dock
 * must still work (with the built-in prompts and no pins).
 */
import type { PerformanceData, SystemControlData } from './types';
import { bytes, pct } from './format';
import { recordClip } from './notepad';

// ---------------------------------------------------------------- storage

function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = globalThis.localStorage?.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function writeJson(key: string, value: unknown): void {
  try {
    globalThis.localStorage?.setItem(key, JSON.stringify(value));
  } catch {
    // Storage full or blocked: the in-memory state still works this session.
  }
}

// ---------------------------------------------------------------- side tasks

export type TaskKind = 'ai' | 'command';
export type TaskStatus = 'running' | 'done' | 'error' | 'cancelled';

/** One side task card. `output` is untrusted model/command text. */
export interface SideTask {
  id: string;
  kind: TaskKind;
  title: string;
  instruction: string;
  /** Data the task was given (a reply, a metrics snapshot). */
  context?: string;
  output: string;
  notes: string[];
  status: TaskStatus;
  started: number;
  finished?: number;
}

let taskSeq = 0;
/** Task ids the backend accepts: ASCII alphanumerics and `-`, ≤ 64 chars. */
export function newTaskId(now = Date.now()): string {
  taskSeq = (taskSeq + 1) % 1_000_000;
  return `t${now.toString(36)}-${taskSeq}`;
}

/** Short card title from free text. */
export function taskTitle(text: string, max = 48): string {
  const flat = text.replace(/\s+/g, ' ').trim();
  return flat.length > max ? `${flat.slice(0, max - 1)}…` : flat;
}

/**
 * Drop `<think>…</think>` reasoning some local models emit. An unclosed
 * block (still streaming) hides everything after it.
 */
export function stripThinking(text: string): string {
  let out = text.replace(/<think>[\s\S]*?<\/think>/g, '');
  const open = out.indexOf('<think>');
  if (open >= 0) out = out.slice(0, open);
  return out.trimStart();
}

/** Actions offered on a chat reply; each becomes a side task over that reply. */
export const REPLY_ACTIONS: { label: string; icon: string; instruction: string }[] = [
  { label: 'Summarize', icon: '📝', instruction: 'Summarize this in 3–5 bullet points.' },
  { label: 'Action items', icon: '✅', instruction: 'List the concrete action items or next steps in this, as a checklist. If there are none, say so.' },
  { label: 'Explain simply', icon: '💡', instruction: 'Explain this in plain language for a non-technical reader, in one short paragraph.' },
  { label: 'Draft email', icon: '✉️', instruction: 'Turn this into a short, professional email I could send to a colleague. Include a subject line.' },
  { label: 'Critique', icon: '🔍', instruction: 'Review this critically: what is missing, risky or likely wrong? Be specific and brief.' }
];

// ---------------------------------------------------------------- prompt library

/** A reusable prompt. `{{input}}` is replaced by what the user types. */
export interface SavedPrompt {
  id: string;
  title: string;
  icon: string;
  text: string;
  /** Where it runs by default. */
  target: 'chat' | 'task';
  builtin?: boolean;
}

export const DEFAULT_PROMPTS: SavedPrompt[] = [
  { id: 'huddle', icon: '🗣️', title: 'Shift huddle agenda', target: 'task', builtin: true,
    text: 'Draft a 10-minute shift huddle agenda for a behavioral health unit covering safety, staffing, census, and follow-ups. Topic focus: {{input}}' },
  { id: 'incident', icon: '🧾', title: 'Incident summary', target: 'task', builtin: true,
    text: 'Write a neutral, factual incident summary (what happened, when, who was notified, immediate actions, follow-up) from these notes. Do not add details that are not in the notes:\n\n{{input}}' },
  { id: 'sop', icon: '📋', title: 'SOP outline', target: 'task', builtin: true,
    text: 'Outline a standard operating procedure for: {{input}}. Include purpose, scope, responsibilities, numbered steps, and documentation requirements.' },
  { id: 'workbook', icon: '📊', title: 'Build a workbook', target: 'chat', builtin: true,
    text: 'Create an .xlsx workbook for {{input}}. Propose the sheets and columns first, then build it with the create_workbook tool.' },
  { id: 'rewrite', icon: '✍️', title: 'Rewrite clearly', target: 'task', builtin: true,
    text: 'Rewrite this so it is clear, concise and professional. Keep the meaning:\n\n{{input}}' },
  { id: 'explain-cmd', icon: '🐧', title: 'Explain a command', target: 'task', builtin: true,
    text: 'Explain what this Linux command does, flag by flag, and any risks: {{input}}' }
];

const PROMPTS_KEY = 'omnix.workspace.prompts.v1';

export function loadPrompts(): SavedPrompt[] {
  const saved = readJson<SavedPrompt[] | null>(PROMPTS_KEY, null);
  if (!Array.isArray(saved)) return DEFAULT_PROMPTS.map((p) => ({ ...p }));
  return saved.filter(
    (p) => p && typeof p.id === 'string' && typeof p.title === 'string' && typeof p.text === 'string'
  );
}

export function savePrompts(prompts: SavedPrompt[]): void {
  writeJson(PROMPTS_KEY, prompts);
}

/** Fill a prompt's `{{input}}` slots (or append the input when it has none). */
export function fillPrompt(text: string, input: string): string {
  const value = input.trim();
  if (text.includes('{{input}}')) return text.replaceAll('{{input}}', value).trim();
  return value ? `${text.trim()}\n\n${value}` : text.trim();
}

// ---------------------------------------------------------------- pinboard

export interface Pin {
  id: string;
  /** Untrusted text (model output or a command result): rendered sanitized. */
  text: string;
  source: string;
  pinned: number;
}

const PINS_KEY = 'omnix.workspace.pins.v1';
/** Pins kept; the oldest drop off. */
export const MAX_PINS = 50;
/** Characters kept per pin. */
export const MAX_PIN_CHARS = 20_000;

export function loadPins(): Pin[] {
  const saved = readJson<Pin[]>(PINS_KEY, []);
  return Array.isArray(saved) ? saved.filter((p) => p && typeof p.text === 'string') : [];
}

export function savePins(pins: Pin[]): void {
  writeJson(PINS_KEY, pins);
}

/** Add a pin at the front, skipping exact duplicates and capping the list. */
export function addPin(pins: Pin[], text: string, source: string, now = Date.now()): Pin[] {
  const clipped = text.trim().slice(0, MAX_PIN_CHARS);
  if (!clipped) return pins;
  const rest = pins.filter((p) => p.text !== clipped);
  return [{ id: `p${now.toString(36)}${rest.length}`, text: clipped, source, pinned: now }, ...rest].slice(0, MAX_PINS);
}

// ---------------------------------------------------------------- situation brief

export const BRIEF_INSTRUCTION =
  'Write a short situation brief for the owner of this computer from the snapshot. Start with one line: ' +
  'overall status (OK / Watch / Act). Then at most 6 bullets: anything firing or failing, resource pressure ' +
  '(CPU, memory, disk, GPU/VRAM), which AI models are loaded and where, and what is scheduled next. End with ' +
  'at most 3 suggested actions. Only use facts from the snapshot.';

/**
 * Plain-text snapshot of the host and ops state for the situation brief.
 * Only local metrics and rule names go in; no chat text or memories.
 */
export function briefContext(
  perf: PerformanceData | null,
  ops: SystemControlData | null,
  now = new Date()
): string {
  const lines: string[] = [`Snapshot taken ${now.toISOString()}`];
  if (perf) {
    const h = perf.host;
    const mem = (h.memory_used / Math.max(1, h.memory_total)) * 100;
    lines.push(
      `CPU ${pct(h.cpu)} (load ${h.load.map((l) => l.toFixed(2)).join(' / ')}), memory ${pct(mem)} of ${bytes(h.memory_total)}, swap ${bytes(h.swap_used)} of ${bytes(h.swap_total)}, uptime ${Math.floor(h.uptime / 3600)} h, ${h.processes} processes`
    );
    for (const d of h.disks.filter((d) => !d.removable).slice(0, 6)) {
      lines.push(`Disk ${d.mount}: ${pct((d.used / Math.max(1, d.total)) * 100)} used, ${bytes(d.total - d.used)} free`);
    }
    if (h.sensors[0]) lines.push(`Hottest sensor: ${h.sensors[0].label} ${h.sensors[0].temperature.toFixed(0)} °C`);
    for (const g of perf.gpus) {
      const vram = g.memory_total ? `, VRAM ${bytes(g.memory_used)} / ${bytes(g.memory_total)}` : '';
      const temp = g.temperature != null ? `, ${g.temperature.toFixed(0)} °C` : '';
      lines.push(`GPU ${g.index} ${g.vendor} ${g.name}: load ${pct(g.utilization)}${vram}${temp}${g.note ? ` (${g.note})` : ''}`);
    }
    const loaded = perf.models.loaded ?? [];
    lines.push(
      loaded.length
        ? `Loaded models: ${loaded.map((m) => `${m.name} (${bytes(m.size_vram)} VRAM, ${m.gpu_percent.toFixed(0)}% on GPU)`).join('; ')}`
        : `Loaded models: none${perf.models.error ? ` (Ollama: ${perf.models.error})` : ''}`
    );
    lines.push(`Configured chat model: ${perf.models.configured || 'not set'}`);
    const a = perf.agent;
    lines.push(`Assistant since start: ${a.turns} turns, ${a.errors} errors, ${a.cancelled} cancelled`);
  } else {
    lines.push('Host metrics: unavailable');
  }
  if (ops?.available) {
    const firing = ops.alerts.filter((a) => a.alert.enabled && a.alert.state.firing);
    lines.push(
      firing.length
        ? `Alerts firing: ${firing.map((a) => `${a.alert.name} (${a.description})`).join('; ')}`
        : `Alerts: ${ops.alerts.filter((a) => a.alert.enabled).length} enabled, none firing`
    );
    const next = ops.tasks
      .filter((t) => t.task.enabled && t.task.next_run)
      .sort((a, b) => String(a.task.next_run).localeCompare(String(b.task.next_run)))
      .slice(0, 3);
    if (next.length) lines.push(`Next scheduled: ${next.map((t) => `${t.task.name} at ${t.task.next_run}`).join('; ')}`);
    const failed = ops.activity.filter((e) => !e.ok).slice(0, 3);
    if (failed.length) lines.push(`Recent failures: ${failed.map((e) => `${e.name} (${e.ts}): ${e.summary}`).join('; ')}`);
  }
  return lines.join('\n');
}

// ---------------------------------------------------------------- clipboard

/**
 * Copy text with the web clipboard API. Returns false when it isn't allowed.
 * Either way the text is kept in the notepad's clip history, so it can still
 * be pasted from there.
 */
export async function copyText(text: string): Promise<boolean> {
  recordClip(text);
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
