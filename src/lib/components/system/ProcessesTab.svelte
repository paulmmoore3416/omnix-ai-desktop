<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { call, errorMessage } from '$lib/api';
  import { loadColor } from '$lib/format';
  import type { ProcessInfo } from '$lib/types';

  let { onError }: { onError: (msg: string) => void } = $props();

  let processes = $state<ProcessInfo[]>([]);
  let filter = $state('');
  let sortBy = $state<'cpu' | 'memory' | 'name'>('cpu');
  let timer: ReturnType<typeof setInterval> | undefined;
  let busy = false;

  async function refresh() {
    if (busy) return;
    busy = true;
    try {
      processes = await call<ProcessInfo[]>('get_processes');
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      busy = false;
    }
  }

  async function kill(pid: number) {
    // The backend raises a native confirmation dialog.
    try {
      await call('kill_process', { pid });
      await refresh();
    } catch (e) {
      onError(errorMessage(e));
    }
  }

  onMount(() => {
    refresh();
    timer = setInterval(refresh, 3000);
  });
  onDestroy(() => clearInterval(timer));

  let rows = $derived(
    processes
      .filter((p) => p.name.toLowerCase().includes(filter.toLowerCase()) || String(p.pid) === filter.trim())
      .sort((a, b) => (sortBy === 'cpu' ? b.cpu - a.cpu : sortBy === 'memory' ? b.memory - a.memory : a.name.localeCompare(b.name)))
  );
</script>

<div class="space-y-4">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <h3 class="text-xl font-bold text-cosmic-cyan">Processes</h3>
    <div class="flex gap-2">
      <input id="proc-filter" bind:value={filter} placeholder="Filter by name or PID" class="bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
      <select id="proc-sort" bind:value={sortBy} class="bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm">
        <option value="cpu">Sort by CPU</option>
        <option value="memory">Sort by memory</option>
        <option value="name">Sort by name</option>
      </select>
    </div>
  </div>
  <table class="w-full text-sm tabular-nums">
    <thead>
      <tr class="text-left text-xs text-gray-400 border-b border-white/10">
        <th class="py-2 font-normal">PID</th><th class="font-normal">Name</th><th class="font-normal text-right">CPU</th><th class="font-normal text-right">Memory</th><th class="font-normal">Status</th><th></th>
      </tr>
    </thead>
    <tbody>
      {#each rows as p (p.pid)}
        <tr class="border-b border-white/5 hover:bg-white/5">
          <td class="py-1.5 text-gray-400">{p.pid}</td>
          <td class="truncate max-w-[16rem]" title={p.name}>{p.name}</td>
          <td class="text-right {loadColor(p.cpu)}">{p.cpu.toFixed(1)}%</td>
          <td class="text-right">{p.memory.toFixed(1)}%</td>
          <td class="text-gray-400">{p.status}</td>
          <td class="text-right"><button onclick={() => kill(p.pid)} class="text-xs text-red-400 hover:text-red-300 px-2">End</button></td>
        </tr>
      {/each}
    </tbody>
  </table>
  {#if rows.length === 0}<p class="text-sm text-gray-400">No matching processes.</p>{/if}
</div>
