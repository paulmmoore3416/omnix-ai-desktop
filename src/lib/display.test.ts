import { beforeEach, describe, expect, it } from 'vitest';
import {
  AVATAR_MAX,
  AVATAR_MIN,
  DEFAULT_DISPLAY,
  DOCK_MAX,
  DOCK_MIN,
  accentColor,
  accentVars,
  clampAvatar,
  clampDock,
  loadDisplay,
  saveDisplay
} from './display';

beforeEach(() => localStorage.clear());

describe('display preferences', () => {
  it('defaults and round-trips', () => {
    expect(loadDisplay()).toEqual(DEFAULT_DISPLAY);
    saveDisplay({ text: 'xl', accent: 'rose', avatar: 1.3, dockWidth: 420 });
    expect(loadDisplay()).toEqual({ text: 'xl', accent: 'rose', avatar: 1.3, dockWidth: 420 });
  });

  it('rejects junk and clamps ranges', () => {
    localStorage.setItem('omnix.display.v1', JSON.stringify({ text: 'huge', accent: 'plaid', avatar: 9, dockWidth: 5 }));
    expect(loadDisplay()).toEqual({ ...DEFAULT_DISPLAY, avatar: AVATAR_MAX, dockWidth: DOCK_MIN });
    localStorage.setItem('omnix.display.v1', 'not json');
    expect(loadDisplay()).toEqual(DEFAULT_DISPLAY);
    expect(clampAvatar(0.1)).toBe(AVATAR_MIN);
    expect(clampAvatar(Number.NaN)).toBe(1);
    expect(clampDock(5000)).toBe(DOCK_MAX);
    expect(clampDock(500, 800)).toBe(400);
  });

  it('resolves accents, including the avatar mood', () => {
    expect(accentColor('violet', '#ff0000')).toBe('#a78bfa');
    expect(accentColor('mood', '#ff0000')).toBe('#ff0000');
    expect(accentVars('#ff0000')).toBe('--accent: #ff0000; --accent-soft: rgba(255, 0, 0, 0.18); --accent-line: rgba(255, 0, 0, 0.5);');
  });
});
