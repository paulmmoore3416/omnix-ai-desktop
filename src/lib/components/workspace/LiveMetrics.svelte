<script lang="ts">
  /**
   * Compact live metrics for the workspace dock: host, GPUs, loaded models,
   * disks and assistant throughput. Polls only while `active` (tab visible).
   */
  import { call, errorMessage } from '$lib/api';
  import { bytes, loadColor, ms, pct, rate } from '$lib/format';
  import type { PerformanceData } from '$lib/types';
  import Sparkline from '../Sparkline.svelte';

  let {
    active = true,
    runningTasks = 0,
    notify
  }: {
    active?: boolean;
    runningTasks?: number;
    notify: (msg: string, type?: 'info' | 'success' | 'error') => void;
  } = $props();

  let perf = $state<PerformanceData | null>(null);
  let error = $state('');
  let unloading = $state<string | null>(null);

  let hist = $derived(perf?.history ?? []);
  let host = $derived(perf?.host);
  let memPct = $derived(host ? (host.memory_used / Math.max(1, host.memory_total)) * 100 : null);
  let netMax = $derived(Math.max(1, ...hist.map((s) => s.net_rx + s.net_tx)));
  let latest = $derived(hist.at(-1));
  let disks = $derived(
    (host?.disks ?? [])
      .filter((d) => !d.removable && d.total > 0)
      .map((d) => ({ ...d, p: (d.used / d.total) * 100 }))
      .sort((a, b) => b.p - a.p)
      .slice(0, 3)
  );
  let chatModel = $derived(perf?.agent.models.find((m) => m.model === perf?.models.configured));

  let busy = false;
  async function load() {
    if (busy) return;
    busy = true;
    try {
      perf = await call<PerformanceData>('get_performance', { historySecs: 300 });
      error = '';
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    if (!active) return;
    load();
    const timer = setInterval(load, 4000);
    return () => clearInterval(timer);
  });

  async function unload(name: string) {
    unloading = name;
    try {
      await call('model_unload', { model: name });
      notify(`Unloaded ${name} (frees its VRAM; it reloads on next use)`, 'success');
      await load();
    } catch (e) {
      notify(errorMessage(e), 'error');
    } finally {
      unloading = null;
    }
  }
</script>

