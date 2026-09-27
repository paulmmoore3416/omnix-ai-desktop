<script lang="ts">
  /**
   * Compact live metrics for the workspace dock: host, GPUs, loaded models,
   * disks and assistant throughput. Polls only while `active` (tab visible).
   *
   * Smooth by design: cheap readings (`get_real_time_stats`, `get_gpus`)
   * arrive every 1.5 s, the heavier `get_performance` (models, disks, agent,
   * history seed) every 10 s. Charts glide on an animation clock that trails
   * real time by one poll, numbers tween, and bars ease, so nothing jumps.
   */
  import { onDestroy } from 'svelte';
  import { Tween } from 'svelte/motion';
  import { cubicOut } from 'svelte/easing';
  import { call, errorMessage } from '$lib/api';
  import { bytes, loadColor, ms, pct, rate } from '$lib/format';
  import { mergeSeries } from '$lib/sparkline';
  import type { GpuInfo, PerformanceData, RealTimeStats } from '$lib/types';
  import LiveSpark from './LiveSpark.svelte';

  let {
    active = true,
    runningTasks = 0,
    notify
  }: {
    active?: boolean;
    runningTasks?: number;
    notify: (msg: string, type?: 'info' | 'success' | 'error') => void;
  } = $props();

  /** Live reading cadence and chart window. */
  const POLL_MS = 1500;
  const DETAIL_MS = 10_000;
  const WINDOW_MS = 120_000;

  type Point = { t: number; cpu: number; mem: number; net: number; gpus: (number | null)[] };

  let perf = $state<PerformanceData | null>(null);
  let gpus = $state<GpuInfo[]>([]);
  let rt = $state<RealTimeStats | null>(null);
  let error = $state('');
  let unloading = $state<string | null>(null);

  let seed = $state<Point[]>([]);
  let live = $state<Point[]>([]);
  let series = $derived(mergeSeries(seed, live, Date.now(), WINDOW_MS * 1.2));

  // Chart clock: trails real time by one poll so new readings slide in.
  let clock = $state(Date.now() - POLL_MS);
  const reducedMotion =
    typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

  // Tweened read-outs.
  const tween = () => new Tween(0, { duration: 900, easing: cubicOut });
  const cpuT = tween();
  const memT = tween();
  let gpuT = $state<Tween<number>[]>([]);

  let host = $derived(perf?.host);
  let netMax = $derived(Math.max(1, ...series.map((s) => s.net)));
  let disks = $derived(
    (host?.disks ?? [])
      .filter((d) => !d.removable && d.total > 0)
      .map((d) => ({ ...d, p: (d.used / d.total) * 100 }))
      .sort((a, b) => b.p - a.p)
      .slice(0, 3)
  );
  let chatModel = $derived(perf?.agent.models.find((m) => m.model === perf?.models.configured));
  let cpuVals = $derived(series.map((p) => ({ t: p.t, v: p.cpu })));
  let memVals = $derived(series.map((p) => ({ t: p.t, v: p.mem })));
  let netVals = $derived(series.map((p) => ({ t: p.t, v: p.net })));
  let gpuVals = $derived(gpus.map((_, i) => series.map((p) => ({ t: p.t, v: p.gpus[i] ?? null }))));

  function gpuTweens(n: number) {
    if (gpuT.length !== n) gpuT = Array.from({ length: n }, (_, i) => gpuT[i] ?? tween());
  }

  function settle(v: number | null | undefined, t: Tween<number>) {
    if (v != null && Number.isFinite(v)) t.target = v;
  }

  let detailBusy = false;
  async function loadDetail() {
    if (detailBusy) return;
    detailBusy = true;
    try {
      const p = await call<PerformanceData>('get_performance', { historySecs: Math.ceil((WINDOW_MS * 1.2) / 1000) });
      perf = p;
      seed = p.history.map((s) => ({
        t: s.ts,
        cpu: s.cpu,
        mem: s.memory,
        net: s.net_rx + s.net_tx,
        gpus: s.gpus.map((g) => g.util ?? null)
      }));
      if (!gpus.length) gpus = p.gpus;
      if (!rt) {
        settle(p.host.cpu, cpuT);
        settle((p.host.memory_used / Math.max(1, p.host.memory_total)) * 100, memT);
      }
      gpuTweens(gpus.length);
      error = '';
    } catch (e) {
      error = errorMessage(e);
    } finally {
      detailBusy = false;
    }
  }

  let liveBusy = false;
  async function loadLive() {
    if (liveBusy) return;
    liveBusy = true;
    try {
      const [r, g] = await Promise.allSettled([
        call<RealTimeStats>('get_real_time_stats'),
        call<GpuInfo[]>('get_gpus')
      ]);
      const now = Date.now();
      if (g.status === 'fulfilled') {
        gpus = g.value;
        gpuTweens(gpus.length);
        gpus.forEach((x, i) => settle(x.utilization, gpuT[i]));
      }
      if (r.status === 'fulfilled') {
        rt = r.value;
        settle(rt.cpu, cpuT);
        settle(rt.memory, memT);
        const point: Point = {
          t: now,
          cpu: rt.cpu,
          mem: rt.memory,
          net: (rt.network?.rx ?? 0) + (rt.network?.tx ?? 0),
          gpus: gpus.map((x) => x.utilization)
        };
        live = [...live.filter((p) => p.t >= now - WINDOW_MS * 1.2), point];
        error = '';
      } else {
        error = errorMessage(r.reason);
      }
      if (reducedMotion) clock = now;
    } finally {
      liveBusy = false;
    }
  }

  // Polling, only while the tab is showing.
  $effect(() => {
    if (!active) return;
    loadDetail();
    loadLive();
    const a = setInterval(loadLive, POLL_MS);
    const b = setInterval(loadDetail, DETAIL_MS);
    return () => {
      clearInterval(a);
      clearInterval(b);
    };
  });

  // Animation clock (~30 fps), paused while hidden or when motion is reduced.
  let raf = 0;
  $effect(() => {
    if (!active || reducedMotion || typeof requestAnimationFrame !== 'function') return;
    let last = 0;
    const frame = (ts: number) => {
      if (ts - last >= 33) {
        last = ts;
        clock = Date.now() - POLL_MS;
      }
      raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);
    return () => cancelAnimationFrame(raf);
  });
  onDestroy(() => cancelAnimationFrame(raf));

  async function unload(name: string) {
    unloading = name;
    try {
      await call('model_unload', { model: name });
      notify(`Unloaded ${name} (frees its VRAM; it reloads on next use)`, 'success');
      await loadDetail();
    } catch (e) {
      notify(errorMessage(e), 'error');
    } finally {
      unloading = null;
    }
  }
