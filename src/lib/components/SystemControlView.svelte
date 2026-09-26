<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { call, errorMessage } from '$lib/api';
  import type { OpsEvent, PerformanceData, SystemControlData, SystemInfo } from '$lib/types';
  import OverviewTab from './system/OverviewTab.svelte';
  import PerformanceTab from './system/PerformanceTab.svelte';
  import ProcessesTab from './system/ProcessesTab.svelte';
  import ServicesTab from './system/ServicesTab.svelte';
  import ModelsTab from './system/ModelsTab.svelte';
  import RulesTab from './system/RulesTab.svelte';
  import MaintenanceTab from './system/MaintenanceTab.svelte';

  let activeTab = $state('overview');
  let lastError = $state('');
  let info = $state('');
  let systemInfo = $state<SystemInfo | null>(null);
  let perf = $state<PerformanceData | null>(null);
  let ops = $state<SystemControlData | null>(null);

  const tabs = [
    { id: 'overview', label: 'Overview', icon: '📊' },
    { id: 'performance', label: 'Performance', icon: '📈' },
    { id: 'processes', label: 'Processes', icon: '⚙️' },
    { id: 'services', label: 'Services', icon: '🔧' },
    { id: 'models', label: 'Models', icon: '🧬' },
    { id: 'alerts', label: 'Alerts', icon: '🚨' },
    { id: 'automation', label: 'Automation', icon: '🤖' },
    { id: 'scheduler', label: 'Scheduler', icon: '⏰' },
    { id: 'maintenance', label: 'Cleanup & Optimize', icon: '🧹' }
  ];

  function onError(msg: string) {
    info = '';
    lastError = msg;
  }
  function onInfo(msg: string) {
    lastError = '';
    info = msg;
  }

  let perfBusy = false;
  async function loadPerf() {
    if (perfBusy) return;
    perfBusy = true;
    try {
      perf = await call<PerformanceData>('get_performance', { historySecs: 900 });
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      perfBusy = false;
    }
  }

  async function loadOps() {
    try {
      ops = await call<SystemControlData>('get_system_control_data');
    } catch (e) {
      onError(errorMessage(e));
    }
  }

  let timer: ReturnType<typeof setInterval> | undefined;
  let unlisten: UnlistenFn | undefined;
  onMount(async () => {
    try {
      systemInfo = await call<SystemInfo>('get_system_info');
    } catch (e) {
      onError(errorMessage(e));
    }
    loadPerf();
    loadOps();
    timer = setInterval(() => {
      if (activeTab === 'overview' || activeTab === 'performance') loadPerf();
      if (['alerts', 'automation', 'scheduler', 'overview'].includes(activeTab)) loadOps();
    }, 3000);
    try {
      unlisten = await listen<OpsEvent>('ops://event', () => loadOps());
    } catch {
      /* not running under Tauri (tests) */
    }
  });
  onDestroy(() => {
    clearInterval(timer);
    unlisten?.();
  });

  let firing = $derived((ops?.alerts ?? []).filter((a) => a.alert.enabled && a.alert.state.firing).length);
</script>

<div class="h-full flex flex-col">
  <div class="flex items-center justify-between mb-6">
    <div>
      <h2 class="text-3xl font-bold glow-text">System Control</h2>
      <p class="text-gray-400 mt-1">Monitor, automate and maintain this computer</p>
    </div>
    {#if firing}
      <button onclick={() => (activeTab = 'alerts')} class="glass-panel px-3 py-2 text-sm bg-red-500/20 border border-red-500/40 text-red-200">
        ⚠ {firing} alert{firing === 1 ? '' : 's'} firing
      </button>
    {/if}
  </div>

  {#if info}
    <div class="glass-panel p-3 mb-4 bg-green-500/20 text-sm flex justify-between gap-3" role="status">
      <span>{info}</span><button onclick={() => (info = '')} class="text-gray-300">✕</button>
    </div>
  {/if}
  {#if lastError}
    <div class="glass-panel p-3 mb-4 bg-red-500/20 text-sm flex justify-between gap-3" role="alert">
      <span>{lastError}</span><button onclick={() => (lastError = '')} class="text-gray-300">✕</button>
    </div>
  {/if}

  <div class="flex-1 flex gap-4 overflow-hidden">
    <div class="w-52 glass-panel p-4 space-y-2 shrink-0 overflow-auto">
      {#each tabs as tab}
        <button
          onclick={() => (activeTab = tab.id)}
          class="w-full text-left px-3 py-2 rounded-lg transition-all flex items-center gap-2
                 {activeTab === tab.id ? 'bg-cosmic-blue/20 text-cosmic-cyan border border-cosmic-blue/50' : 'hover:bg-white/5 text-gray-300'}"
        >
          <span class="text-lg">{tab.icon}</span>
          <span class="text-sm font-medium">{tab.label}</span>
          {#if tab.id === 'alerts' && firing}<span class="ml-auto text-xs bg-red-500/40 px-1.5 rounded">{firing}</span>{/if}
        </button>
      {/each}
    </div>

    <div class="flex-1 glass-panel p-6 overflow-auto">
      {#if activeTab === 'overview'}
        <OverviewTab {perf} info={systemInfo} />
      {:else if activeTab === 'performance'}
        <PerformanceTab {perf} />
      {:else if activeTab === 'processes'}
        <ProcessesTab {onError} />
      {:else if activeTab === 'services'}
        <ServicesTab {onError} {onInfo} />
      {:else if activeTab === 'models'}
        <ModelsTab {onError} {onInfo} />
      {:else if activeTab === 'alerts'}
        <RulesTab kind="alerts" data={ops} reload={loadOps} {onError} {onInfo} />
      {:else if activeTab === 'automation'}
        <RulesTab kind="automations" data={ops} reload={loadOps} {onError} {onInfo} />
      {:else if activeTab === 'scheduler'}
        <RulesTab kind="tasks" data={ops} reload={loadOps} {onError} {onInfo} />
      {:else if activeTab === 'maintenance'}
        <MaintenanceTab {onError} {onInfo} />
      {/if}
    </div>
  </div>
</div>
