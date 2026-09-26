<script lang="ts">
  import { onMount } from 'svelte';
  import { call, errorMessage } from '$lib/api';
  import { pct } from '$lib/format';
  import type { DockerStatus, Service } from '$lib/types';

  let { onError, onInfo }: { onError: (msg: string) => void; onInfo: (msg: string) => void } = $props();

  let view = $state<'services' | 'docker'>('services');
  let services = $state<Service[]>([]);
  let docker = $state<DockerStatus | null>(null);
  let loading = $state(false);
  let filter = $state('');
  let scope = $state<'all' | 'system' | 'user'>('all');
  let stateFilter = $state<'running' | 'failed' | 'all'>('running');
  let busy = $state('');
  let logs = $state<{ title: string; text: string } | null>(null);

  async function load() {
    loading = true;
    try {
      [services, docker] = await Promise.all([call<Service[]>('list_services'), call<DockerStatus>('list_containers')]);
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      loading = false;
    }
  }
  onMount(load);

  async function serviceAction(s: Service, action: string) {
    busy = `${s.scope}/${s.unit}`;
    try {
      const r = await call<{ exit_code: number | null; output: string }>('toggle_service', { scope: s.scope, unit: s.unit, action });
      if (r.exit_code === 0) onInfo(`${action} ${s.unit}: done`);
      else onError(`${action} ${s.unit} exited ${r.exit_code}: ${r.output.trim().slice(0, 200)}`);
      await load();
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      busy = '';
    }
  }

  async function serviceLogs(s: Service) {
    try {
      logs = { title: `${s.unit} (${s.scope})`, text: await call<string>('service_logs', { scope: s.scope, unit: s.unit, lines: 300 }) };
    } catch (e) {
      onError(errorMessage(e));
    }
  }

  async function containerAction(name: string, action: string) {
    busy = name;
    try {
      const r = await call<{ exit_code: number | null; output: string }>('control_container', { name, action });
      if (r.exit_code === 0) onInfo(`${action} ${name}: done`);
      else onError(`${action} ${name} exited ${r.exit_code}: ${r.output.trim().slice(0, 200)}`);
      await load();
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      busy = '';
    }
  }

  async function containerLogs(name: string) {
    try {
      logs = { title: `container ${name}`, text: await call<string>('container_logs', { name, lines: 300 }) };
    } catch (e) {
      onError(errorMessage(e));
    }
  }

  let shown = $derived(
    services.filter(
      (s) =>
        (scope === 'all' || s.scope === scope) &&
        (stateFilter === 'all' || (stateFilter === 'failed' ? s.active === 'failed' : s.active === 'active' || s.active === 'failed')) &&
        (!filter || `${s.unit} ${s.description}`.toLowerCase().includes(filter.toLowerCase()))
    )
  );
  let failed = $derived(services.filter((s) => s.active === 'failed').length);

  function dot(s: Service) {
    if (s.active === 'failed') return 'bg-red-400';
    if (s.active === 'active') return s.sub === 'running' ? 'bg-green-400' : 'bg-cosmic-cyan';
    return 'bg-gray-500';
  }
</script>

