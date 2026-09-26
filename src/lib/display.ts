/**
 * Display preferences: text size (workspace and chat), the workspace accent
 * colour, the avatar's size on Home and the workspace width. Per-user
 * conveniences in `localStorage`, guarded like every other stored setting.
 *
 * Avatar mood colours stay in `avatar.ts`; an accent of `mood` just follows
 * whatever colour the avatar currently shows.
 */
import { rgba } from './avatar';

export type TextSize = 'sm' | 'md' | 'lg' | 'xl';

/** Base font size in px for the workspace and for chat messages. */
export const TEXT_SIZES: Record<TextSize, { label: string; dock: number; chat: number }> = {
  sm: { label: 'S', dock: 13, chat: 14 },
  md: { label: 'M', dock: 14, chat: 16 },
  lg: { label: 'L', dock: 15.5, chat: 17.5 },
  xl: { label: 'XL', dock: 17, chat: 19 }
};

export type AccentId = 'cyan' | 'sky' | 'violet' | 'emerald' | 'amber' | 'rose' | 'mood';

/** Accent swatches. Each is legible on the dark glass (≥ 7:1 on #151a28). */
export const ACCENTS: { id: AccentId; label: string; color: string }[] = [
  { id: 'cyan', label: 'Cyan', color: '#22d3ee' },
  { id: 'sky', label: 'Sky', color: '#60a5fa' },
  { id: 'violet', label: 'Violet', color: '#a78bfa' },
  { id: 'emerald', label: 'Emerald', color: '#34d399' },
  { id: 'amber', label: 'Amber', color: '#fbbf24' },
  { id: 'rose', label: 'Rose', color: '#fb7185' },
  { id: 'mood', label: 'Follow the avatar', color: '' }
];

export interface DisplayPrefs {
  text: TextSize;
  accent: AccentId;
  /** Avatar size multiplier on Home. */
  avatar: number;
  /** Workspace width, px. */
  dockWidth: number;
}

export const AVATAR_MIN = 0.5;
export const AVATAR_MAX = 1.6;
export const DOCK_MIN = 300;
export const DOCK_MAX = 680;
export const DEFAULT_DISPLAY: DisplayPrefs = { text: 'md', accent: 'cyan', avatar: 1, dockWidth: 368 };

const KEY = 'omnix.display.v1';

export const clampAvatar = (v: number) =>
  Math.round(Math.min(AVATAR_MAX, Math.max(AVATAR_MIN, Number.isFinite(v) ? v : 1)) * 100) / 100;

export const clampDock = (v: number, viewport = Infinity) =>
  Math.round(Math.min(DOCK_MAX, viewport * 0.5, Math.max(DOCK_MIN, Number.isFinite(v) ? v : DEFAULT_DISPLAY.dockWidth)));

export function loadDisplay(): DisplayPrefs {
  let s: Partial<DisplayPrefs> = {};
  try {
    s = JSON.parse(globalThis.localStorage?.getItem(KEY) ?? '{}') ?? {};
  } catch {
    /* unreadable: defaults */
  }
  return {
    text: s.text && s.text in TEXT_SIZES ? s.text : DEFAULT_DISPLAY.text,
    accent: ACCENTS.some((a) => a.id === s.accent) ? (s.accent as AccentId) : DEFAULT_DISPLAY.accent,
    avatar: typeof s.avatar === 'number' ? clampAvatar(s.avatar) : DEFAULT_DISPLAY.avatar,
    dockWidth: typeof s.dockWidth === 'number' ? clampDock(s.dockWidth) : DEFAULT_DISPLAY.dockWidth
  };
}

export function saveDisplay(p: DisplayPrefs): void {
  try {
    globalThis.localStorage?.setItem(KEY, JSON.stringify(p));
  } catch {
    /* storage unavailable */
  }
}

/** The accent's hex colour (`mood` resolves to the avatar's current colour). */
export function accentColor(id: AccentId, moodColor: string): string {
  return id === 'mood' ? moodColor : (ACCENTS.find((a) => a.id === id)?.color ?? ACCENTS[0].color);
}

/** CSS custom properties for an accent: solid, soft fill and outline. */
export function accentVars(hex: string): string {
  return `--accent: ${hex}; --accent-soft: ${rgba(hex, 0.18)}; --accent-line: ${rgba(hex, 0.5)};`;
}
