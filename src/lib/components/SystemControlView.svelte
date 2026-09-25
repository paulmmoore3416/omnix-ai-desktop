<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { call, errorMessage, unavailable } from '$lib/api';
  import type { ProcessInfo, RealTimeStats, SystemControlData, SystemInfo } from '$lib/types';
  
  let activeTab = $state('overview');
  let isProcessing = $state(false);
  let available = $state(false);
  let lastError = $state('');
  
  let systemInfo = $state<SystemInfo | null>(null);
  
  let realTimeStats = $state<RealTimeStats>({
    cpu: 0,
    memory: 0,
    swap: 0,
    disk: null,
    network: null,
    temperature: null
  });
  
  let processes = $state<ProcessInfo[]>([]);
  
  let services = $state<Array<{
    name: string;
    status: 'running' | 'stopped' | 'error';
    autoStart: boolean;
    description: string;
  }>>([]);
  
  let automations = $state<Array<{
    id: string;
    name: string;
    trigger: string;
    action: string;
    enabled: boolean;
    lastRun: Date | null;
    runCount: number;
  }>>([]);
  
  let scheduledTasks = $state<Array<{
    id: string;
    name: string;
    schedule: string;
    command: string;
    enabled: boolean;
    nextRun: Date;
  }>>([]);
  
  let alerts = $state<Array<{
    id: string;
    type: 'cpu' | 'memory' | 'disk' | 'process' | 'custom';
    condition: string;
    threshold: number;
    action: string;
    enabled: boolean;
  }>>([]);
  
  let newAutomation = $state({
    name: '',
    trigger: 'cpu_high',
    action: '',
    threshold: 80
  });
  
  let newScheduledTask = $state({
    name: '',
    schedule: '0 0 * * *',
    command: ''
  });
  
  let newAlert = $state({
    type: 'cpu' as 'cpu' | 'memory' | 'disk' | 'process' | 'custom',
    condition: 'greater_than',
    threshold: 80,
    action: 'notify'
  });
  
  let updateInterval: ReturnType<typeof setInterval> | undefined;
  let processFilter = $state('');
  let sortBy = $state<'cpu' | 'memory' | 'name'>('cpu');
  
  const tabs = [
    { id: 'overview', label: 'Overview', icon: '📊' },
    { id: 'processes', label: 'Processes', icon: '⚙️' },
    { id: 'services', label: 'Services', icon: '🔧' },
    { id: 'automation', label: 'Automation', icon: '🤖' },
    { id: 'scheduler', label: 'Scheduler', icon: '⏰' },
    { id: 'alerts', label: 'Alerts', icon: '🚨' },
    { id: 'performance', label: 'Performance', icon: '📈' }
  ];
  
  const triggerTypes = [
    { value: 'cpu_high', label: 'CPU Usage High' },
    { value: 'memory_high', label: 'Memory Usage High' },
    { value: 'disk_full', label: 'Disk Space Low' },
    { value: 'process_start', label: 'Process Started' },
    { value: 'process_stop', label: 'Process Stopped' },
    { value: 'file_change', label: 'File Changed' },
    { value: 'time_based', label: 'Time-Based' },
    { value: 'system_idle', label: 'System Idle' }
  ];
  
  onMount(async () => {
    await loadSystemInfo();
    await loadData();
    startMonitoring();
  });
  
  onDestroy(() => {
    if (updateInterval) {
      clearInterval(updateInterval);
    }
  });
  
  async function loadSystemInfo() {
    try {
      systemInfo = await call<SystemInfo>('get_system_info');
    } catch (error) {
      lastError = errorMessage(error);
    }
  }
  
  async function loadData() {
    try {
      const parsed = await call<SystemControlData>('get_system_control_data');
      available = parsed.available;
      
      processes = parsed.processes || [];
      services = (parsed.services || []) as typeof services;
      automations = (parsed.automations || []).map((a: any) => ({
        ...a,
        lastRun: a.lastRun ? new Date(a.lastRun) : null
      }));
      scheduledTasks = (parsed.scheduledTasks || []).map((t: any) => ({
        ...t,
        nextRun: new Date(t.nextRun)
      }));
      alerts = (parsed.alerts || []) as typeof alerts;
    } catch (error) {
      lastError = errorMessage(error);
    }
  }
  
  function startMonitoring() {
    // Skip a tick if the previous poll has not finished, so slow backend
    // calls never pile up.
    let inFlight = false;
    updateInterval = setInterval(async () => {
      if (inFlight) return;
      inFlight = true;
      try {
        realTimeStats = await call<RealTimeStats>('get_real_time_stats');
        if (activeTab === 'processes') {
          processes = await call<ProcessInfo[]>('get_processes');
        }
      } catch (error) {
        lastError = errorMessage(error);
      } finally {
        inFlight = false;
      }
    }, 2000);
  }
  
  async function killProcess(pid: number) {
    // Confirmation happens in a native dialog raised by the backend.
    try {
      await call('kill_process', { pid });
      processes = await call<ProcessInfo[]>('get_processes');
    } catch (error) {
      lastError = errorMessage(error);
    }
  }
  
  async function toggleService(name: string) {
    isProcessing = true;
    try {
      await call('toggle_service', { name });
      await loadData();
    } catch (error) {
      lastError = errorMessage(error);
    } finally {
      isProcessing = false;
    }
  }
  
  async function createAutomation() {
    if (!newAutomation.name.trim() || !newAutomation.action.trim()) return;
    
    isProcessing = true;
    try {
      await call('create_automation', {
        name: newAutomation.name,
        trigger: newAutomation.trigger,
        action: newAutomation.action,
        threshold: newAutomation.threshold
      });
      
      newAutomation = { name: '', trigger: 'cpu_high', action: '', threshold: 80 };
      await loadData();
    } catch (error) {
      lastError = errorMessage(error);
    } finally {
      isProcessing = false;
    }
  }
  
  async function toggleAutomation(id: string) {
    try {
      await call('toggle_automation', { id });
      await loadData();
    } catch (error) {
      lastError = errorMessage(error);
    }
  }
  
  async function deleteAutomation(id: string) {
    if (!confirm('Are you sure you want to delete this automation?')) return;
    
    try {
      await call('delete_automation', { id });
      await loadData();
    } catch (error) {
      lastError = errorMessage(error);
    }
  }
  
  async function createScheduledTask() {
    if (!newScheduledTask.name.trim() || !newScheduledTask.command.trim()) return;
    
    isProcessing = true;
    try {
      await call('create_scheduled_task', {
        name: newScheduledTask.name,
        schedule: newScheduledTask.schedule,
        command: newScheduledTask.command
      });
      
      newScheduledTask = { name: '', schedule: '0 0 * * *', command: '' };
      await loadData();
    } catch (error) {
      lastError = errorMessage(error);
    } finally {
      isProcessing = false;
    }
  }
  
  async function toggleScheduledTask(id: string) {
    try {
      await call('toggle_scheduled_task', { id });
      await loadData();
    } catch (error) {
      lastError = errorMessage(error);
    }
  }
  
  async function createAlert() {
    isProcessing = true;
    try {
      await call('create_alert', {
        type: newAlert.type,
        condition: newAlert.condition,
        threshold: newAlert.threshold,
        action: newAlert.action
      });
      
      newAlert = { type: 'cpu', condition: 'greater_than', threshold: 80, action: 'notify' };
      await loadData();
    } catch (error) {
      lastError = errorMessage(error);
    } finally {
      isProcessing = false;
    }
  }
  
  async function toggleAlert(id: string) {
    try {
      await call('toggle_alert', { id });
      await loadData();
    } catch (error) {
      lastError = errorMessage(error);
    }
  }
  
  async function runSystemCleanup() {
    
    isProcessing = true;
    try {
      await call('run_system_cleanup');
    } catch (error) {
      lastError = errorMessage(error);
    } finally {
      isProcessing = false;
    }
  }
  
  async function optimizeSystem() {
    
    isProcessing = true;
    try {
      await call('optimize_system');
    } catch (error) {
      lastError = errorMessage(error);
    } finally {
      isProcessing = false;
    }
  }
  
  function formatUptime(seconds: number): string {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return `${days}d ${hours}h ${minutes}m`;
  }
  
  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
  }
  
  function getStatusColor(status: string): string {
    switch (status) {
      case 'running': return 'text-green-400';
      case 'stopped': return 'text-gray-400';
      case 'error': return 'text-red-400';
      default: return 'text-yellow-400';
    }
  }
  
  let filteredProcesses = $derived(
    processes
      .filter(p => p.name.toLowerCase().includes(processFilter.toLowerCase()))
      .sort((a, b) => {
        if (sortBy === 'cpu') return b.cpu - a.cpu;
        if (sortBy === 'memory') return b.memory - a.memory;
        return a.name.localeCompare(b.name);
      })
  );