<div class="space-y-4">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div class="flex gap-2">
      <button onclick={() => (view = 'services')} class="px-3 py-1.5 rounded-lg text-sm {view === 'services' ? 'bg-cosmic-blue/30 text-cosmic-cyan' : 'bg-white/5'}">
        Services {#if failed}<span class="ml-1 text-xs bg-red-500/30 text-red-300 px-1.5 rounded">{failed} failed</span>{/if}
      </button>
      <button onclick={() => (view = 'docker')} class="px-3 py-1.5 rounded-lg text-sm {view === 'docker' ? 'bg-cosmic-blue/30 text-cosmic-cyan' : 'bg-white/5'}">
        Docker {#if docker?.available}<span class="ml-1 text-xs text-gray-400">{docker.containers.filter((c) => c.state === 'running').length}/{docker.containers.length}</span>{/if}
      </button>
    </div>
    <button onclick={load} disabled={loading} class="glass-panel px-3 py-1.5 text-sm hover:bg-white/10 disabled:opacity-50">{loading ? 'Loading…' : '↻ Refresh'}</button>
  </div>

  {#if view === 'services'}
    <div class="flex flex-wrap gap-2">
      <input id="svc-filter" bind:value={filter} placeholder="Filter services" class="flex-1 min-w-[12rem] bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
      <select id="svc-scope" bind:value={scope} class="bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm">
        <option value="all">System + user</option><option value="system">System</option><option value="user">User</option>
      </select>
      <select id="svc-state" bind:value={stateFilter} class="bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm">
        <option value="running">Active + failed</option><option value="failed">Failed only</option><option value="all">All</option>
      </select>
    </div>
    <p class="text-xs text-gray-400">Every change opens a confirmation dialog. System services also ask for your password through your desktop's normal prompt.</p>
    <div class="space-y-1">
      {#each shown as s (s.scope + s.unit)}
        <div class="flex items-center gap-3 py-2 px-2 rounded hover:bg-white/5 border-b border-white/5">
          <span class="w-2 h-2 rounded-full shrink-0 {dot(s)}" title="{s.active} ({s.sub})"></span>
          <div class="min-w-0 flex-1">
            <div class="text-sm font-medium truncate">{s.unit} <span class="text-xs text-gray-500">{s.scope}</span></div>
            <div class="text-xs text-gray-400 truncate">{s.description} · {s.active}/{s.sub}{#if s.enabled} · {s.enabled}{/if}</div>
          </div>
          <div class="flex gap-1 shrink-0 text-xs">
            {#if s.active === 'active'}
              <button disabled={busy !== ''} onclick={() => serviceAction(s, 'restart')} class="px-2 py-1 rounded bg-white/5 hover:bg-white/10 disabled:opacity-40">Restart</button>
              <button disabled={busy !== ''} onclick={() => serviceAction(s, 'stop')} class="px-2 py-1 rounded bg-white/5 hover:bg-red-500/20 disabled:opacity-40">Stop</button>
            {:else}
              <button disabled={busy !== ''} onclick={() => serviceAction(s, 'start')} class="px-2 py-1 rounded bg-white/5 hover:bg-green-500/20 disabled:opacity-40">Start</button>
            {/if}
            <button onclick={() => serviceLogs(s)} class="px-2 py-1 rounded bg-white/5 hover:bg-white/10">Logs</button>
          </div>
        </div>
      {/each}
      {#if !loading && shown.length === 0}<p class="text-sm text-gray-400">No services match.</p>{/if}
    </div>
  {:else if docker && !docker.available}
    <div class="glass-panel p-4 bg-yellow-500/10 border border-yellow-500/30 text-sm">{docker.reason}</div>
  {:else if docker}
    <div class="space-y-1">
      {#each docker.containers as c (c.id)}
        <div class="flex items-center gap-3 py-2 px-2 rounded hover:bg-white/5 border-b border-white/5">
          <span class="w-2 h-2 rounded-full shrink-0 {c.state === 'running' ? 'bg-green-400' : c.state === 'exited' ? 'bg-gray-500' : 'bg-yellow-400'}"></span>
          <div class="min-w-0 flex-1">
            <div class="text-sm font-medium truncate">{c.name}{#if c.project}<span class="text-xs text-gray-500 ml-2">{c.project}</span>{/if}</div>
            <div class="text-xs text-gray-400 truncate">{c.image} · {c.status}{#if c.ports} · {c.ports}{/if}</div>
          </div>
          {#if c.state === 'running'}
            <div class="text-xs text-gray-300 tabular-nums shrink-0 text-right hidden md:block">CPU {pct(c.cpu, 1)}<br />{c.memory ?? ''}</div>
          {/if}
          <div class="flex gap-1 shrink-0 text-xs">
            {#if c.state === 'running'}
              <button disabled={busy !== ''} onclick={() => containerAction(c.name, 'restart')} class="px-2 py-1 rounded bg-white/5 hover:bg-white/10 disabled:opacity-40">Restart</button>
              <button disabled={busy !== ''} onclick={() => containerAction(c.name, 'stop')} class="px-2 py-1 rounded bg-white/5 hover:bg-red-500/20 disabled:opacity-40">Stop</button>
            {:else if c.state === 'paused'}
              <button disabled={busy !== ''} onclick={() => containerAction(c.name, 'unpause')} class="px-2 py-1 rounded bg-white/5 disabled:opacity-40">Resume</button>
            {:else}
              <button disabled={busy !== ''} onclick={() => containerAction(c.name, 'start')} class="px-2 py-1 rounded bg-white/5 hover:bg-green-500/20 disabled:opacity-40">Start</button>
            {/if}
            <button onclick={() => containerLogs(c.name)} class="px-2 py-1 rounded bg-white/5 hover:bg-white/10">Logs</button>
          </div>
        </div>
      {/each}
      {#if docker.containers.length === 0}<p class="text-sm text-gray-400">No containers.</p>{/if}
    </div>
  {/if}

  {#if logs}
    <div class="glass-panel p-4 bg-black/40 space-y-2">
      <div class="flex justify-between items-center">
        <h4 class="font-bold text-sm">Logs: {logs.title}</h4>
        <button onclick={() => (logs = null)} class="text-sm text-gray-400 hover:text-white">Close</button>
      </div>
      <pre class="text-xs font-mono whitespace-pre-wrap break-all max-h-96 overflow-auto">{logs.text || '(no output)'}</pre>
    </div>
  {/if}
</div>
