<script lang="ts">
  import Sparkline from '../Sparkline.svelte';
  import { bytes, duration, loadColor, pct, rate } from '$lib/format';
  import type { PerformanceData, SystemInfo } from '$lib/types';

  let { perf, info }: { perf: PerformanceData | null; info: SystemInfo | null } = $props();

  let hist = $derived(perf?.history ?? []);
  let latest = $derived(hist.at(-1));
  let host = $derived(perf?.host);
  let memPct = $derived(host ? (host.memory_used / Math.max(1, host.memory_total)) * 100 : null);
  let netMax = $derived(Math.max(1, ...hist.map((s) => s.net_rx + s.net_tx)));
</script>

<div class="space-y-6">
  <h3 class="text-xl font-bold text-cosmic-cyan">Overview</h3>

  {#if !perf}
    <p class="text-sm text-gray-400">Collecting metrics…</p>
  {:else}
    <!-- KPI tiles with 15-minute history -->
    <div class="grid grid-cols-2 xl:grid-cols-4 gap-4">
      <div class="glass-panel p-4 bg-white/5">
        <div class="flex justify-between items-baseline">
          <span class="text-sm text-gray-400">CPU</span>
          <span class="text-2xl font-bold tabular-nums {loadColor(latest?.cpu)}">{pct(latest?.cpu ?? host?.cpu)}</span>
        </div>
        <div class="text-cosmic-cyan mt-2"><Sparkline values={hist.map((s) => s.cpu)} label="CPU history" /></div>
        <div class="text-xs text-gray-500 mt-1">load {host?.load.map((l) => l.toFixed(2)).join(' · ')}</div>
      </div>
      <div class="glass-panel p-4 bg-white/5">
        <div class="flex justify-between items-baseline">
          <span class="text-sm text-gray-400">Memory</span>
          <span class="text-2xl font-bold tabular-nums {loadColor(memPct)}">{pct(memPct)}</span>
        </div>
        <div class="text-cosmic-purple mt-2"><Sparkline values={hist.map((s) => s.memory)} label="Memory history" /></div>
        <div class="text-xs text-gray-500 mt-1">{bytes(host?.memory_used)} of {bytes(host?.memory_total)} · {bytes(host?.memory_available)} available</div>
      </div>
      {#each perf.gpus as g, i (g.pci ?? g.index)}
        <div class="glass-panel p-4 bg-white/5">
          <div class="flex justify-between items-baseline gap-2">
            <span class="text-sm text-gray-400 truncate" title={g.name}>GPU {g.index} · {g.vendor.toUpperCase()}</span>
            <span class="text-2xl font-bold tabular-nums {loadColor(g.utilization)}">{pct(g.utilization)}</span>
          </div>
          <div class="text-green-400 mt-2"><Sparkline values={hist.map((s) => s.gpus[i]?.util ?? null)} label="GPU {g.index} load history" /></div>
          <div class="text-xs text-gray-500 mt-1 truncate">
            {#if g.memory_total}{bytes(g.memory_used)} / {bytes(g.memory_total)} VRAM{/if}{#if g.temperature != null} · {g.temperature.toFixed(0)} °C{/if}
          </div>
        </div>
      {/each}
      <div class="glass-panel p-4 bg-white/5">
        <div class="flex justify-between items-baseline">
          <span class="text-sm text-gray-400">Network</span>
          <span class="text-lg font-bold tabular-nums text-yellow-300">↓ {rate(latest?.net_rx)} ↑ {rate(latest?.net_tx)}</span>
        </div>
        <div class="text-yellow-300 mt-2"><Sparkline values={hist.map((s) => s.net_rx + s.net_tx)} max={netMax} label="Network history" /></div>
        <div class="text-xs text-gray-500 mt-1">peak {rate(netMax)} in window</div>
      </div>
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
      <div class="glass-panel p-4 bg-white/5">
        <h4 class="font-bold mb-3 text-sm">This computer</h4>
        <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
          <dt class="text-gray-400">Host</dt><dd>{info?.hostname ?? '—'}</dd>
          <dt class="text-gray-400">OS</dt><dd>{info?.os ?? '—'} {info?.os_version ?? ''}</dd>
          <dt class="text-gray-400">Kernel</dt><dd>{info?.kernel ?? '—'}</dd>
          <dt class="text-gray-400">CPU</dt><dd>{info?.cpu_model ?? '—'} · {host?.per_core.length} threads{#if host?.cpu_freq_mhz} · {(host.cpu_freq_mhz / 1000).toFixed(2)} GHz{/if}</dd>
          <dt class="text-gray-400">Uptime</dt><dd>{host ? duration(host.uptime) : '—'}</dd>
          <dt class="text-gray-400">Processes</dt><dd>{host?.processes ?? '—'}</dd>
          {#if host?.sensors.length}
            <dt class="text-gray-400">Hottest</dt><dd>{host.sensors[0].label} {host.sensors[0].temperature.toFixed(0)} °C</dd>
          {/if}
        </dl>
      </div>
      <div class="glass-panel p-4 bg-white/5">
        <h4 class="font-bold mb-3 text-sm">Storage</h4>
        <div class="space-y-3">
          {#each host?.disks ?? [] as d (d.name + d.mount)}
            {@const p = (d.used / Math.max(1, d.total)) * 100}
            <div>
              <div class="flex justify-between text-sm"><span class="truncate" title={d.name}>{d.mount}</span><span class="tabular-nums {loadColor(p)}">{pct(p)}</span></div>
              <div class="w-full h-2 bg-white/10 rounded-full overflow-hidden mt-1">
                <div class="h-full {p >= 90 ? 'bg-red-400' : p >= 70 ? 'bg-yellow-400' : 'bg-cosmic-cyan'}" style="width: {p}%"></div>
              </div>
              <div class="text-xs text-gray-500 mt-0.5">{bytes(d.total - d.used)} free of {bytes(d.total)} · {d.fs}</div>
            </div>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>
