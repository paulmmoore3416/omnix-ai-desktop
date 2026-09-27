/**
 * Command palette (Ctrl+K): one box to jump to a view, run a workspace
 * action, use a saved prompt, open a note or search memory.
 *
 * Items are plain descriptions plus a callback; ranking is a small,
 * dependency-free fuzzy match so it stays instant on every keystroke.
 */

export interface PaletteItem {
  id: string;
  group: string;
  icon: string;
  label: string;
  /** Secondary text (shortcut, preview). Plain text only. */
  hint?: string;
  /** Extra words that should match (synonyms). */
  keywords?: string;
  run: () => void;
  /** Optional Shift+Enter action (e.g. add to the notepad). */
  alt?: { label: string; run: () => void };
}

/** A `semantic_search` result (untrusted text). */
export interface MemoryHit {
  id: string;
  content: string;
  score: number;
  kind?: string;
  source?: string;
  collection?: string | null;
}

/** Order groups are shown in when there is no query. */
export const GROUP_ORDER = ['Actions', 'Go to', 'Workspace', 'Notes', 'Prompts', 'Commands', 'Display', 'Memory', 'Ask'];

function subsequence(needle: string, hay: string): boolean {
  let i = 0;
  for (const ch of hay) if (ch === needle[i] && ++i === needle.length) return true;
  return needle.length === 0;
}

/**
 * Score an item against a query (higher is better; 0 = no match). Every
 * query word must match: as a word prefix (best), a substring, or loosely
 * as a subsequence of the label.
 */
export function score(item: Pick<PaletteItem, 'label' | 'keywords' | 'group'>, query: string): number {
  const q = query.trim().toLowerCase();
  if (!q) return 1;
  const label = item.label.toLowerCase();
  const hay = `${label} ${(item.keywords ?? '').toLowerCase()} ${item.group.toLowerCase()}`;
  const words = hay.split(/[^\p{L}\p{N}/]+/u).filter(Boolean);
  let total = 0;
  for (const w of q.split(/\s+/)) {
    if (words.some((x) => x.startsWith(w))) total += label.startsWith(w) ? 12 : 8;
    else if (hay.includes(w)) total += 4;
    else if (w.length > 1 && subsequence(w, label)) total += 1;
    else return 0;
  }
  // Prefer shorter labels for the same match (closer to what was typed).
  return total + Math.max(0, 1 - label.length / 100);
}

/** Matching items, best first; ties keep group order then insertion order. */
export function rank(items: PaletteItem[], query: string, limit = 40): PaletteItem[] {
  const groupIdx = (g: string) => {
    const i = GROUP_ORDER.indexOf(g);
    return i < 0 ? GROUP_ORDER.length : i;
  };
  return items
    .map((item, i) => ({ item, i, s: score(item, query) }))
    .filter((x) => x.s > 0)
    .sort((a, b) => (query.trim() ? b.s - a.s : 0) || groupIdx(a.item.group) - groupIdx(b.item.group) || a.i - b.i)
    .slice(0, limit)
    .map((x) => x.item);
}

/** One line of memory text for a result row. */
export function preview(text: string, max = 140): string {
  const flat = text.replace(/\s+/g, ' ').trim();
  return flat.length > max ? `${flat.slice(0, max - 1)}…` : flat;
}
