import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (cmd: string, args?: unknown) => invoke(cmd, args) }));

import Notepad from './Notepad.svelte';
import { loadNotes, recordClip } from '$lib/notepad';

const props = () => ({ notify: vi.fn(), onInsert: vi.fn(), onTask: vi.fn(), onPin: vi.fn() });

beforeEach(() => {
  localStorage.clear();
  invoke.mockReset();
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe('Notepad', () => {
  it('saves what you type and renders it as sanitized Markdown', async () => {
    render(Notepad, props());
    const area = screen.getByLabelText('Note text');
    await fireEvent.input(area, { target: { value: '# Rounds\n- [ ] call **lab**\n<img src=x onerror=alert(1)>' } });
    vi.advanceTimersByTime(500);
    expect(loadNotes().notes[0].text).toContain('# Rounds');

    await fireEvent.click(screen.getByTitle('Preview Markdown'));
    expect(screen.getByRole('heading', { name: 'Rounds' })).toBeInTheDocument();
    expect(screen.getByText('lab').tagName).toBe('STRONG');
    expect(document.body.textContent).toContain('☐');
    expect(document.querySelector('.preview img')).toBeNull();
    // The picker shows the note's title.
    expect(screen.getByRole('option', { name: 'Rounds' })).toBeInTheDocument();
  });

  it('formats the selection and ticks tasks with Ctrl+Enter', async () => {
    render(Notepad, props());
    const area = screen.getByLabelText('Note text') as HTMLTextAreaElement;
    await fireEvent.input(area, { target: { value: 'buy milk' } });
    area.setSelectionRange(0, 8);
    await fireEvent.click(screen.getByTitle('Task (Ctrl+Enter ticks it)'));
    expect(area.value).toBe('- [ ] buy milk');
    area.setSelectionRange(3, 3);
    await fireEvent.keyDown(area, { key: 'Enter', ctrlKey: true });
    expect(area.value).toBe('- [x] buy milk');
  });

  it('resizes with the keyboard and remembers the height', async () => {
    render(Notepad, props());
    const grip = screen.getByLabelText(/Resize the notepad/);
    const section = screen.getByLabelText('Notepad');
    expect(section.getAttribute('style')).toContain('height: 260px');
    await fireEvent.keyDown(grip, { key: 'ArrowUp' });
    expect(section.getAttribute('style')).toContain('height: 284px');
    expect(JSON.parse(localStorage.getItem('omnix.notepad.layout.v1')!).height).toBe(284);
  });

  it('collapses to a header', async () => {
    render(Notepad, props());
    await fireEvent.click(screen.getByTitle('Collapse'));
    expect(screen.queryByLabelText('Note text')).toBeNull();
    await fireEvent.click(screen.getByTitle('Expand'));
    expect(screen.getByLabelText('Note text')).toBeInTheDocument();
  });

  it('lists clips and adds one to the note', async () => {
    recordClip('copied reply text');
    render(Notepad, props());
    await fireEvent.click(screen.getByTitle(/Clips/));
    await fireEvent.click(screen.getByText('copied reply text'));
    // Adding a clip returns to the editor.
    expect((screen.getByLabelText('Note text') as HTMLTextAreaElement).value).toContain('copied reply text');
  });

  it('runs Tidy as a side task over the note', async () => {
    const p = props();
    render(Notepad, p);
    await fireEvent.input(screen.getByLabelText('Note text'), { target: { value: 'NAS is 10.0.0.5' } });
    await fireEvent.click(screen.getByTitle(/Tidy/));
    expect(p.onTask).toHaveBeenCalledWith(expect.stringContaining('Tidy'), expect.objectContaining({ context: 'NAS is 10.0.0.5' }));
  });

  it('asks before deleting a note with text', async () => {
    render(Notepad, props());
    await fireEvent.input(screen.getByLabelText('Note text'), { target: { value: 'keep me?' } });
    await fireEvent.click(screen.getByTitle('Delete this note'));
    expect((screen.getByLabelText('Note text') as HTMLTextAreaElement).value).toBe('keep me?');
    await fireEvent.click(screen.getByTitle('Click again to delete'));
    expect((screen.getByLabelText('Note text') as HTMLTextAreaElement).value).toBe('');
  });
});

describe('Notepad memory sync', () => {
  it('keeps a note in kb-core, re-syncs after edits and removes it on toggle off', async () => {
    invoke.mockResolvedValue({ id: 'd1', chunks: 1 });
    const p = props();
    render(Notepad, p);
    const area = screen.getByLabelText('Note text');
    await fireEvent.input(area, { target: { value: '# Backups\nZFS snapshots nightly' } });
    vi.advanceTimersByTime(500);
    const id = loadNotes().notes[0].id;
    await fireEvent.click(screen.getByTitle(/^Keep in memory/));
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('sync_note', { id, content: '# Backups\nZFS snapshots nightly' }));
    await vi.waitFor(() => expect(p.notify).toHaveBeenCalledWith(expect.stringContaining('Kept in memory'), 'success'));

    invoke.mockClear();
    await fireEvent.input(area, { target: { value: '# Backups\nZFS snapshots hourly' } });
    vi.advanceTimersByTime(3000);
    expect(invoke).not.toHaveBeenCalledWith('sync_note', expect.anything());
    vi.advanceTimersByTime(1500);
    expect(invoke).toHaveBeenCalledWith('sync_note', { id, content: '# Backups\nZFS snapshots hourly' });
    vi.advanceTimersByTime(500);
    expect(loadNotes().notes[0].synced).toBe(true);

    await fireEvent.click(screen.getByTitle(/^Kept in memory/));
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('unsync_note', { id }));
  });

  it('reports a failed sync and stays off', async () => {
    invoke.mockRejectedValue({ kind: 'Memory', message: 'kb-core is not reachable' });
    const p = props();
    render(Notepad, p);
    await fireEvent.input(screen.getByLabelText('Note text'), { target: { value: 'x' } });
    await fireEvent.click(screen.getByTitle(/^Keep in memory/));
    await vi.waitFor(() => expect(p.notify).toHaveBeenCalledWith(expect.stringContaining('Keep in memory'), 'error'));
    expect(screen.getByTitle(/^Keep in memory/)).toHaveAttribute('aria-pressed', 'false');
  });
});
