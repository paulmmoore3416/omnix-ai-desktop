<script lang="ts">
  import { onMount } from 'svelte';
  import { call, errorMessage } from '$lib/api';
  import type { UsageSummary } from '$lib/types';

  // Reference cloud prices (USD per million tokens) used only for the
  // "what this would have cost" estimate. Editable, remembered per browser
  // profile; they are not model ids and nothing is fetched.
  const RATE_KEY = 'omnix.usage.rates';
  let rates = $state(loadRates());
  let windowDays = $state(30);
  let summary = $state<UsageSummary | null>(null);
  let error = $state('');
  let confirmClear = $state(false);

  function loadRates(): { input: number; output: number } {
    try {
      const r = JSON.parse(localStorage.getItem(RATE_KEY) ?? '');
      if (typeof r?.input === 'number' && typeof r?.output === 'number') return r;
    } catch {
      /* default below */
    }
    return { input: 3, output: 15 };
  }

  function saveRates() {
    try {
      localStorage.setItem(RATE_KEY, JSON.stringify(rates));
    } catch {
      /* storage unavailable: keep in memory */
    }
  }

  onMount(refresh);

  async function refresh() {
    try {
      summary = await call<UsageSummary>('usage_summary', { days: windowDays });
      error = '';
    } catch (e) {
      error = errorMessage(e);
    }
  }

  async function clear() {
    if (!confirmClear) {
      confirmClear = true;
      return;
    }
    confirmClear = false;
    try {
      summary = await call<UsageSummary>('usage_clear');
    } catch (e) {
      error = errorMessage(e);
    }
  }

  const fmt = (n: number) => n.toLocaleString();
  const hours = (ms: number) => (ms / 3_600_000).toFixed(ms < 360_000 ? 2 : 1);

  let saved = $derived(
    summary
      ? (summary.window.local_prompt_tokens * rates.input + summary.window.local_output_tokens * rates.output) / 1_000_000
      : 0
  );
  let peak = $derived(Math.max(1, ...(summary?.days.map((d) => d.turns) ?? [1])));
  let topModels = $derived(
    summary ? Object.entries(summary.window.models).sort((a, b) => b[1] - a[1]).slice(0, 6) : []
  );
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h3 class="text-xl font-bold text-cosmic-cyan">Usage</h3>
    <select
      bind:value={windowDays}
      onchange={refresh}
      class="bg-white/5 border border-white/10 rounded-lg px-3 py-1 text-sm"
      aria-label="Window"
    >
      <option value={7}>Last 7 days</option>
      <option value={30}>Last 30 days</option>
      <option value={90}>Last 90 days</option>
      <option value={366}>Last year</option>
    </select>
  </div>

  <p class="text-sm text-gray-400">
    Daily totals kept on this machine only: counts, tokens and model names. No prompts, replies or file names are
    recorded, and nothing is sent anywhere.
  </p>

  {#if error}
    <div class="text-sm text-red-300">{error}</div>
  {:else if summary}
    <div class="grid grid-cols-2 lg:grid-cols-4 gap-3">
      <div class="glass-panel p-3 bg-white/5">
        <div class="text-xs text-gray-400">Turns</div>
        <div class="text-2xl font-semibold">{fmt(summary.window.turns)}</div>
        <div class="text-xs text-gray-500">{fmt(summary.window.local_turns)} local · {fmt(summary.window.cloud_turns)} cloud</div>
      </div>
      <div class="glass-panel p-3 bg-white/5">
        <div class="text-xs text-gray-400">Tokens served locally</div>
        <div class="text-2xl font-semibold">{fmt(summary.window.local_prompt_tokens + summary.window.local_output_tokens)}</div>
        <div class="text-xs text-gray-500">{fmt(summary.window.local_output_tokens)} generated</div>
      </div>
      <div class="glass-panel p-3 bg-white/5">
        <div class="text-xs text-gray-400">Cloud-equivalent cost avoided</div>
        <div class="text-2xl font-semibold text-green-400">${saved.toFixed(2)}</div>
        <div class="text-xs text-gray-500">at the reference rates below</div>
      </div>
      <div class="glass-panel p-3 bg-white/5">
        <div class="text-xs text-gray-400">Agent work</div>
        <div class="text-2xl font-semibold">{fmt(summary.window.tool_calls)}</div>
        <div class="text-xs text-gray-500">tool calls · {hours(summary.window.active_ms)} h generating</div>
      </div>
    </div>

    {#if summary.days.length}
      <div class="glass-panel p-4 bg-white/5">
        <div class="text-xs text-gray-400 mb-2">Turns per day</div>
        <div class="flex items-end gap-1 h-24" role="img" aria-label="Turns per day">
          {#each summary.days as d (d.date)}
            <div
              class="flex-1 min-w-[3px] rounded-t bg-cosmic-cyan/70"
              style="height: {Math.max(4, (d.turns / peak) * 100)}%"
              title="{d.date}: {d.turns} turns, {fmt(d.output_tokens)} output tokens"
            ></div>
          {/each}
        </div>
      </div>
    {:else}
      <div class="glass-panel p-4 bg-white/5 text-sm text-gray-400">No activity recorded in this window yet.</div>
    {/if}

    {#if topModels.length}
      <div class="glass-panel p-4 bg-white/5">
        <div class="text-xs text-gray-400 mb-2">Models</div>
        {#each topModels as [m, n] (m)}
          <div class="flex justify-between text-sm"><span class="font-mono">{m}</span><span>{fmt(n)} turns</span></div>
        {/each}
      </div>
    {/if}

    <div class="grid grid-cols-2 gap-4">
      <label class="text-sm">
        Reference input price ($ / 1M tokens)
        <input type="number" min="0" step="0.1" bind:value={rates.input} onchange={saveRates} class="w-full mt-1 bg-white/5 border border-white/10 rounded-lg px-3 py-2" />
      </label>
      <label class="text-sm">
        Reference output price ($ / 1M tokens)
        <input type="number" min="0" step="0.1" bind:value={rates.output} onchange={saveRates} class="w-full mt-1 bg-white/5 border border-white/10 rounded-lg px-3 py-2" />
      </label>
    </div>

    <div class="flex items-center justify-between text-xs text-gray-500">
      <span>
        All time: {fmt(summary.all_time.turns)} turns{summary.since ? ` since ${summary.since}` : ''}
      </span>
      <button onclick={clear} class="px-3 py-1 rounded-lg {confirmClear ? 'bg-red-500/40 text-white' : 'bg-white/10 hover:bg-white/20'}">
        {confirmClear ? 'Click again to delete the usage history' : 'Clear usage history'}
      </button>
    </div>
  {/if}
</div>
