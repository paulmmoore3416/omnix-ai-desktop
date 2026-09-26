<script lang="ts">
  /**
   * Minimal SVG sparkline. `values` are plotted on a fixed 0..max scale
   * (so 50% always sits at mid-height); `null` values are gaps (not measured).
   * Lines are smooth monotone curves (no overshoot past the data).
   * Colour comes from `currentColor` — set it with a text-* class.
   */
  import { areaPath, smoothPath, yFor, type XY } from '$lib/sparkline';

  let {
    values,
    max = 100,
    height = 40,
    label = ''
  }: { values: (number | null)[]; max?: number; height?: number; label?: string } = $props();

  const W = 200;

  let top = $derived(max > 0 ? max : Math.max(1, ...values.map((v) => v ?? 0)));
  let runs = $derived.by(() => {
    const n = values.length;
    if (n < 2) return [] as { line: string; area: string }[];
    const out: { line: string; area: string }[] = [];
    let cur: XY[] = [];
    const flush = () => {
      if (cur.length > 1) {
        const line = smoothPath(cur);
        out.push({ line, area: areaPath(line, cur, height) });
      }
      cur = [];
    };
    values.forEach((v, i) => {
      if (v == null || !Number.isFinite(v)) return flush();
      cur.push({ x: (i / (n - 1)) * W, y: yFor(v, top, height) });
    });
    flush();
    return out;
  });
</script>

<svg viewBox="0 0 {W} {height}" preserveAspectRatio="none" class="w-full block" style="height: {height}px" role="img" aria-label={label}>
  <line x1="0" x2={W} y1={height / 2} y2={height / 2} stroke="currentColor" stroke-opacity="0.08" stroke-dasharray="2 3" />
  {#each runs as r}
    <path d={r.area} fill="currentColor" fill-opacity="0.12" />
    <path d={r.line} fill="none" stroke="currentColor" stroke-width="1.5" vector-effect="non-scaling-stroke" stroke-linejoin="round" />
  {/each}
</svg>
