import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (cmd: string, args?: unknown) => invoke(cmd, args) }));

import LiveMetrics from './LiveMetrics.svelte';

const gpu = { index: 0, vendor: 'nvidia', name: 'GTX 1060', driver: null, pci: '01:00.0', utilization: 40, memory_used: 1e9, memory_total: 6e9, temperature: 55, power_w: 60, power_limit_w: null, fan_percent: null, clock_mhz: null, clock_max_mhz: null, note: null };
const perf = {
  host: { cpu: 10, per_core: [], cpu_freq_mhz: null, load: [0, 0, 0], memory_total: 100, memory_used: 50, memory_available: 50, swap_total: 0, swap_used: 0, disks: [{ name: 'sda', mount: '/', fs: 'ext4', total: 100, used: 95, removable: false }], network: [], sensors: [], uptime: 1, processes: 1 },
  gpus: [gpu],
  history: [0, 1, 2].map((i) => ({ ts: Date.now() - (3 - i) * 5000, cpu: 10 + i, memory: 50, swap: 0, disk: null, net_rx: 1, net_tx: 1, temp: null, gpus: [{ util: 30, mem: 10, temp: 50 }] })),
  agent: { uptime_s: 1, turns: 3, errors: 0, cancelled: 0, models: [], tools: [], recall: { runs: 0, hits: 0, empty: 0 }, capture: { runs: 0, saved: 0 }, recent: [] },
  models: { configured: 'qwen3:8b', installed: [], loaded: [{ name: 'qwen3:8b', size_vram: 5e9, gpu_percent: 100 }], error: null },
  memory_store: null
};

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_performance') return Promise.resolve(perf);
    if (cmd === 'get_real_time_stats') return Promise.resolve({ cpu: 42, memory: 61, swap: 0, disk: 50, network: { rx: 2048, tx: 1024 }, temperature: 50 });
    if (cmd === 'get_gpus') return Promise.resolve([gpu]);
    return Promise.resolve(null);
  });
});
afterEach(cleanup);

describe('LiveMetrics', () => {
  it('polls cheap live readings alongside the detailed snapshot', async () => {
    render(LiveMetrics, { notify: vi.fn() });
    expect(await screen.findByText('qwen3:8b')).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith('get_performance', { historySecs: 144 });
    expect(invoke).toHaveBeenCalledWith('get_real_time_stats', undefined);
    expect(invoke).toHaveBeenCalledWith('get_gpus', undefined);
    expect(await screen.findByText(/GTX 1060/)).toBeInTheDocument();
    expect(await screen.findByText(/↓ 2.0 KB\/s/)).toBeInTheDocument();
    // Charts are drawn as smooth paths.
    expect(document.querySelectorAll('svg path').length).toBeGreaterThan(0);
  });

  it('does nothing while inactive', async () => {
    render(LiveMetrics, { notify: vi.fn(), active: false });
    await new Promise((r) => setTimeout(r, 10));
    expect(invoke).not.toHaveBeenCalled();
  });
});