<div class="space-y-3 text-sm">
  {#if error && !perf}
    <p class="text-red-300 text-xs">{error}</p>
  {:else if !perf}
    <p class="text-gray-400 text-xs">Collecting metrics…</p>
  {:else}
    <div class="grid grid-cols-2 gap-2">
      <div class="tile">
        <div class="flex justify-between items-baseline"><span class="label">CPU</span><span class="value {loadColor(host?.cpu)}">{pct(host?.cpu)}</span></div>
        <div class="text-cosmic-cyan"><Sparkline values={hist.map((s) => s.cpu)} height={28} label="CPU, last 5 minutes" /></div>
      </div>
      <div class="tile">
        <div class="flex justify-between items-baseline"><span class="label">Memory</span><span class="value {loadColor(memPct)}">{pct(memPct)}</span></div>
        <div class="text-cosmic-purple"><Sparkline values={hist.map((s) => s.memory)} height={28} label="Memory, last 5 minutes" /></div>
      </div>
    </div>

    {#each perf.gpus as g, i (g.pci ?? g.index)}
      {@const vram = g.memory_total ? ((g.memory_used ?? 0) / g.memory_total) * 100 : null}
      <div class="tile">
        <div class="flex justify-between items-baseline gap-2">
          <span class="label truncate" title={g.name}>GPU {g.index} · {g.vendor.toUpperCase()} {g.name}</span>
          <span class="value {loadColor(g.utilization)}">{pct(g.utilization)}</span>
        </div>
        <div class="text-green-400"><Sparkline values={hist.map((s) => s.gpus[i]?.util ?? null)} height={22} label="GPU {g.index} load" /></div>
        {#if vram != null}
          <div class="bar mt-1"><div class="h-full {vram >= 90 ? 'bg-red-400' : 'bg-green-400'}" style="width: {vram}%"></div></div>
        {/if}
        <div class="text-[11px] text-gray-500 mt-0.5 truncate">
          {#if g.memory_total}{bytes(g.memory_used)} / {bytes(g.memory_total)} VRAM{/if}{#if g.temperature != null} · {g.temperature.toFixed(0)} °C{/if}{#if g.power_w != null} · {g.power_w.toFixed(0)} W{/if}
          {#if g.note}<span title={g.note}> · ⓘ</span>{/if}
        </div>
      </div>
    {/each}

    <div class="tile">
      <div class="label mb-1">Loaded models</div>
      {#if perf.models.loaded?.length}
        <ul class="space-y-1">
          {#each perf.models.loaded as m (m.name)}
            <li class="flex items-center gap-2">
              <span class="flex-1 truncate" title={m.name}>{m.name}</span>
              <span class="text-[11px] tabular-nums {m.gpu_percent < 100 ? 'text-yellow-300' : 'text-gray-400'}" title="Share of the model on GPU">{bytes(m.size_vram)} · {m.gpu_percent.toFixed(0)}% GPU</span>
              <button class="mini" disabled={unloading === m.name} onclick={() => unload(m.name)} title="Unload from memory">⏏</button>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="text-xs text-gray-500">{perf.models.error ? `Ollama: ${perf.models.error}` : 'None loaded (the next request loads the chat model)'}</p>
      {/if}
    </div>

    <div class="grid grid-cols-2 gap-2">
      <div class="tile">
        <div class="label">Network</div>
        <div class="text-xs tabular-nums text-yellow-300">↓ {rate(latest?.net_rx)} ↑ {rate(latest?.net_tx)}</div>
        <div class="text-yellow-300"><Sparkline values={hist.map((s) => s.net_rx + s.net_tx)} max={netMax} height={20} label="Network" /></div>
      </div>
      <div class="tile">
        <div class="label">Assistant</div>
        <div class="text-xs text-gray-300 tabular-nums">{perf.agent.turns} turns · {perf.agent.errors} errors</div>
        <div class="text-xs text-gray-300 tabular-nums">
          {chatModel?.tokens_per_sec != null ? `${chatModel.tokens_per_sec.toFixed(1)} tok/s` : '— tok/s'} · TTFT {ms(chatModel?.avg_ttft_ms)}
        </div>
        <div class="text-xs {runningTasks ? 'text-cosmic-cyan' : 'text-gray-500'}">{runningTasks} side task{runningTasks === 1 ? '' : 's'} running</div>
      </div>
    </div>

    {#if disks.length}
      <div class="tile space-y-1.5">
        <div class="label">Storage</div>
        {#each disks as d (d.name + d.mount)}
          <div>
            <div class="flex justify-between text-xs"><span class="truncate" title={d.name}>{d.mount}</span><span class="tabular-nums {loadColor(d.p)}">{pct(d.p)} · {bytes(d.total - d.used)} free</span></div>
            <div class="bar"><div class="h-full {d.p >= 90 ? 'bg-red-400' : d.p >= 70 ? 'bg-yellow-400' : 'bg-cosmic-cyan'}" style="width: {d.p}%"></div></div>
          </div>
        {/each}
      </div>
    {/if}
    {#if error}<p class="text-[11px] text-red-300">Last refresh failed: {error}</p>{/if}
  {/if}
</div>

<style>
  .tile {
    padding: 0.6rem 0.7rem;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }
  .label {
    font-size: 0.72rem;
    color: rgb(156 163 175);
  }
  .value {
    font-size: 1.1rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .bar {
    height: 4px;
    border-radius: 9999px;
    background: rgba(255, 255, 255, 0.1);
    overflow: hidden;
  }
  .mini {
    padding: 0 0.35rem;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.06);
    font-size: 0.75rem;
  }
  .mini:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.14);
  }
  .mini:disabled {
    opacity: 0.4;
  }
</style>
