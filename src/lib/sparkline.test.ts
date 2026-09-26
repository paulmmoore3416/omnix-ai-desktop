import { describe, expect, it } from 'vitest';
import { mergeSeries, smoothPath, timedRuns, yFor } from './sparkline';

/** Every number in a path, as [x, y] pairs. */
function coords(d: string): number[][] {
  const nums = (d.match(/-?\d+(\.\d+)?/g) ?? []).map(Number);
  const out: number[][] = [];
  for (let i = 0; i < nums.length; i += 2) out.push([nums[i], nums[i + 1]]);
  return out;
}

describe('smoothPath', () => {
  it('handles tiny inputs', () => {
    expect(smoothPath([])).toBe('');
    expect(smoothPath([{ x: 0, y: 5 }])).toBe('M0,5');
    expect(smoothPath([{ x: 0, y: 5 }, { x: 10, y: 7 }])).toBe('M0,5L10,7');
  });

  it('never overshoots the data range (monotone)', () => {
    const pts = [0, 40, 40, 5, 39, 0, 40].map((y, i) => ({ x: i * 10, y }));
    const ys = coords(smoothPath(pts)).map(([, y]) => y);
    expect(Math.min(...ys)).toBeGreaterThanOrEqual(0);
    expect(Math.max(...ys)).toBeLessThanOrEqual(40);
  });

  it('keeps flat runs flat', () => {
    const d = smoothPath([{ x: 0, y: 10 }, { x: 10, y: 10 }, { x: 20, y: 10 }]);
    expect(coords(d).every(([, y]) => y === 10)).toBe(true);
  });
});

describe('yFor', () => {
  it('clamps to the chart with a 1-unit inset', () => {
    expect(yFor(0, 100, 30)).toBe(29);
    expect(yFor(100, 100, 30)).toBe(1);
    expect(yFor(250, 100, 30)).toBe(1);
    expect(yFor(-5, 100, 30)).toBe(29);
  });
});

describe('timedRuns', () => {
  const opts = { origin: 100_000, window: 100_000, width: 200, height: 30, max: 100, maxGap: 5_000 };

  it('places readings by time', () => {
    const [run] = timedRuns([{ t: 0, v: 0 }, { t: 50_000, v: 50 }, { t: 100_000, v: 100 }].map((p) => p), {
      ...opts,
      maxGap: 60_000
    });
    expect(run.map((p) => p.x)).toEqual([0, 100, 200]);
  });

  it('breaks on nulls and long gaps and drops single points', () => {
    const runs = timedRuns(
      [
        { t: 90_000, v: 1 },
        { t: 92_000, v: 2 },
        { t: 94_000, v: null },
        { t: 96_000, v: 3 },
        { t: 97_000, v: 4 },
        { t: 99_900, v: 5 }
      ],
      { ...opts, maxGap: 2_500 }
    );
    expect(runs.map((r) => r.length)).toEqual([2, 2]);
  });

  it('skips readings well before the window', () => {
    const runs = timedRuns([{ t: -50_000, v: 1 }, { t: 60_000, v: 2 }, { t: 61_000, v: 3 }], opts);
    expect(runs[0]).toHaveLength(2);
  });
});

describe('mergeSeries', () => {
  it('keeps history only before the first live reading and trims old points', () => {
    const history = [{ t: 10 }, { t: 20 }, { t: 30 }];
    const live = [{ t: 25 }, { t: 35 }];
    expect(mergeSeries(history, live, 40, 25).map((p) => p.t)).toEqual([20, 25, 35]);
    expect(mergeSeries(history, [], 40, 100).map((p) => p.t)).toEqual([10, 20, 30]);
  });
});
