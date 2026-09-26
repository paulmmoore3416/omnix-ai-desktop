<script lang="ts">
  /**
   * Minimal SVG sparkline. `values` are plotted on a fixed 0..max scale
   * (so 50% always sits at mid-height); `null` values are gaps (not measured).
   * Colour comes from `currentColor` — set it with a text-* class.
   */
  let {
    values,
    max = 100,
    height = 40,
    label = ''
  }: { values: (number | null)[]; max?: number; height?: number; label?: string } = $props();

  const W = 200;

  let top = $derived(max > 0 ? max : Math.max(1, ...values.map((v) => v ?? 0)));
  let segments = $derived.by(() => {
    const n = values.length;
    if (n < 2) return [] as string[];
    const segs: string[] = [];
    let cur: string[] = [];
    values.forEach((v, i) => {
      if (v == null || !Number.isFinite(v)) {
        if (cur.length > 1) segs.push(cur.join(' '));
        cur = [];
        return;
      }
      const x = (i / (n - 1)) * W;
      const y = height - (Math.min(Math.max(v, 0), top) / top) * (height - 2) - 1;
      cur.push(`${x.toFixed(1)},${y.toFixed(1)}`);
    });
    if (cur.length > 1) segs.push(cur.join(' '));
    return segs;
  });
</script>

<svg viewBox="0 0 {W} {height}" preserveAspectRatio="none" class="w-full block" style="height: {height}px" role="img" aria-label={label}>
  <line x1="0" x2={W} y1={height / 2} y2={height / 2} stroke="currentColor" stroke-opacity="0.08" stroke-dasharray="2 3" />
  {#each segments as pts}
    <polygon points="{pts.split(' ')[0].split(',')[0]},{height} {pts} {pts.split(' ').at(-1)?.split(',')[0]},{height}" fill="currentColor" fill-opacity="0.12" />
    <polyline points={pts} fill="none" stroke="currentColor" stroke-width="1.5" vector-effect="non-scaling-stroke" stroke-linejoin="round" />
  {/each}
</svg>