</script>

<div class="live space-y-3">
  {#if error && !perf && !rt}
    <p class="err">{error}</p>
  {:else if !perf && !rt}
    <p class="sub">Collecting metrics…</p>
  {:else}
    <div class="grid grid-cols-2 gap-2">
      <div class="tile">
        <div class="flex justify-between items-baseline"><span class="label">CPU</span><span class="value {loadColor(cpuT.current)}">{pct(cpuT.current)}</span></div>
        <div class="c-cpu"><LiveSpark values={cpuVals} now={clock} window={WINDOW_MS} height={30} label="CPU, last 2 minutes" /></div>
      </div>
      <div class="tile">
        <div class="flex justify-between items-baseline"><span class="label">Memory</span><span class="value {loadColor(memT.current)}">{pct(memT.current)}</span></div>
        <div class="c-mem"><LiveSpark values={memVals} now={clock} window={WINDOW_MS} height={30} label="Memory, last 2 minutes" /></div>
      </div>
    </div>

    {#each gpus as g, i (g.pci ?? g.index)}
      {@const vram = g.memory_total ? ((g.memory_used ?? 0) / g.memory_total) * 100 : null}
      {@const util = gpuT[i]?.current ?? g.utilization}
      <div class="tile">
        <div class="flex justify-between items-baseline gap-2">
          <span class="label truncate" title={g.name}>GPU {g.index} · {g.vendor.toUpperCase()} {g.name}</span>
          <span class="value {loadColor(util)}">{pct(g.utilization == null ? null : util)}</span>
        </div>
        <div class="c-gpu"><LiveSpark values={gpuVals[i] ?? []} now={clock} window={WINDOW_MS} height={24} label="GPU {g.index} load" /></div>
        {#if vram != null}
          <div class="bar mt-1"><div class="fill {vram >= 90 ? 'hot' : 'gpu'}" style="width: {vram}%"></div></div>
        {/if}
        <div class="sub mt-0.5 truncate">
          {#if g.memory_total}{bytes(g.memory_used)} / {bytes(g.memory_total)} VRAM{/if}{#if g.temperature != null} · {g.temperature.toFixed(0)} °C{/if}{#if g.power_w != null} · {g.power_w.toFixed(0)} W{/if}
          {#if g.note}<span title={g.note}> · ⓘ</span>{/if}
        </div>
      </div>
    {/each}

    {#if perf}
      <div class="tile">
        <div class="label mb-1">Loaded models</div>
        {#if perf.models.loaded?.length}
          <ul class="space-y-1">
            {#each perf.models.loaded as m (m.name)}
              <li class="flex items-center gap-2">
                <span class="flex-1 truncate body" title={m.name}>{m.name}</span>
                <span class="sub tabular-nums {m.gpu_percent < 100 ? 'text-amber-300' : ''}" title="Share of the model on GPU">{bytes(m.size_vram)} · {m.gpu_percent.toFixed(0)}% GPU</span>
                <button class="mini" disabled={unloading === m.name} onclick={() => unload(m.name)} title="Unload from memory">⏏</button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="sub">{perf.models.error ? `Ollama: ${perf.models.error}` : 'None loaded (the next request loads the chat model)'}</p>
        {/if}
      </div>
    {/if}

    <div class="grid grid-cols-2 gap-2">
      <div class="tile">
        <div class="label">Network</div>
        <div class="small tabular-nums c-net">↓ {rate(rt?.network?.rx)} ↑ {rate(rt?.network?.tx)}</div>
        <div class="c-net"><LiveSpark values={netVals} now={clock} window={WINDOW_MS} max={netMax} height={22} label="Network" /></div>
      </div>
      <div class="tile">
        <div class="label">Assistant</div>
        {#if perf}
          <div class="small body tabular-nums">{perf.agent.turns} turns · {perf.agent.errors} errors</div>
          <div class="small body tabular-nums">
            {chatModel?.tokens_per_sec != null ? `${chatModel.tokens_per_sec.toFixed(1)} tok/s` : '— tok/s'} · TTFT {ms(chatModel?.avg_ttft_ms)}
          </div>
        {/if}
        <div class="small {runningTasks ? 'accent' : 'sub'}">{runningTasks} side task{runningTasks === 1 ? '' : 's'} running</div>
      </div>
    </div>

    {#if disks.length}
      <div class="tile space-y-1.5">
        <div class="label">Storage</div>
        {#each disks as d (d.name + d.mount)}
          <div>
            <div class="flex justify-between small"><span class="truncate body" title={d.name}>{d.mount}</span><span class="tabular-nums {loadColor(d.p)}">{pct(d.p)} · {bytes(d.total - d.used)} free</span></div>
            <div class="bar"><div class="fill {d.p >= 90 ? 'hot' : d.p >= 70 ? 'warm' : 'cool'}" style="width: {d.p}%"></div></div>
          </div>
        {/each}
      </div>
    {/if}
    {#if error}<p class="err">Last refresh failed: {error}</p>{/if}
  {/if}
</div>

<style>
  .tile {
    padding: 0.65rem 0.75rem;
    border-radius: 12px;
    background: var(--tile, rgba(255, 255, 255, 0.05));
    border: 1px solid var(--tile-border, rgba(255, 255, 255, 0.08));
  }
  .label {
    font-size: 0.82em;
    font-weight: 600;
    letter-spacing: 0.01em;
    color: var(--text-dim, #c3cad6);
  }
  .value {
    font-size: 1.3em;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .small { font-size: 0.86em; }
  .body { color: var(--text, #eef1f6); }
  .sub { font-size: 0.8em; color: var(--text-faint, #9aa3b4); }
  .err { font-size: 0.8em; color: #fca5a5; }
  .accent { color: var(--accent, #22d3ee); }
  .c-cpu { color: var(--c-cpu, #38bdf8); }
  .c-mem { color: var(--c-mem, #a78bfa); }
  .c-gpu { color: var(--c-gpu, #34d399); }
  .c-net { color: var(--c-net, #fbbf24); }
  .bar {
    height: 5px;
    border-radius: 9999px;
    background: rgba(255, 255, 255, 0.1);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: 9999px;
    transition: width 0.9s cubic-bezier(0.22, 1, 0.36, 1), background-color 0.4s ease;
  }
  .fill.gpu { background: var(--c-gpu, #34d399); }
  .fill.cool { background: var(--accent, #22d3ee); }
  .fill.warm { background: #fbbf24; }
  .fill.hot { background: #f87171; }
  .mini {
    padding: 0 0.35rem;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.06);
    font-size: 0.85em;
  }
  .mini:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.14);
  }
  .mini:disabled {
    opacity: 0.4;
  }
  @media (prefers-reduced-motion: reduce) {
    .fill { transition: none; }
  }
</style>
