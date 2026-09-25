import { describe, expect, it } from 'vitest';
import { CONDITIONS, MOODS, SIGNALS, conditionForError, mix, rgba } from './avatar';

const HEX = /^#[0-9a-f]{6}$/;

describe('avatar colour language', () => {
  it('uses valid, distinct colours for every mood and condition', () => {
    const colours = [...Object.values(MOODS), ...Object.values(CONDITIONS)].map((s) => s.color);
    colours.forEach((c) => expect(c).toMatch(HEX));
    // A condition must never be mistaken for a mood, so no colour is reused.
    expect(new Set(colours).size).toBe(colours.length);
  });

  it('documents every entry', () => {
    for (const s of [...Object.values(MOODS), ...Object.values(CONDITIONS), ...Object.values(SIGNALS)]) {
      expect(s.label).not.toBe('');
      expect(s.meaning).not.toBe('');
      expect(s.effect).not.toBe('');
    }
  });

  it('maps backend error kinds to conditions', () => {
    expect(conditionForError('policy_denied')).toBe('security');
    expect(conditionForError('not_approved')).toBe('security');
    expect(conditionForError('local_only')).toBe('security');
    expect(conditionForError('http')).toBe('network');
    expect(conditionForError('unavailable')).toBe('network');
    expect(conditionForError('execution')).toBeNull();
    expect(conditionForError(undefined)).toBeNull();
  });

  it('converts and mixes hex colours', () => {
    expect(rgba('#29d8ff', 0.5)).toBe('rgba(41, 216, 255, 0.5)');
    expect(mix('#000000', '#ffffff', 0)).toBe('#000000');
    expect(mix('#000000', '#ffffff', 1)).toBe('#ffffff');
    expect(mix('#ff0000', '#0000ff', 0.5)).toBe('#800080');
  });
});
