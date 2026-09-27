import { describe, expect, it } from 'vitest';
import { preview, rank, score, type PaletteItem } from './palette';

const item = (label: string, group = 'Go to', keywords?: string): PaletteItem => ({
  id: label,
  group,
  icon: '•',
  label,
  keywords,
  run: () => {}
});

describe('palette ranking', () => {
  const items = [
    item('Settings'),
    item('System Control'),
    item('Live metrics', 'Workspace', 'cpu gpu memory vram'),
    item('Situation brief', 'Actions', 'status summary'),
    item('Text size L', 'Display', 'font bigger')
  ];

  it('keeps group order without a query', () => {
    expect(rank(items, '').map((i) => i.label)).toEqual([
      'Situation brief',
      'Settings',
      'System Control',
      'Live metrics',
      'Text size L'
    ]);
  });

  it('prefers label prefixes, then words, then keywords', () => {
    expect(rank(items, 'sys')[0].label).toBe('System Control');
    expect(rank(items, 'gpu').map((i) => i.label)).toEqual(['Live metrics']);
    expect(rank(items, 'font').map((i) => i.label)).toEqual(['Text size L']);
  });

  it('requires every word and allows loose subsequences', () => {
    expect(rank(items, 'system brief')).toEqual([]);
    expect(rank(items, 'sttgs').map((i) => i.label)).toEqual(['Settings']);
    expect(score(item('Settings'), 'zzz')).toBe(0);
  });

  it('previews text on one line', () => {
    expect(preview('a\n\n b   c')).toBe('a b c');
    expect(preview('x'.repeat(200), 10)).toBe('xxxxxxxxx…');
  });
});
