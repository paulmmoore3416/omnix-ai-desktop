<script lang="ts">
  /**
   * Live sparkline placed by time. The curve is rebuilt only when readings
   * arrive; between readings the whole chart glides left with `now`, so
   * motion is continuous instead of a jump per sample. `now` should trail
   * real time by about one poll interval, so each new reading slides in from
   * the right edge rather than popping in.
   */
  import { areaPath, smoothPath, timedRuns, type TimedValue } from '$lib/sparkline';

  let {
    values,
    now,
    window = 120_000,
    max = 100,
    height = 28,
    maxGap = 20_000,
    label = ''
  }: {
    values: TimedValue[];
    /** Chart clock, ms (the right edge). */
    now: number;
    window?: number;
    max?: number;
    height?: number;
    /** Readings further apart than this are drawn as a gap. */
    maxGap?: number;
    label?: string;
  } = $props();

  const W = 200;

  // Anchor the geometry at the newest reading; `now` only moves the group.
  let origin = $derived(values.length ? values[values.length - 1].t : 0);
  let runs = $derived(
    timedRuns(values, { origin, window, width: W, height, max, maxGap }).map((pts) => {
      const line = smoothPath(pts);
      return { line, area: areaPath(line, pts, height) };
    })
  );
  let shift = $derived(((origin - now) / window) * W);
</script>

<svg viewBox="0 0 {W} {height}" preserveAspectRatio="none" class="w-full block overflow-hidden" style="height: {height}px" role="img" aria-label={label}>
  <line x1="0" x2={W} y1={height / 2} y2={height / 2} stroke="currentColor" stroke-opacity="0.08" stroke-dasharray="2 3" />
  <g transform="translate({shift.toFixed(2)} 0)">
    {#each runs as r}
      <path d={r.area} fill="currentColor" fill-opacity="0.14" />
      <path d={r.line} fill="none" stroke="currentColor" stroke-width="1.6" vector-effect="non-scaling-stroke" stroke-linejoin="round" stroke-linecap="round" />
    {/each}
  </g>
</svg>
