/**
 * Workspace notepad and clipboard history.
 *
 * Notes are Markdown written by the user; clips are the last things copied
 * with OMNIX's own Copy buttons. Both are per-user conveniences kept in
 * `localStorage` (like pins): every read and write is guarded, and the
 * notepad still works for the session when storage is missing or full.
 */

export interface Note {
  id: string;
  text: string;
  updated: number;
  /** Kept indexed in kb-core (`notes` knowledge base) while true. */
  synced?: boolean;
}

export interface Clip {
  id: string;
  text: string;
  at: number;
}

const NOTES_KEY = 'omnix.notepad.v1';
const CLIPS_KEY = 'omnix.clips.v1';
const LAYOUT_KEY = 'omnix.notepad.layout.v1';

/** Characters kept per note (kb-core's document limit is far above this). */
export const MAX_NOTE_CHARS = 200_000;
export const MAX_NOTES = 30;
export const MAX_CLIPS = 25;
export const MAX_CLIP_CHARS = 10_000;

function read<T>(key: string, fallback: T): T {
  try {
    const raw = globalThis.localStorage?.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function write(key: string, value: unknown): void {
  try {
    globalThis.localStorage?.setItem(key, JSON.stringify(value));
  } catch {
    // Storage full or blocked: in-memory state still works this session.
  }
}

let seq = 0;
function newId(prefix: string, now: number) {
  seq = (seq + 1) % 1_000_000;
  return `${prefix}${now.toString(36)}${seq}`;
}

// ---------------------------------------------------------------- notes

export function newNote(text = '', now = Date.now()): Note {
  return { id: newId('n', now), text, updated: now };
}

export interface NotesState {
  notes: Note[];
  active: string;
}

export function loadNotes(): NotesState {
  const saved = read<Partial<NotesState> | null>(NOTES_KEY, null);
  const notes = Array.isArray(saved?.notes)
    ? saved.notes.filter((n) => n && typeof n.id === 'string' && typeof n.text === 'string')
    : [];
  if (!notes.length) notes.push(newNote());
  const active = notes.some((n) => n.id === saved?.active) ? (saved!.active as string) : notes[0].id;
  return { notes, active };
}

export function saveNotes(state: NotesState): void {
  write(NOTES_KEY, state);
}

/** A note's title: its first heading or line, trimmed of Markdown marks. */
export function noteTitle(text: string, max = 40): string {
  const line = text.split('\n').find((l) => l.trim()) ?? '';
  const clean = line
    .replace(/^\s*(#{1,6}\s+|[-*+]\s+(\[[ xX]\]\s+)?|>\s*|\d+[.)]\s+)/, '')
    .replace(/[*_`~]/g, '')
    .trim();
  if (!clean) return 'Untitled';
  return clean.length > max ? `${clean.slice(0, max - 1)}…` : clean;
}

/** Append text to a note with a blank line between, capped at the limit. */
export function appendText(note: string, text: string, source?: string): string {
  const block = source ? `> ${source}\n\n${text.trim()}` : text.trim();
  const joined = note.trim() ? `${note.trimEnd()}\n\n${block}\n` : `${block}\n`;
  return joined.slice(0, MAX_NOTE_CHARS);
}

/**
 * GFM task items (`- [ ]`, `- [x]`) as glyphs. Checkbox inputs are stripped
 * by the sanitizer, so the preview shows ☐ / ☑ instead.
 */
export function taskGlyphs(md: string): string {
  return md.replace(/^(\s*[-*+]\s+)\[( |x|X)\](?=\s)/gm, (_m, lead: string, mark: string) =>
    `${lead}${mark === ' ' ? '☐' : '☑'}`
  );
}

/** Toggle the task checkbox on a given (0-based) line, if it has one. */
export function toggleTask(md: string, line: number): string {
  const lines = md.split('\n');
  const l = lines[line];
  if (l == null) return md;
  const m = /^(\s*[-*+]\s+)\[( |x|X)\]/.exec(l);
  if (!m) return md;
  lines[line] = `${m[1]}[${m[2] === ' ' ? 'x' : ' '}]${l.slice(m[0].length)}`;
  return lines.join('\n');
}

export interface Edit {
  text: string;
  start: number;
  end: number;
}

/**
 * Markdown toolbar edits. Wraps the selection (or inserts a placeholder)
 * and returns the new text plus the selection to restore.
 */
export function applyFormat(
  text: string,
  start: number,
  end: number,
  kind: 'bold' | 'italic' | 'code' | 'heading' | 'bullet' | 'task' | 'quote' | 'link'
): Edit {
  const sel = text.slice(start, end);
  const wrap = (l: string, r: string, placeholder: string): Edit => {
    const inner = sel || placeholder;
    const out = text.slice(0, start) + l + inner + r + text.slice(end);
    return { text: out, start: start + l.length, end: start + l.length + inner.length };
  };
  const prefixLines = (prefix: string): Edit => {
    const lineStart = text.lastIndexOf('\n', start - 1) + 1;
    const block = text.slice(lineStart, end);
    const out = block
      .split('\n')
      .map((l) => (l.startsWith(prefix) ? l.slice(prefix.length) : prefix + l))
      .join('\n');
    const next = text.slice(0, lineStart) + out + text.slice(end);
    return { text: next, start: lineStart, end: lineStart + out.length };
  };
  switch (kind) {
    case 'bold':
      return wrap('**', '**', 'bold');
    case 'italic':
      return wrap('*', '*', 'italic');
    case 'code':
      return sel.includes('\n') ? wrap('```\n', '\n```', '') : wrap('`', '`', 'code');
    case 'link':
      return wrap('[', '](https://)', 'text');
    case 'heading':
      return prefixLines('## ');
    case 'bullet':
      return prefixLines('- ');
    case 'task':
      return prefixLines('- [ ] ');
    case 'quote':
      return prefixLines('> ');
  }
}

/** Word and character counts for the status line. */
export function counts(text: string): { words: number; chars: number } {
  const words = text.trim() ? text.trim().split(/\s+/).length : 0;
  return { words, chars: text.length };
}

// ---------------------------------------------------------------- clips

type ClipListener = (clips: Clip[]) => void;
const listeners = new Set<ClipListener>();

export function loadClips(): Clip[] {
  const saved = read<Clip[]>(CLIPS_KEY, []);
  return Array.isArray(saved) ? saved.filter((c) => c && typeof c.text === 'string') : [];
}

export function saveClips(clips: Clip[]): void {
  write(CLIPS_KEY, clips);
  for (const l of listeners) l(clips);
}

/** Record a copy in the clipboard history (newest first, de-duplicated). */
export function recordClip(text: string, now = Date.now()): Clip[] {
  const clipped = text.trim().slice(0, MAX_CLIP_CHARS);
  if (!clipped) return loadClips();
  const rest = loadClips().filter((c) => c.text !== clipped);
  const clips = [{ id: newId('c', now), text: clipped, at: now }, ...rest].slice(0, MAX_CLIPS);
  saveClips(clips);
  return clips;
}

/** Be told when the clip history changes (returns an unsubscribe). */
export function onClips(fn: ClipListener): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

// ---------------------------------------------------------------- layout

export interface NotepadLayout {
  open: boolean;
  /** Box height, px. */
  height: number;
  mode: 'edit' | 'preview' | 'split';
}

export const DEFAULT_LAYOUT: NotepadLayout = { open: true, height: 260, mode: 'edit' };
export const MIN_HEIGHT = 120;

export function loadLayout(): NotepadLayout {
  const s = read<Partial<NotepadLayout>>(LAYOUT_KEY, {});
  return {
    open: typeof s.open === 'boolean' ? s.open : DEFAULT_LAYOUT.open,
    height: typeof s.height === 'number' && s.height >= MIN_HEIGHT ? s.height : DEFAULT_LAYOUT.height,
    mode: s.mode === 'preview' || s.mode === 'split' ? s.mode : 'edit'
  };
}

export function saveLayout(l: NotepadLayout): void {
  write(LAYOUT_KEY, l);
}

/** Clamp a dragged height between the minimum and the room available. */
export function clampHeight(h: number, available: number): number {
  return Math.round(Math.min(Math.max(h, MIN_HEIGHT), Math.max(MIN_HEIGHT, available)));
}
