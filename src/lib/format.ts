/** Shared number/time formatting for the dashboards. */

export function bytes(n: number | null | undefined, digits = 1): string {
  if (n == null || !Number.isFinite(n)) return '—';
  if (n === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.min(units.length - 1, Math.floor(Math.log(Math.abs(n)) / Math.log(1024)));
  return `${(n / Math.pow(1024, i)).toFixed(i === 0 ? 0 : digits)} ${units[i]}`;
}

export function rate(n: number | null | undefined): string {
  return n == null ? '—' : `${bytes(n)}/s`;
}

export function pct(n: number | null | undefined, digits = 0): string {
  return n == null || !Number.isFinite(n) ? '—' : `${n.toFixed(digits)}%`;
}

export function duration(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return d > 0 ? `${d}d ${h}h` : h > 0 ? `${h}h ${m}m` : `${m}m`;
}

export function ms(n: number | null | undefined): string {
  if (n == null) return '—';
  return n >= 1000 ? `${(n / 1000).toFixed(1)} s` : `${Math.round(n)} ms`;
}

export function ago(ts: string | number | null | undefined): string {
  if (ts == null) return 'never';
  const s = (Date.now() - new Date(ts).getTime()) / 1000;
  if (Number.isNaN(s)) return '';
  if (s < 0) {
    const f = -s;
    if (f < 60) return 'in <1m';
    if (f < 3600) return `in ${Math.round(f / 60)}m`;
    if (f < 86400) return `in ${Math.round(f / 3600)}h`;
    return `in ${Math.round(f / 86400)}d`;
  }
  if (s < 60) return 'just now';
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86400)}d ago`;
}

/** Tailwind text colour for a 0–100 load value. */
export function loadColor(v: number | null | undefined): string {
  if (v == null) return 'text-gray-400';
  if (v >= 90) return 'text-red-400';
  if (v >= 70) return 'text-yellow-400';
  return 'text-green-400';
}