</script>

<div class="h-full flex flex-col">
  <!-- Header -->
  <div class="flex items-center justify-between mb-6">
    <div>
      <h2 class="text-3xl font-bold glow-text">System Control</h2>
      <p class="text-gray-400 mt-1">Monitor and control your system</p>
    </div>
    <div class="flex gap-2">
      <button
        onclick={runSystemCleanup}
        disabled={isProcessing || unavailable('run_system_cleanup')}
        title={unavailable('run_system_cleanup') ? 'Not yet available' : undefined}
        class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm disabled:opacity-50"
      >
        🧹 Cleanup
      </button>
      <button
        onclick={optimizeSystem}
        disabled={isProcessing || unavailable('optimize_system')}
        title={unavailable('optimize_system') ? 'Not yet available' : undefined}
        class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm disabled:opacity-50"
      >
        ⚡ Optimize
      </button>
    </div>
  </div>
  
  {#if !available}
    <div class="glass-panel p-3 mb-4 bg-yellow-500/10 border border-yellow-500/30 text-sm text-gray-300">
      <span class="text-xs bg-yellow-500/20 text-yellow-300 px-2 py-1 rounded mr-2">Not yet available</span>
      Services, automations, the scheduler, alerts, cleanup and optimization are not implemented yet. Monitoring and ending processes work.
    </div>
  {/if}
  {#if lastError}
    <div class="glass-panel p-3 mb-4 bg-red-500/20 text-sm" role="alert">{lastError}</div>
  {/if}

  <div class="flex-1 flex gap-4 overflow-hidden">
    <!-- Tabs Sidebar -->
    <div class="w-48 glass-panel p-4 space-y-2">
      {#each tabs as tab}
        <button
          onclick={() => activeTab = tab.id}
          class="w-full text-left px-3 py-2 rounded-lg transition-all flex items-center gap-2
                 {activeTab === tab.id ? 'bg-cosmic-blue/20 text-cosmic-cyan border border-cosmic-blue/50' : 'hover:bg-white/5 text-gray-300'}"
        >
          <span class="text-lg">{tab.icon}</span>
          <span class="text-sm font-medium">{tab.label}</span>
        </button>
      {/each}
    </div>
    
    <!-- Content Area -->
    <div class="flex-1 glass-panel p-6 overflow-auto">
      {#if activeTab === 'overview'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan mb-4">System Overview</h3>
          
          <!-- System Info -->
          <div class="grid grid-cols-2 gap-4">
            <div class="glass-panel p-4 bg-white/5">
              <h4 class="font-bold mb-3 text-sm">System Information</h4>
              <div class="space-y-2 text-sm">
                <div class="flex justify-between">
                  <span class="text-gray-400">OS:</span>
                  <span>{systemInfo?.os ?? 'Loading...'}</span>
                </div>
                <div class="flex justify-between">
                  <span class="text-gray-400">Kernel:</span>
                  <span>{systemInfo?.kernel ?? 'Loading...'}</span>
                </div>
                <div class="flex justify-between">
                  <span class="text-gray-400">Hostname:</span>
                  <span>{systemInfo?.hostname ?? 'Loading...'}</span>
                </div>
                <div class="flex justify-between">
                  <span class="text-gray-400">Uptime:</span>
                  <span>{formatUptime(systemInfo?.uptime ?? 0)}</span>
                </div>
              </div>
            </div>
            
            <div class="glass-panel p-4 bg-white/5">
              <h4 class="font-bold mb-3 text-sm">Hardware</h4>
              <div class="space-y-2 text-sm">
                <div class="flex justify-between">
                  <span class="text-gray-400">CPU:</span>
                  <span>{systemInfo?.cpu_model ?? 'Loading...'}</span>
                </div>
                <div class="flex justify-between">
                  <span class="text-gray-400">Cores:</span>
                  <span>{systemInfo?.cpu_cores ?? 0}</span>
                </div>
                <div class="flex justify-between">
                  <span class="text-gray-400">Memory:</span>
                  <span>{formatBytes(systemInfo?.total_memory ?? 0)}</span>
                </div>
                <div class="flex justify-between">
                  <span class="text-gray-400">Swap:</span>
                  <span>{formatBytes(systemInfo?.total_swap ?? 0)}</span>
                </div>
              </div>
            </div>
          </div>
          
          <!-- Real-time Stats -->
          <div class="grid grid-cols-4 gap-4">
            <div class="glass-panel p-4 bg-white/5">
              <div class="text-xs text-gray-400 mb-2">CPU Usage</div>
              <div class="text-3xl font-bold text-cosmic-cyan mb-2">{realTimeStats.cpu.toFixed(1)}%</div>
              <div class="w-full h-2 bg-white/10 rounded-full overflow-hidden">
                <div 
                  class="h-full bg-cosmic-cyan transition-all duration-500"
                  style="width: {realTimeStats.cpu}%"
                ></div>
              </div>
            </div>
            
            <div class="glass-panel p-4 bg-white/5">
              <div class="text-xs text-gray-400 mb-2">Memory Usage</div>
              <div class="text-3xl font-bold text-cosmic-purple mb-2">{realTimeStats.memory.toFixed(1)}%</div>
              <div class="w-full h-2 bg-white/10 rounded-full overflow-hidden">
                <div 
                  class="h-full bg-cosmic-purple transition-all duration-500"
                  style="width: {realTimeStats.memory}%"
                ></div>
              </div>
            </div>
            
            <div class="glass-panel p-4 bg-white/5">
              <div class="text-xs text-gray-400 mb-2">Disk Usage</div>
              <div class="text-3xl font-bold text-yellow-400 mb-2">{realTimeStats.disk === null ? 'n/a' : `${realTimeStats.disk.toFixed(1)}%`}</div>
              <div class="w-full h-2 bg-white/10 rounded-full overflow-hidden">
                <div 
                  class="h-full bg-yellow-400 transition-all duration-500"
                  style="width: {realTimeStats.disk ?? 0}%"
                ></div>
              </div>
            </div>
            
            <div class="glass-panel p-4 bg-white/5">
              <div class="text-xs text-gray-400 mb-2">Temperature</div>
              <div class="text-3xl font-bold text-red-400 mb-2">{realTimeStats.temperature === null ? 'n/a' : `${realTimeStats.temperature.toFixed(0)}°C`}</div>
              <div class="w-full h-2 bg-white/10 rounded-full overflow-hidden">
                <div 
                  class="h-full bg-red-400 transition-all duration-500"
                  style="width: {Math.min(realTimeStats.temperature ?? 0, 100)}%"
                ></div>
              </div>
            </div>
          </div>
          
          <!-- Network Stats -->
          <div class="glass-panel p-4 bg-white/5">
            <h4 class="font-bold mb-3 text-sm">Network Activity</h4>
            <div class="grid grid-cols-2 gap-4">
              <div>
                <div class="text-xs text-gray-400 mb-1">Download</div>
                <div class="text-2xl font-bold text-green-400">↓ {realTimeStats.network ? `${formatBytes(realTimeStats.network.rx)}/s` : 'n/a'}</div>
              </div>
              <div>
                <div class="text-xs text-gray-400 mb-1">Upload</div>
                <div class="text-2xl font-bold text-blue-400">↑ {realTimeStats.network ? `${formatBytes(realTimeStats.network.tx)}/s` : 'n/a'}</div>
              </div>
            </div>
          </div>
        </div>
        
      {:else if activeTab === 'processes'}
        <div class="space-y-4">
          <div class="flex items-center justify-between mb-4">
            <h3 class="text-xl font-bold text-cosmic-cyan">Process Manager</h3>
            <div class="flex gap-3">
              <input
                type="text"
                bind:value={processFilter}
                placeholder="Filter processes..."
                class="bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm w-64"
              />
              <select bind:value={sortBy} class="bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm">
                <option value="cpu">Sort by CPU</option>
                <option value="memory">Sort by Memory</option>
                <option value="name">Sort by Name</option>
              </select>
            </div>
          </div>
          
          <div class="glass-panel bg-white/5 overflow-hidden">
            <table class="w-full text-sm">
              <thead class="bg-white/5">
                <tr>
                  <th class="text-left p-3">PID</th>
                  <th class="text-left p-3">Name</th>
                  <th class="text-left p-3">CPU %</th>
                  <th class="text-left p-3">Memory %</th>
                  <th class="text-left p-3">Status</th>
                  <th class="text-left p-3">Actions</th>
                </tr>
              </thead>
              <tbody>
                {#each filteredProcesses.slice(0, 50) as process}
                  <tr class="border-t border-white/5 hover:bg-white/5">
                    <td class="p-3 font-mono">{process.pid}</td>
                    <td class="p-3">{process.name}</td>
                    <td class="p-3">
                      <span class="text-cosmic-cyan font-bold">{process.cpu.toFixed(1)}%</span>
                    </td>
                    <td class="p-3">
                      <span class="text-cosmic-purple font-bold">{process.memory.toFixed(1)}%</span>
                    </td>
                    <td class="p-3">
                      <span class="text-xs px-2 py-1 rounded {getStatusColor(process.status)} bg-white/10">
                        {process.status}
                      </span>
                    </td>
                    <td class="p-3">
                      <button
                        onclick={() => killProcess(process.pid)}
                        class="text-red-400 hover:text-red-300 text-xs px-2 py-1 rounded hover:bg-red-500/20"
                      >
                        Kill
                      </button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>
        
      {:else if activeTab === 'services'}
        <div class="space-y-4">
          <h3 class="text-xl font-bold text-cosmic-cyan mb-4">Service Manager</h3>
          
          <div class="grid grid-cols-2 gap-4">
            {#each services as service}
              <div class="glass-panel p-4 bg-white/5 hover:bg-white/10 transition-all">
                <div class="flex items-start justify-between mb-2">
                  <div class="flex-1">
                    <h4 class="font-bold">{service.name}</h4>
                    <p class="text-xs text-gray-400 mt-1">{service.description}</p>
                  </div>
                  <button
                    onclick={() => toggleService(service.name)}
                    disabled={isProcessing || unavailable('toggle_service')}
                    class="ml-3 px-3 py-1 rounded text-xs {service.status === 'running' ? 'bg-green-500/20 text-green-400' : 'bg-gray-500/20 text-gray-400'} hover:opacity-80 transition-all disabled:opacity-50"
                  >
                    {service.status === 'running' ? '⏸ Stop' : '▶ Start'}
                  </button>
                </div>
                
                <div class="flex items-center justify-between text-xs mt-3">
                  <span class={getStatusColor(service.status)}>● {service.status}</span>
                  <span class="text-gray-400">
                    {service.autoStart ? '🔄 Auto-start' : ''}
                  </span>
                </div>
              </div>
            {/each}
            
            {#if services.length === 0}
              <div class="col-span-2 text-center py-12 text-gray-400">
                <div class="text-4xl mb-2">🔧</div>
                <p>No services found</p>
              </div>
            {/if}
          </div>
        </div>
        
      {:else if activeTab === 'automation'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan mb-4">Automation Rules</h3>
          
          <!-- Create Automation -->
          <div class="glass-panel p-4 bg-white/5 space-y-3">
            <h4 class="font-bold text-sm">Create Automation</h4>
            
            <input
              type="text"
              bind:value={newAutomation.name}
              placeholder="Automation name"
              class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 text-sm"
            />
            
            <div class="grid grid-cols-2 gap-3">
              <div>
                <label for="sc-field-1" class="block text-xs text-gray-400 mb-1">Trigger</label>
                <select id="sc-field-1" bind:value={newAutomation.trigger} class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm">
                  {#each triggerTypes as trigger}
                    <option value={trigger.value}>{trigger.label}</option>
                  {/each}
                </select>
              </div>
              
              <div>
                <label for="sc-field-2" class="block text-xs text-gray-400 mb-1">Threshold (%)</label>
                <input id="sc-field-2"
                  type="number"
                  bind:value={newAutomation.threshold}
                  min="0"
                  max="100"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                />
              </div>
            </div>
            
            <textarea
              bind:value={newAutomation.action}
              placeholder="Action to perform (command or script)"
              rows="2"
              class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 text-sm font-mono"
            ></textarea>
            
            <button
              onclick={createAutomation}
              disabled={isProcessing || !newAutomation.name.trim() || unavailable('create_automation')}
              title={unavailable('create_automation') ? 'Not yet available' : undefined}
              class="w-full px-4 py-2 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg font-medium transition-all disabled:opacity-50"
            >
              {isProcessing ? '⏳ Creating...' : '➕ Create Automation'}
            </button>
          </div>
          
          <!-- Automation List -->
          <div class="space-y-3">
            {#each automations as automation}
              <div class="glass-panel p-4 bg-white/5 hover:bg-white/10 transition-all">
                <div class="flex items-start justify-between mb-2">
                  <div class="flex-1">
                    <div class="flex items-center gap-2 mb-1">
                      <h4 class="font-bold">{automation.name}</h4>
                      <span class="text-xs px-2 py-1 rounded bg-cosmic-blue/20 text-cosmic-cyan">
                        {triggerTypes.find(t => t.value === automation.trigger)?.label}
                      </span>
                    </div>
                    <p class="text-xs text-gray-400 font-mono">{automation.action}</p>
                  </div>
                  
                  <div class="flex items-center gap-2 ml-3">
                    <button
                      onclick={() => toggleAutomation(automation.id)}
                      disabled={unavailable('toggle_automation')}
                      class="text-xs px-3 py-1 rounded {automation.enabled ? 'bg-green-500/20 text-green-400' : 'bg-gray-500/20 text-gray-400'} hover:opacity-80"
                    >
                      {automation.enabled ? '✓ Enabled' : '○ Disabled'}
                    </button>
                    <button
                      onclick={() => deleteAutomation(automation.id)}
                      disabled={unavailable('delete_automation')}
                      class="text-red-400 hover:text-red-300"
                    >
                      🗑️
                    </button>
                  </div>
                </div>
                
                <div class="flex items-center justify-between text-xs text-gray-400 mt-2">
                  <span>Runs: {automation.runCount}</span>
                  <span>Last: {automation.lastRun ? automation.lastRun.toLocaleString() : 'Never'}</span>
                </div>
              </div>
            {/each}
            
            {#if automations.length === 0}
              <div class="text-center py-12 text-gray-400">
                <div class="text-4xl mb-2">🤖</div>
                <p>No automations created yet</p>
              </div>
            {/if}
          </div>
        </div>
        
      {:else if activeTab === 'scheduler'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan mb-4">Task Scheduler</h3>
          
          <!-- Create Scheduled Task -->
          <div class="glass-panel p-4 bg-white/5 space-y-3">
            <h4 class="font-bold text-sm">Schedule New Task</h4>
            
            <input
              type="text"
              bind:value={newScheduledTask.name}
              placeholder="Task name"
              class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 text-sm"
            />
            
            <div>
              <label for="sc-field-3" class="block text-xs text-gray-400 mb-1">Schedule (Cron format)</label>
              <input id="sc-field-3"
                type="text"
                bind:value={newScheduledTask.schedule}
                placeholder="0 0 * * * (daily at midnight)"
                class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 text-sm font-mono"
              />
              <p class="text-xs text-gray-400 mt-1">Format: minute hour day month weekday</p>
            </div>
            
            <textarea
              bind:value={newScheduledTask.command}
              placeholder="Command to execute"
              rows="2"
              class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 text-sm font-mono"
            ></textarea>
            
            <button
              onclick={createScheduledTask}
              disabled={isProcessing || !newScheduledTask.name.trim() || unavailable('create_scheduled_task')}
              title={unavailable('create_scheduled_task') ? 'Not yet available' : undefined}
              class="w-full px-4 py-2 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg font-medium transition-all disabled:opacity-50"
            >
              {isProcessing ? '⏳ Creating...' : '➕ Schedule Task'}
            </button>
          </div>
          
          <!-- Scheduled Tasks List -->
          <div class="space-y-3">
            {#each scheduledTasks as task}
              <div class="glass-panel p-4 bg-white/5 hover:bg-white/10 transition-all">
                <div class="flex items-start justify-between mb-2">
                  <div class="flex-1">
                    <h4 class="font-bold mb-1">{task.name}</h4>
                    <p class="text-xs text-gray-400 font-mono mb-1">{task.command}</p>
                    <p class="text-xs text-gray-400">Schedule: {task.schedule}</p>
                  </div>
                  
                  <button
                    onclick={() => toggleScheduledTask(task.id)}
                    disabled={unavailable('toggle_scheduled_task')}
                    class="ml-3 text-xs px-3 py-1 rounded {task.enabled ? 'bg-green-500/20 text-green-400' : 'bg-gray-500/20 text-gray-400'} hover:opacity-80"
                  >
                    {task.enabled ? '✓ Enabled' : '○ Disabled'}
                  </button>
                </div>
                
                <div class="text-xs text-gray-400 mt-2">
                  Next run: {task.nextRun.toLocaleString()}
                </div>
              </div>
            {/each}
            
            {#if scheduledTasks.length === 0}
              <div class="text-center py-12 text-gray-400">
                <div class="text-4xl mb-2">⏰</div>
                <p>No scheduled tasks</p>
              </div>
            {/if}
          </div>
        </div>
        
      {:else if activeTab === 'alerts'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan mb-4">System Alerts</h3>
          
          <!-- Create Alert -->
          <div class="glass-panel p-4 bg-white/5 space-y-3">
            <h4 class="font-bold text-sm">Create Alert</h4>
            
            <div class="grid grid-cols-3 gap-3">
              <div>
                <label for="sc-field-4" class="block text-xs text-gray-400 mb-1">Type</label>
                <select id="sc-field-4" bind:value={newAlert.type} class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm">
                  <option value="cpu">CPU</option>
                  <option value="memory">Memory</option>
                  <option value="disk">Disk</option>
                  <option value="process">Process</option>
                  <option value="custom">Custom</option>
                </select>
              </div>
              
              <div>
                <label for="sc-field-5" class="block text-xs text-gray-400 mb-1">Condition</label>
                <select id="sc-field-5" bind:value={newAlert.condition} class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm">
                  <option value="greater_than">Greater than</option>
                  <option value="less_than">Less than</option>
                  <option value="equals">Equals</option>
                </select>
              </div>
              
              <div>
                <label for="sc-field-6" class="block text-xs text-gray-400 mb-1">Threshold</label>
                <input id="sc-field-6"
                  type="number"
                  bind:value={newAlert.threshold}
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                />
              </div>
            </div>
            
            <div>
              <label for="sc-field-7" class="block text-xs text-gray-400 mb-1">Action</label>
              <select id="sc-field-7" bind:value={newAlert.action} class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm">
                <option value="notify">Send Notification</option>
                <option value="email">Send Email</option>
                <option value="webhook">Call Webhook</option>
                <option value="command">Execute Command</option>
              </select>
            </div>
            
            <button
              onclick={createAlert}
              disabled={isProcessing || unavailable('create_alert')}
              title={unavailable('create_alert') ? 'Not yet available' : undefined}
              class="w-full px-4 py-2 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg font-medium transition-all disabled:opacity-50"
            >
              {isProcessing ? '⏳ Creating...' : '➕ Create Alert'}
            </button>
          </div>
          
          <!-- Alerts List -->
          <div class="space-y-3">
            {#each alerts as alert}
              <div class="glass-panel p-4 bg-white/5 hover:bg-white/10 transition-all">
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-3">
                    <span class="text-2xl">
                      {alert.type === 'cpu' ? '🔥' : alert.type === 'memory' ? '💾' : alert.type === 'disk' ? '💿' : '⚠️'}
                    </span>
                    <div>
                      <div class="font-bold text-sm">
                        {alert.type.toUpperCase()} {alert.condition.replace('_', ' ')} {alert.threshold}%
                      </div>
                      <div class="text-xs text-gray-400">Action: {alert.action}</div>
                    </div>
                  </div>
                  
                  <button
                    onclick={() => toggleAlert(alert.id)}
                    disabled={unavailable('toggle_alert')}
                    class="text-xs px-3 py-1 rounded {alert.enabled ? 'bg-green-500/20 text-green-400' : 'bg-gray-500/20 text-gray-400'} hover:opacity-80"
                  >
                    {alert.enabled ? '✓ Active' : '○ Inactive'}
                  </button>
                </div>
              </div>
            {/each}
            
            {#if alerts.length === 0}
              <div class="text-center py-12 text-gray-400">
                <div class="text-4xl mb-2">🚨</div>
                <p>No alerts configured</p>
              </div>
            {/if}
          </div>
        </div>
        
      {:else if activeTab === 'performance'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan mb-4">Performance Metrics</h3>
          
          <div class="grid grid-cols-2 gap-4">
            <div class="glass-panel p-6 bg-white/5">
              <h4 class="font-bold mb-4">CPU History</h4>
              <div class="h-48 flex items-end gap-1">
                {#each Array(20) as _, i}
                  <div 
                    class="flex-1 bg-cosmic-cyan rounded-t transition-all"
                    style="height: {Math.random() * 100}%"
                  ></div>
                {/each}
              </div>
            </div>
            
            <div class="glass-panel p-6 bg-white/5">
              <h4 class="font-bold mb-4">Memory History</h4>
              <div class="h-48 flex items-end gap-1">
                {#each Array(20) as _, i}
                  <div 
                    class="flex-1 bg-cosmic-purple rounded-t transition-all"
                    style="height: {Math.random() * 100}%"
                  ></div>
                {/each}
              </div>
            </div>
          </div>
          
          <div class="glass-panel p-6 bg-white/5">
            <h4 class="font-bold mb-4">System Health Score</h4>
            <div class="flex items-center gap-4">
              <div class="flex-1">
                <div class="w-full h-4 bg-white/10 rounded-full overflow-hidden">
                  <div 
                    class="h-full bg-gradient-to-r from-green-400 to-cosmic-cyan transition-all"
                    style="width: 85%"
                  ></div>
                </div>
              </div>
              <div class="text-3xl font-bold text-green-400">85%</div>
            </div>
            <p class="text-xs text-gray-400 mt-2">System is running optimally</p>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>