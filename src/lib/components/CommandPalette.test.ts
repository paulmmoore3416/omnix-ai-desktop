import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import CommandPalette from './CommandPalette.svelte';
import type { MemoryHit, PaletteItem } from '$lib/palette';

afterEach(cleanup);

function setup(searchMemory: (q: string) => Promise<MemoryHit[]> = vi.fn(async () => [])) {
  const ran: string[] = [];
  const items: PaletteItem[] = ['Settings', 'Knowledge', 'System Control'].map((l) => ({
    id: l,
    group: 'Go to',
    icon: '•',
    label: l,
    run: () => ran.push(l)
  }));
  const p = { open: true, items, searchMemory, onMemory: vi.fn(), onAsk: vi.fn(), onTask: vi.fn() };
  render(CommandPalette, p);
  return { ran, p, box: screen.getByLabelText('Search commands and memory') };
}

describe('CommandPalette', () => {
  it('filters and runs the highlighted item with Enter', async () => {
    const { ran, box } = setup();
    await fireEvent.input(box, { target: { value: 'know' } });
    await fireEvent.keyDown(box, { key: 'Enter' });
    expect(ran).toEqual(['Knowledge']);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('moves with the arrow keys and offers ask / side task for free text', async () => {
    const { p, box } = setup();
    await fireEvent.input(box, { target: { value: 'what is my nas ip' } });
    expect(screen.getByText('Ask OMNIX: what is my nas ip')).toBeInTheDocument();
    await fireEvent.keyDown(box, { key: 'ArrowDown' });
    await fireEvent.keyDown(box, { key: 'Enter' });
    expect(p.onTask).toHaveBeenCalledWith('what is my nas ip');
  });

  it('searches memory after a pause and hands hits on as text', async () => {
    const search = vi.fn(async (_q: string): Promise<MemoryHit[]> => [
      { id: 'm1', content: 'NAS is at <b>10.0.0.5</b>', score: 0.8, kind: 'memory', collection: 'homelab' },
      { id: 'm2', content: 'weak match', score: 0.1, kind: 'memory' }
    ]);
    const { p, box } = setup(search);
    await fireEvent.input(box, { target: { value: 'nas address' } });
    const row = await screen.findByText('NAS is at <b>10.0.0.5</b>', {}, { timeout: 1500 });
    expect(search).toHaveBeenCalledWith('nas address');
    expect(document.querySelector('.palette b')).toBeNull(); // plain text, never HTML
    expect(screen.queryByText('weak match')).toBeNull(); // below the relevance floor
    expect(screen.getByText(/80% · memory · homelab/)).toBeInTheDocument();
    await fireEvent.click(row, { shiftKey: true });
    expect(p.onMemory).toHaveBeenCalledWith(expect.objectContaining({ id: 'm1' }), 'note');
  });

  it('closes on Escape and skips memory for slash commands', async () => {
    const search = vi.fn(async (_q: string): Promise<MemoryHit[]> => []);
    const { box } = setup(search);
    await fireEvent.input(box, { target: { value: '/execute' } });
    await new Promise((r) => setTimeout(r, 350));
    expect(search).not.toHaveBeenCalled();
    await fireEvent.keyDown(box, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});

describe('CommandPalette grouping', () => {
  it('clusters interleaved matches under one header per group', async () => {
    const mk = (label: string, group: string): PaletteItem => ({ id: label, group, icon: '•', label, run: () => {} });
    render(CommandPalette, {
      open: true,
      items: [mk('Shift huddle agenda', 'Prompts'), mk('Shift notes', 'Notes'), mk('Shift report', 'Prompts')],
      onMemory: vi.fn(),
      onAsk: vi.fn(),
      onTask: vi.fn()
    });
    await fireEvent.input(screen.getByLabelText('Search commands and memory'), { target: { value: 'shift' } });
    const headers = [...document.querySelectorAll('.group-label')].map((h) => h.textContent);
    expect(headers).toEqual(['Notes', 'Prompts', 'Ask']);
    const rows = screen.getAllByRole('option').map((o) => o.textContent?.replace('•', '').trim());
    expect(rows.slice(0, 3)).toEqual(['Shift notes', 'Shift report', 'Shift huddle agenda']);
  });
});
