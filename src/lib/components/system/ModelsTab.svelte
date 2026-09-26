<script lang="ts">
  import { onMount } from 'svelte';
  import { Channel } from '@tauri-apps/api/core';
  import { call, errorMessage } from '$lib/api';
  import { ago, bytes } from '$lib/format';
  import type { InstalledModel, LoadedModel, Settings } from '$lib/types';

  let { onError, onInfo }: { onError: (msg: string) => void; onInfo: (msg: string) => void } = $props();

  let installed = $state<InstalledModel[]>([]);
  let loaded = $state<LoadedModel[]>([]);
  let configured = $state('');
  let busy = $state('');
  let pullName = $state('');
  let pull = $state<{ name: string; status: string; completed: number | null; total: number | null } | null>(null);

  async function load() {
    try {
      const r = await call<{ installed: InstalledModel[]; loaded: LoadedModel[]; configured: string }>('list_models_detail');
      installed = r.installed;
      loaded = r.loaded;
      configured = r.configured;
    } catch (e) {
      onError(errorMessage(e));
    }
  }
  onMount(load);

  async function act(cmd: string, model: string, done: string) {
    busy = model;
    try {
      await call(cmd, { model });
      onInfo(done);
      await load();
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      busy = '';
    }
  }

  async function useForChat(model: string) {
    busy = model;
    try {
      const s = await call<Settings>('load_settings');
      s.ai.provider = 'ollama';
      s.ai.ollama_model = model;
      await call('save_settings', { settings: s });
      onInfo(`OMNIX now chats with ${model}`);
      await load();
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      busy = '';
    }
  }

  async function startPull() {
    const name = pullName.trim();
    if (!name) return;
    const ch = new Channel<{ status: string; completed: number | null; total: number | null }>();
    pull = { name, status: 'waiting for approval', completed: null, total: null };
    ch.onmessage = (p) => {
      pull = { name, ...p };
    };
    try {
      await call('model_pull', { model: name, onProgress: ch });
      onInfo(`Downloaded ${name}`);
      pullName = '';
      await load();
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      pull = null;
    }
  }

  let loadedByName = $derived(new Map(loaded.map((m) => [m.name, m])));
  let diskTotal = $derived(installed.reduce((a, m) => a + m.size, 0));
</script>

<div class="space-y-5">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <h3 class="text-xl font-bold text-cosmic-cyan">Models</h3>
    <button onclick={load} class="glass-panel px-3 py-1.5 text-sm hover:bg-white/10">↻ Refresh</button>
  </div>

  <div class="glass-panel p-4 bg-white/5 space-y-2">
    <label for="pull-name" class="block text-sm font-medium">Download a model from the Ollama library</label>
    <div class="flex gap-2">
      <input id="pull-name" bind:value={pullName} placeholder="e.g. llama3.2:3b or qwen3:14b" disabled={pull !== null}
        onkeydown={(e) => e.key === 'Enter' && startPull()}
        class="flex-1 bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
      <button onclick={startPull} disabled={!pullName.trim() || pull !== null}
        class="px-4 py-2 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg text-sm disabled:opacity-50">Download</button>
    </div>
    {#if pull}
      {@const p = pull.total ? ((pull.completed ?? 0) / pull.total) * 100 : null}
      <div class="text-xs text-gray-300">{pull.name}: {pull.status}{#if p != null} · {bytes(pull.completed)} / {bytes(pull.total)}{/if}</div>
      {#if p != null}
        <div class="w-full h-2 bg-white/10 rounded-full overflow-hidden"><div class="h-full bg-cosmic-cyan" style="width: {p}%"></div></div>
      {/if}
    {/if}
  </div>

  <div class="text-sm text-gray-400">{installed.length} installed · {bytes(diskTotal)} on disk · {loaded.length} in memory</div>
  <div class="space-y-2">
    {#each installed as m (m.name)}
      {@const l = loadedByName.get(m.name)}
      <div class="glass-panel p-3 bg-white/5 flex flex-wrap items-center gap-3">
        <div class="min-w-0 flex-1">
          <div class="font-medium text-sm truncate">
            {m.name}
            {#if m.name === configured}<span class="ml-2 text-xs bg-cosmic-blue/30 text-cosmic-cyan px-1.5 py-0.5 rounded">chat model</span>{/if}
            {#if l}<span class="ml-2 text-xs bg-green-500/20 text-green-300 px-1.5 py-0.5 rounded">loaded · {l.gpu_percent.toFixed(0)}% GPU</span>{/if}
          </div>
          <div class="text-xs text-gray-400">
            {bytes(m.size)}{#if m.parameter_size} · {m.parameter_size}{/if}{#if m.quantization} · {m.quantization}{/if}{#if m.family} · {m.family}{/if}
            {#if l} · in memory {bytes(l.size)} ({bytes(l.size_vram)} VRAM) · unloads {ago(l.expires_at)}{/if}
          </div>
        </div>
        <div class="flex gap-1 text-xs shrink-0">
          {#if !m.name.includes('embed') && m.name !== configured}
            <button disabled={busy !== ''} onclick={() => useForChat(m.name)} class="px-2 py-1 rounded bg-white/5 hover:bg-white/10 disabled:opacity-40">Use for chat</button>
          {/if}
          {#if l}
            <button disabled={busy !== ''} onclick={() => act('model_unload', m.name, `Unloaded ${m.name}`)} class="px-2 py-1 rounded bg-white/5 hover:bg-white/10 disabled:opacity-40">Unload</button>
          {:else}
            <button disabled={busy !== ''} onclick={() => act('model_load', m.name, `Loaded ${m.name}`)} class="px-2 py-1 rounded bg-white/5 hover:bg-white/10 disabled:opacity-40">{busy === m.name ? 'Loading…' : 'Load'}</button>
          {/if}
          <button disabled={busy !== ''} onclick={() => act('model_delete', m.name, `Deleted ${m.name}`)} class="px-2 py-1 rounded bg-white/5 hover:bg-red-500/20 text-red-300 disabled:opacity-40">Delete</button>
        </div>
      </div>
    {/each}
  </div>
</div>
