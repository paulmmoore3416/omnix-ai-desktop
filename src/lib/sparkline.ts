/**
 * Geometry for sparklines: smooth curves that never overshoot the data, and
 * time-based placement so live charts glide instead of jumping per sample.
 */

export interface XY {
  x: number;
  y: number;
}

/** A timestamped reading. `null` = not measured (drawn as a gap). */
export interface TimedValue {
  t: number;
  v: number | null;
}

const f = (n: number) => (Math.round(n * 10) / 10).toString();

/**
 * SVG path through the points as a monotone cubic (Fritsch–Carlson). Unlike
 * a Catmull-Rom spline it never overshoots, so a 0–100 % line can't bulge
 * past the chart edges or dip below zero between two readings.
 */
export function smoothPath(pts: XY[]): string {
  const n = pts.length;
  if (n === 0) return '';
  if (n === 1) return `M${f(pts[0].x)},${f(pts[0].y)}`;
  if (n === 2) return `M${f(pts[0].x)},${f(pts[0].y)}L${f(pts[1].x)},${f(pts[1].y)}`;

  const dx: number[] = [];
  const slope: number[] = [];
  for (let i = 0; i < n - 1; i++) {
    const h = pts[i + 1].x - pts[i].x;
    dx.push(h);
    slope.push(h === 0 ? 0 : (pts[i + 1].y - pts[i].y) / h);
  }
  // Tangents: zero at local extrema, harmonic-mean-like elsewhere.
  const m: number[] = new Array(n);
  m[0] = slope[0];
  m[n - 1] = slope[n - 2];
  for (let i = 1; i < n - 1; i++) {
    m[i] = slope[i - 1] * slope[i] <= 0 ? 0 : (slope[i - 1] + slope[i]) / 2;
  }
  for (let i = 0; i < n - 1; i++) {
    if (slope[i] === 0) {
      m[i] = 0;
      m[i + 1] = 0;
      continue;
    }
    const a = m[i] / slope[i];
    const b = m[i + 1] / slope[i];
    const s = a * a + b * b;
    if (s > 9) {
      const t = 3 / Math.sqrt(s);
      m[i] = t * a * slope[i];
      m[i + 1] = t * b * slope[i];
    }
  }

  let d = `M${f(pts[0].x)},${f(pts[0].y)}`;
  for (let i = 0; i < n - 1; i++) {
    const h = dx[i] / 3;
    const c1x = pts[i].x + h;
    const c1y = pts[i].y + m[i] * h;
    const c2x = pts[i + 1].x - h;
    const c2y = pts[i + 1].y - m[i + 1] * h;
    d += `C${f(c1x)},${f(c1y)} ${f(c2x)},${f(c2y)} ${f(pts[i + 1].x)},${f(pts[i + 1].y)}`;
  }
  return d;
}

/** Close a line path down to the baseline so it can be filled. */
export function areaPath(line: string, pts: XY[], baseline: number): string {
  if (pts.length < 2) return '';
  return `${line}L${f(pts[pts.length - 1].x)},${f(baseline)}L${f(pts[0].x)},${f(baseline)}Z`;
}

/** Map a value onto the chart's height (0 at the bottom, `max` at the top, 1-unit inset). */
export function yFor(v: number, max: number, height: number): number {
  const top = max > 0 ? max : 1;
  return height - (Math.min(Math.max(v, 0), top) / top) * (height - 2) - 1;
}

/**
 * Split readings into drawable runs, placed by time: `x = width` at
 * `origin`, `x = 0` one `window` earlier. Nulls and gaps longer than
 * `maxGap` ms break the line. Points just outside the window are kept so the
 * curve enters the edge smoothly.
 */
export function timedRuns(
  values: TimedValue[],
  opts: { origin: number; window: number; width: number; height: number; max: number; maxGap: number }
): XY[][] {
  const { origin, window, width, height, max, maxGap } = opts;
  const runs: XY[][] = [];
  let cur: XY[] = [];
  let lastT = -Infinity;
  const from = origin - window * 1.1;
  for (const p of values) {
    if (p.t < from) continue;
    if (p.v == null || !Number.isFinite(p.v) || p.t - lastT > maxGap) {
      if (cur.length) runs.push(cur);
      cur = [];
    }
    if (p.v != null && Number.isFinite(p.v)) {
      cur.push({ x: width - ((origin - p.t) / window) * width, y: yFor(p.v, max, height) });
      lastT = p.t;
    } else {
      lastT = -Infinity;
    }
  }
  if (cur.length) runs.push(cur);
  return runs.filter((r) => r.length > 1);
}

/**
 * Merge seeded history with live readings: history is kept only up to the
 * first live reading, so the two never interleave. Output is sorted and
 * trimmed to `keepMs` before `now`.
 */
export function mergeSeries<T extends { t: number }>(history: T[], live: T[], now: number, keepMs: number): T[] {
  const firstLive = live.length ? live[0].t : Infinity;
  const cutoff = now - keepMs;
  return [...history.filter((h) => h.t < firstLive), ...live].filter((p) => p.t >= cutoff);
}
