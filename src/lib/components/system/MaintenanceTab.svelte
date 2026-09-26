<script lang="ts">
  import { onMount } from 'svelte';
  import { call, errorMessage } from '$lib/api';
  import { bytes } from '$lib/format';
  import type { CleanupItem, Recommendation } from '$lib/types';

  let { onError, onInfo }: { onError: (msg: string) => void; onInfo: (msg: string) => void } = $props();

  let items = $state<CleanupItem[] | null>(null);
  let selected = $state<Record<string, boolean>>({});
  let recs = $state<Recommendation[] | null>(null);
  let scanning = $state(false);
  let analysing = $state(false);
  let cleaning = $state(false);
  let applying = $state('');

  async function scan() {
    scanning = true;
    try {
      items = await call<CleanupItem[]>('scan_cleanup');
      selected = Object.fromEntries(items.filter((i) => i.kind === 'files' && i.id !== 'trash').map((i) => [i.id, true]));
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      scanning = false;
    }
  }

  async function analyse() {
    analysing = true;
    try {
      recs = await call<Recommendation[]>('optimize_system');
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      analysing = false;
    }
  }

  onMount(() => {
    analyse();
    scan();
  });

  async function clean() {
    const ids = Object.entries(selected).filter(([, v]) => v).map(([k]) => k);
    if (!ids.length) return;
    cleaning = true;
    try {
      const r = await call<{ id: string; ok: boolean; freed: number; message: string }[]>('run_system_cleanup', { ids });
      const freed = r.reduce((a, x) => a + x.freed, 0);
      const failed = r.filter((x) => !x.ok);
      if (failed.length) onError(`Some items failed: ${failed.map((f) => `${f.id}: ${f.message}`).join('; ')}`);
      onInfo(`Freed ${bytes(freed)}`);
      await scan();
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      cleaning = false;
    }
  }

  async function apply(r: Recommendation) {
    if (!r.action) return;
    if (r.action.kind === 'open_cleanup') {
      document.getElementById('cleanup-section')?.scrollIntoView({ behavior: 'smooth' });
      return;
    }
    applying = r.id;
    try {
      onInfo(await call<string>('apply_recommendation', { kind: r.action.kind, params: r.action.params }));
      await analyse();
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      applying = '';
    }
  }

  let total = $derived((items ?? []).filter((i) => selected[i.id]).reduce((a, i) => a + i.bytes, 0));
  const sev = { critical: 'border-red-500/50 bg-red-500/10', warning: 'border-yellow-500/40 bg-yellow-500/10', info: 'border-white/10 bg-white/5' };
</script>

<div class="space-y-8">
  <section class="space-y-3">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h3 class="text-xl font-bold text-cosmic-cyan">Optimize</h3>
        <p class="text-sm text-gray-400">Findings from measured state: models spilling to CPU, idle models holding VRAM, failed services, full disks, heat, memory pressure, memory-store upkeep.</p>
      </div>
      <button onclick={analyse} disabled={analysing} class="glass-panel px-3 py-1.5 text-sm hover:bg-white/10 disabled:opacity-50">{analysing ? 'Analysing…' : '↻ Re-check'}</button>
    </div>
    {#if recs && recs.length === 0}
      <div class="glass-panel p-4 bg-green-500/10 border border-green-500/30 text-sm">✓ Nothing to fix right now.</div>
    {/if}
    {#each recs ?? [] as r (r.id)}
      <div class="glass-panel p-4 border {sev[r.severity]} flex flex-wrap items-start gap-3">
        <div class="min-w-0 flex-1">
          <div class="text-sm font-bold">{r.title} <span class="text-xs font-normal text-gray-400 ml-1">{r.area.replace('_', ' ')}</span></div>
          <div class="text-sm text-gray-300 mt-1">{r.detail}</div>
        </div>
        {#if r.action}
          <button disabled={applying !== ''} onclick={() => apply(r)} class="px-3 py-1.5 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg text-sm disabled:opacity-50 shrink-0">
            {applying === r.id ? 'Working…' : r.action.label}
          </button>
        {/if}
      </div>
    {/each}
  </section>

  <section id="cleanup-section" class="space-y-3">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h3 class="text-xl font-bold text-cosmic-cyan">Cleanup</h3>
        <p class="text-sm text-gray-400">Only caches that rebuild themselves, the Trash, unused Docker layers and old journal entries. You confirm the list before anything is deleted.</p>
      </div>
      <button onclick={scan} disabled={scanning} class="glass-panel px-3 py-1.5 text-sm hover:bg-white/10 disabled:opacity-50">{scanning ? 'Scanning…' : '↻ Scan'}</button>
    </div>
    {#if items && items.length === 0}<p class="text-sm text-gray-400">Nothing to clean.</p>{/if}
    <div class="space-y-1">
      {#each items ?? [] as i (i.id)}
        <label class="flex items-center gap-3 py-2 px-2 rounded hover:bg-white/5 border-b border-white/5 cursor-pointer">
          <input type="checkbox" bind:checked={selected[i.id]} />
          <div class="min-w-0 flex-1">
            <div class="text-sm font-medium">{i.label}</div>
            <div class="text-xs text-gray-400">{i.description}{#if i.command} · runs <code>{i.command}</code> (asks first){/if}</div>
          </div>
          <span class="text-sm tabular-nums shrink-0">{i.partial ? 'up to ' : ''}{bytes(i.bytes)}</span>
        </label>
      {/each}
    </div>
    {#if items && items.length}
      <button onclick={clean} disabled={cleaning || total === 0} class="px-4 py-2 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg text-sm disabled:opacity-50">
        {cleaning ? 'Cleaning…' : `Clean ${bytes(total)}`}
      </button>
    {/if}
  </section>
</div>
