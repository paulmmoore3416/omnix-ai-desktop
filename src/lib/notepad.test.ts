import { beforeEach, describe, expect, it } from 'vitest';
import {
  MAX_CLIPS,
  MIN_HEIGHT,
  appendText,
  applyFormat,
  clampHeight,
  counts,
  loadClips,
  loadLayout,
  loadNotes,
  newNote,
  noteTitle,
  onClips,
  recordClip,
  saveLayout,
  saveNotes,
  taskGlyphs,
  toggleTask
} from './notepad';

beforeEach(() => localStorage.clear());

describe('notes', () => {
  it('starts with one empty note and round-trips', () => {
    const s = loadNotes();
    expect(s.notes).toHaveLength(1);
    expect(s.active).toBe(s.notes[0].id);
    const n = newNote('# Plan');
    saveNotes({ notes: [n, ...s.notes], active: n.id });
    const again = loadNotes();
    expect(again.notes[0].text).toBe('# Plan');
    expect(again.active).toBe(n.id);
  });

  it('survives corrupt storage', () => {
    localStorage.setItem('omnix.notepad.v1', '{nope');
    expect(loadNotes().notes).toHaveLength(1);
    localStorage.setItem('omnix.notepad.v1', JSON.stringify({ notes: [{ bad: 1 }], active: 'x' }));
    expect(loadNotes().notes[0].text).toBe('');
  });

  it('titles from the first meaningful line', () => {
    expect(noteTitle('\n\n## **Shift** notes\nmore')).toBe('Shift notes');
    expect(noteTitle('- [ ] call pharmacy')).toBe('call pharmacy');
    expect(noteTitle('   ')).toBe('Untitled');
    expect(noteTitle('x'.repeat(80), 10)).toBe('xxxxxxxxx…');
  });

  it('appends with spacing and an optional source quote', () => {
    expect(appendText('', 'hello')).toBe('hello\n');
    expect(appendText('a\n\n', 'b', 'Reply')).toBe('a\n\n> Reply\n\nb\n');
  });

  it('counts words and characters', () => {
    expect(counts('')).toEqual({ words: 0, chars: 0 });
    expect(counts(' two  words\n')).toEqual({ words: 2, chars: 12 });
  });
});

describe('markdown helpers', () => {
  it('shows task items as glyphs', () => {
    expect(taskGlyphs('- [ ] open\n  * [x] done\n- [link](x)')).toBe('- ☐ open\n  * ☑ done\n- [link](x)');
  });

  it('toggles the task on a line', () => {
    const md = 'intro\n- [ ] one\n- [x] two';
    expect(toggleTask(md, 1)).toBe('intro\n- [x] one\n- [x] two');
    expect(toggleTask(md, 2)).toBe('intro\n- [ ] one\n- [ ] two');
    expect(toggleTask(md, 0)).toBe(md);
    expect(toggleTask(md, 9)).toBe(md);
  });

  it('wraps the selection or inserts a placeholder', () => {
    expect(applyFormat('say hi', 4, 6, 'bold')).toEqual({ text: 'say **hi**', start: 6, end: 8 });
    expect(applyFormat('', 0, 0, 'italic')).toEqual({ text: '*italic*', start: 1, end: 7 });
    expect(applyFormat('a\nb', 0, 3, 'code').text).toBe('```\na\nb\n```');
  });

  it('prefixes and un-prefixes whole lines', () => {
    const on = applyFormat('one\ntwo', 1, 6, 'task');
    expect(on.text).toBe('- [ ] one\n- [ ] two');
    expect(applyFormat(on.text, 0, on.text.length, 'task').text).toBe('one\ntwo');
    expect(applyFormat('x\ntitle', 3, 3, 'heading').text).toBe('x\n## title');
  });
});

describe('clips', () => {
  it('records newest first, de-duplicates, caps and notifies', () => {
    const seen: number[] = [];
    const off = onClips((c) => seen.push(c.length));
    recordClip('a', 1);
    recordClip('b', 2);
    recordClip('a', 3);
    expect(loadClips().map((c) => c.text)).toEqual(['a', 'b']);
    expect(seen).toEqual([1, 2, 2]);
    off();
    for (let i = 0; i < MAX_CLIPS + 5; i++) recordClip(`c${i}`, i);
    expect(loadClips()).toHaveLength(MAX_CLIPS);
    expect(seen).toHaveLength(3);
  });

  it('ignores blank text', () => {
    recordClip('   ');
    expect(loadClips()).toEqual([]);
  });
});

describe('layout', () => {
  it('defaults, validates and clamps', () => {
    expect(loadLayout()).toEqual({ open: true, height: 260, mode: 'edit' });
    saveLayout({ open: false, height: 50, mode: 'split' });
    expect(loadLayout()).toEqual({ open: false, height: 260, mode: 'split' });
    expect(clampHeight(10, 500)).toBe(MIN_HEIGHT);
    expect(clampHeight(900, 500)).toBe(500);
    expect(clampHeight(300, 50)).toBe(MIN_HEIGHT);
  });
});
