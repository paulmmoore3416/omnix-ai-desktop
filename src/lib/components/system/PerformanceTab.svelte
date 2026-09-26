<script lang="ts">
  import Sparkline from '../Sparkline.svelte';
  import { ago, bytes, duration, loadColor, ms, pct, rate } from '$lib/format';
  import type { PerformanceData } from '$lib/types';

  let { perf }: { perf: PerformanceData | null } = $props();

  let hist = $derived(perf?.history ?? []);
  let agent = $derived(perf?.agent);
  let loaded = $derived(perf?.models.loaded ?? []);
  let installed = $derived(perf?.models.installed ?? []);
  let successRate = $derived(
    agent && agent.turns > 0 ? ((agent.turns - agent.errors - agent.cancelled) / agent.turns) * 100 : null
  );
  let recallRate = $derived(agent && agent.recall.runs > 0 ? ((agent.recall.runs - agent.recall.empty) / agent.recall.runs) * 100 : null);
</script>

<div class="space-y-6">
  <h3 class="text-xl font-bold text-cosmic-cyan">Performance</h3>
  {#if !perf}
    <p class="text-sm text-gray-400">Collecting metrics…</p>
  {:else}
    <!-- GPUs -->
    <section class="space-y-3">
      <h4 class="font-bold text-sm uppercase tracking-wide text-gray-400">GPUs ({perf.gpus.length})</h4>
      {#if perf.gpus.length === 0}
        <p class="text-sm text-gray-400">No GPU detected.</p>
      {/if}
      <div class="grid grid-cols-1 xl:grid-cols-2 gap-4">
        {#each perf.gpus as g, i (g.pci ?? g.index)}
          {@const memP = g.memory_total ? ((g.memory_used ?? 0) / g.memory_total) * 100 : null}
          <div class="glass-panel p-4 bg-white/5 space-y-3">
            <div class="flex justify-between items-start gap-3">
              <div class="min-w-0">
                <div class="font-bold truncate" title={g.name}>GPU {g.index} · {g.name}</div>
                <div class="text-xs text-gray-400">{g.vendor.toUpperCase()} · driver {g.driver ?? 'none'}{#if g.pci} · {g.pci}{/if}</div>
              </div>
              <div class="text-right shrink-0">
                <div class="text-2xl font-bold tabular-nums {loadColor(g.utilization)}">{pct(g.utilization)}</div>
                <div class="text-xs text-gray-400">load</div>
              </div>
            </div>
            {#if g.note}<p class="text-xs text-yellow-300">{g.note}</p>{/if}
            <div class="grid grid-cols-2 gap-3">
              <div>
                <div class="text-xs text-gray-400 mb-1">Load, 15 min</div>
                <div class="text-green-400"><Sparkline values={hist.map((s) => s.gpus[i]?.util ?? null)} height={32} label="GPU load" /></div>
              </div>
              <div>
                <div class="text-xs text-gray-400 mb-1">VRAM, 15 min</div>
                <div class="text-cosmic-purple"><Sparkline values={hist.map((s) => s.gpus[i]?.mem ?? null)} height={32} label="GPU memory" /></div>
              </div>
            </div>
            <dl class="grid grid-cols-2 sm:grid-cols-4 gap-2 text-sm tabular-nums">
              <div><dt class="text-xs text-gray-400">VRAM</dt><dd>{bytes(g.memory_used)} / {bytes(g.memory_total)} <span class="{loadColor(memP)}">({pct(memP)})</span></dd></div>
              <div><dt class="text-xs text-gray-400">Temperature</dt><dd>{g.temperature != null ? `${g.temperature.toFixed(0)} °C` : '—'}</dd></div>
              <div><dt class="text-xs text-gray-400">Power</dt><dd>{g.power_w != null ? `${g.power_w.toFixed(0)} W` : '—'}{#if g.power_limit_w} / {g.power_limit_w.toFixed(0)} W{/if}</dd></div>
              <div><dt class="text-xs text-gray-400">Clock · Fan</dt><dd>{g.clock_mhz ?? '—'}{#if g.clock_max_mhz}/{g.clock_max_mhz}{/if} MHz · {pct(g.fan_percent)}</dd></div>
            </dl>
            {#if g.processes.length}
              <div class="text-xs">
                <div class="text-gray-400 mb-1">Processes using VRAM</div>
                {#each g.processes as p (p.pid)}
                  <div class="flex justify-between"><span class="truncate">{p.name} <span class="text-gray-500">({p.pid})</span></span><span class="tabular-nums">{bytes(p.memory)}</span></div>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </section>

    <!-- Host -->
    <section class="grid grid-cols-1 xl:grid-cols-2 gap-4">
      <div class="glass-panel p-4 bg-white/5">
        <h4 class="font-bold text-sm mb-3">CPU cores ({perf.host.per_core.length}) · {pct(perf.host.cpu)} total</h4>
        <div class="grid gap-1" style="grid-template-columns: repeat(auto-fill, minmax(1.75rem, 1fr));">
          {#each perf.host.per_core as c, i}
            <div class="h-12 bg-white/10 rounded relative overflow-hidden" title="core {i}: {c.toFixed(0)}%">
              <div class="absolute bottom-0 left-0 right-0 {c >= 90 ? 'bg-red-400' : c >= 70 ? 'bg-yellow-400' : 'bg-cosmic-cyan'}" style="height: {c}%"></div>
            </div>
          {/each}
        </div>
        <div class="text-xs text-gray-400 mt-2">
          load average {perf.host.load.map((l) => l.toFixed(2)).join(' / ')} (1/5/15 min){#if perf.host.cpu_freq_mhz} · {(perf.host.cpu_freq_mhz / 1000).toFixed(2)} GHz avg{/if}
        </div>
      </div>
      <div class="glass-panel p-4 bg-white/5 space-y-3">
        <h4 class="font-bold text-sm">Memory</h4>
        <div class="text-cosmic-purple"><Sparkline values={hist.map((s) => s.memory)} height={36} label="memory history" /></div>
        <dl class="grid grid-cols-2 gap-2 text-sm tabular-nums">
          <div><dt class="text-xs text-gray-400">Used</dt><dd>{bytes(perf.host.memory_used)} / {bytes(perf.host.memory_total)}</dd></div>
          <div><dt class="text-xs text-gray-400">Available</dt><dd>{bytes(perf.host.memory_available)}</dd></div>
          <div><dt class="text-xs text-gray-400">Swap</dt><dd>{bytes(perf.host.swap_used)} / {bytes(perf.host.swap_total)}</dd></div>
          <div><dt class="text-xs text-gray-400">Uptime</dt><dd>{duration(perf.host.uptime)}</dd></div>
        </dl>
      </div>
      <div class="glass-panel p-4 bg-white/5">
        <h4 class="font-bold text-sm mb-3">Network interfaces</h4>
        <table class="w-full text-sm tabular-nums">
          <thead><tr class="text-xs text-gray-400 text-left"><th class="font-normal">Interface</th><th class="font-normal text-right">↓ now</th><th class="font-normal text-right">↑ now</th><th class="font-normal text-right">↓ total</th></tr></thead>
          <tbody>
            {#each perf.host.network.filter((n) => n.total_rx + n.total_tx > 0) as n (n.name)}
              <tr class="border-t border-white/5"><td class="py-1 truncate max-w-[10rem]">{n.name}</td><td class="text-right">{rate(n.rx)}</td><td class="text-right">{rate(n.tx)}</td><td class="text-right text-gray-400">{bytes(n.total_rx)}</td></tr>
            {/each}
          </tbody>
        </table>
      </div>
      <div class="glass-panel p-4 bg-white/5">
        <h4 class="font-bold text-sm mb-3">Temperature sensors</h4>
        {#if perf.host.sensors.length === 0}
          <p class="text-sm text-gray-400">No sensors exposed.</p>
        {:else}
          <div class="grid grid-cols-2 gap-x-4 gap-y-1 text-sm tabular-nums">
            {#each perf.host.sensors.slice(0, 12) as s (s.label)}
              <div class="flex justify-between gap-2"><span class="truncate text-gray-300" title={s.label}>{s.label}</span><span class="{s.critical && s.temperature >= s.critical - 5 ? 'text-red-400' : s.temperature >= 80 ? 'text-yellow-400' : ''}">{s.temperature.toFixed(0)} °C</span></div>
            {/each}
          </div>
        {/if}
      </div>
    </section>

    <!-- Agent -->
    {#if agent}
      <section class="space-y-3">
        <h4 class="font-bold text-sm uppercase tracking-wide text-gray-400">Agent · since {duration(agent.uptime_s)} ago</h4>
        <div class="grid grid-cols-2 lg:grid-cols-4 gap-4">
          <div class="glass-panel p-4 bg-white/5"><div class="text-2xl font-bold tabular-nums">{agent.turns}</div><div class="text-xs text-gray-400">replies · {pct(successRate)} completed</div></div>
          <div class="glass-panel p-4 bg-white/5"><div class="text-2xl font-bold tabular-nums">{agent.tools.reduce((a, t) => a + t.calls, 0)}</div><div class="text-xs text-gray-400">tool calls · {agent.tools.reduce((a, t) => a + t.failed, 0)} failed</div></div>
          <div class="glass-panel p-4 bg-white/5"><div class="text-2xl font-bold tabular-nums">{agent.recall.hits}</div><div class="text-xs text-gray-400">memories recalled · {pct(recallRate)} of replies</div></div>
          <div class="glass-panel p-4 bg-white/5"><div class="text-2xl font-bold tabular-nums">{agent.capture.saved}</div><div class="text-xs text-gray-400">facts learned in {agent.capture.runs} runs</div></div>
        </div>
        <div class="grid grid-cols-1 xl:grid-cols-2 gap-4">
          <div class="glass-panel p-4 bg-white/5">
            <h5 class="font-bold text-sm mb-2">Models</h5>
            {#if agent.models.length === 0}
              <p class="text-sm text-gray-400">No replies yet.</p>
            {:else}
              <table class="w-full text-sm tabular-nums">
                <thead><tr class="text-xs text-gray-400 text-left"><th class="font-normal">Model</th><th class="font-normal text-right">Tokens/s</th><th class="font-normal text-right">First token</th><th class="font-normal text-right">Output</th><th class="font-normal text-right">Cold starts</th></tr></thead>
                <tbody>
                  {#each agent.models as m (m.model)}
                    <tr class="border-t border-white/5">
                      <td class="py-1 truncate max-w-[10rem]" title={m.model}>{m.model}</td>
                      <td class="text-right">{m.tokens_per_sec != null ? m.tokens_per_sec.toFixed(1) : '—'}</td>
                      <td class="text-right">{ms(m.avg_ttft_ms)}</td>
                      <td class="text-right" title={m.tokens_estimated ? 'partly estimated (≈4 chars/token)' : 'reported by the model server'}>{m.tokens_estimated ? '≈' : ''}{m.output_tokens.toLocaleString()}</td>
                      <td class="text-right">{m.cold_starts}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            {/if}
          </div>
          <div class="glass-panel p-4 bg-white/5">
            <h5 class="font-bold text-sm mb-2">Tools</h5>
            {#if agent.tools.length === 0}
              <p class="text-sm text-gray-400">No tool calls yet.</p>
            {:else}
              <table class="w-full text-sm tabular-nums">
                <thead><tr class="text-xs text-gray-400 text-left"><th class="font-normal">Tool</th><th class="font-normal text-right">Calls</th><th class="font-normal text-right">Failed</th><th class="font-normal text-right">Avg</th><th class="font-normal text-right">Max</th></tr></thead>
                <tbody>
                  {#each [...agent.tools].sort((a, b) => b.calls - a.calls) as t (t.tool)}
                    <tr class="border-t border-white/5">
                      <td class="py-1 truncate max-w-[12rem]" title={t.tool}>{t.tool}</td>
                      <td class="text-right">{t.calls}</td>
                      <td class="text-right {t.failed ? 'text-red-400' : ''}">{t.failed}</td>
                      <td class="text-right">{ms(t.avg_ms)}</td>
                      <td class="text-right">{ms(t.max_ms)}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            {/if}
          </div>
        </div>
        {#if agent.recent.length}
          <div class="glass-panel p-4 bg-white/5">
            <h5 class="font-bold text-sm mb-2">Recent replies</h5>
            <div class="space-y-1 text-sm tabular-nums">
              {#each agent.recent.slice(0, 8) as r (r.ts + r.duration_ms)}
                <div class="flex gap-3 items-center">
                  <span class="w-2 h-2 rounded-full shrink-0 {r.outcome === 'ok' ? 'bg-green-400' : r.outcome === 'cancelled' ? 'bg-gray-400' : 'bg-red-400'}"></span>
                  <span class="text-gray-400 w-20 shrink-0">{ago(r.ts)}</span>
                  <span class="truncate flex-1">{r.model}</span>
                  <span class="text-gray-300">{ms(r.duration_ms)}</span>
                  <span class="text-gray-500 hidden sm:inline">first token {ms(r.ttft_ms)} · {r.output_tokens} tok · {r.tools} tools · {r.recalled} recalled</span>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      </section>
    {/if}

    <!-- Models in memory -->
    <section class="glass-panel p-4 bg-white/5 space-y-3">
      <h4 class="font-bold text-sm">Models in memory</h4>
      {#if perf.models.error}
        <p class="text-sm text-yellow-300">{perf.models.error}</p>
      {:else if loaded.length === 0}
        <p class="text-sm text-gray-400">No model is loaded ({installed.length} installed). The first reply loads one.</p>
      {:else}
        {#each loaded as m (m.name)}
          <div class="space-y-1">
            <div class="flex justify-between text-sm gap-3">
              <span class="font-medium truncate">{m.name}{#if m.name === perf.models.configured}<span class="ml-2 text-xs text-cosmic-cyan">in use</span>{/if}</span>
              <span class="tabular-nums text-gray-300 shrink-0">{bytes(m.size)} · {m.gpu_percent.toFixed(0)}% on GPU{#if m.context_length} · ctx {m.context_length.toLocaleString()}{/if}</span>
            </div>
            <div class="w-full h-2 bg-white/10 rounded-full overflow-hidden flex" title="GPU {bytes(m.size_vram)} · system RAM {bytes(m.size - m.size_vram)}">
              <div class="h-full bg-green-400" style="width: {m.gpu_percent}%"></div>
              <div class="h-full bg-yellow-400" style="width: {100 - m.gpu_percent}%"></div>
            </div>
            <div class="text-xs text-gray-500">{m.parameter_size ?? ''} {m.quantization ?? ''} · unloads {ago(m.expires_at)}</div>
          </div>
        {/each}
      {/if}
      {#if perf.memory_store}
        <div class="border-t border-white/10 pt-3 text-sm text-gray-300">
          Memory engine: {perf.memory_store.embed_model} embeddings · {perf.memory_store.vectors ?? 0} vectors ·
          {perf.memory_store.searches_24h ?? 0} searches today (avg {ms(perf.memory_store.avg_search_ms)}) ·
          learning model {perf.memory_store.llm_model ?? 'off'}
        </div>
      {/if}
    </section>
  {/if}
</div>
