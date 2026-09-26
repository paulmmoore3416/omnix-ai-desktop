<script lang="ts">
  import type { Condition, Metric } from '$lib/types';

  let { condition = $bindable(), idPrefix }: { condition: Condition; idPrefix: string } = $props();

  const metrics: { value: Metric; label: string; unit: string; target?: string }[] = [
    { value: 'cpu', label: 'CPU usage', unit: '%' },
    { value: 'memory', label: 'Memory usage', unit: '%' },
    { value: 'swap', label: 'Swap usage', unit: '%' },
    { value: 'disk', label: 'Disk usage', unit: '%' },
    { value: 'temperature', label: 'Hottest sensor', unit: '°C' },
    { value: 'gpu_util', label: 'GPU load', unit: '%', target: 'GPU index (empty = any)' },
    { value: 'gpu_memory', label: 'GPU memory', unit: '%', target: 'GPU index (empty = any)' },
    { value: 'gpu_temp', label: 'GPU temperature', unit: '°C', target: 'GPU index (empty = any)' },
    { value: 'process_missing', label: 'Process not running', unit: '', target: 'process name, e.g. ollama' },
    { value: 'service_down', label: 'Service down', unit: '', target: 'unit or user/unit, e.g. user/omnix-kb-core.service' },
    { value: 'container_down', label: 'Container down', unit: '', target: 'container name' },
    { value: 'ollama_down', label: 'Ollama unreachable', unit: '' },
    { value: 'kb_core_down', label: 'Memory service unreachable', unit: '' }
  ];
  let meta = $derived(metrics.find((m) => m.value === condition.metric) ?? metrics[0]);
  let boolean = $derived(meta.unit === '');
</script>

<div class="grid grid-cols-2 md:grid-cols-4 gap-2">
  <div>
    <label for="{idPrefix}-metric" class="block text-xs text-gray-400 mb-1">When</label>
    <select id="{idPrefix}-metric" bind:value={condition.metric} class="w-full bg-white/5 border border-white/10 rounded-lg px-2 py-2 text-sm">
      {#each metrics as m}<option value={m.value}>{m.label}</option>{/each}
    </select>
  </div>
  {#if !boolean}
    <div>
      <label for="{idPrefix}-op" class="block text-xs text-gray-400 mb-1">is</label>
      <select id="{idPrefix}-op" bind:value={condition.op} class="w-full bg-white/5 border border-white/10 rounded-lg px-2 py-2 text-sm">
        <option value="above">above</option><option value="below">below</option>
      </select>
    </div>
    <div>
      <label for="{idPrefix}-threshold" class="block text-xs text-gray-400 mb-1">Threshold ({meta.unit})</label>
      <input id="{idPrefix}-threshold" type="number" bind:value={condition.threshold} class="w-full bg-white/5 border border-white/10 rounded-lg px-2 py-2 text-sm" />
    </div>
  {/if}
  <div>
    <label for="{idPrefix}-sustain" class="block text-xs text-gray-400 mb-1">For at least (seconds)</label>
    <input id="{idPrefix}-sustain" type="number" min="0" max="86400" bind:value={condition.sustain_secs} class="w-full bg-white/5 border border-white/10 rounded-lg px-2 py-2 text-sm" />
  </div>
  {#if meta.target}
    <div class="col-span-2 md:col-span-4">
      <label for="{idPrefix}-target" class="block text-xs text-gray-400 mb-1">Target</label>
      <input id="{idPrefix}-target" bind:value={condition.target} placeholder={meta.target} class="w-full bg-white/5 border border-white/10 rounded-lg px-2 py-2 text-sm" />
    </div>
  {/if}
</div>
