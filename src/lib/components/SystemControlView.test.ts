import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';

const gpu = (index: number, vendor: string, name: string, util: number) => ({
  index, vendor, name, driver: vendor === 'amd' ? 'amdgpu' : 'nvidia', pci: `0000:${index}:00.0`,
  utilization: util, memory_used: 3e9, memory_total: 8e9, temperature: 50, power_w: 30, power_limit_w: 120,
  fan_percent: 20, clock_mhz: 1300, clock_max_mhz: 1400, processes: [], note: null
});

const perf = {
  host: {
    cpu: 12, per_core: [10, 20, 30, 40], cpu_freq_mhz: 3200, load: [0.5, 0.4, 0.3],
    memory_total: 48e9, memory_used: 12e9, memory_available: 34e9, swap_total: 8e9, swap_used: 0,
    disks: [{ name: 'nvme0n1p2', mount: '/', fs: 'ext4', total: 1e12, used: 4e11, removable: false }],
    network: [{ name: 'eth0', rx: 1000, tx: 500, total_rx: 1e9, total_tx: 1e8 }],
    sensors: [{ label: 'Package id 0', temperature: 55, critical: 100 }], uptime: 7200, processes: 400
  },
  gpus: [gpu(0, 'amd', 'AMD Radeon RX 580', 5), gpu(1, 'nvidia', 'NVIDIA GeForce GTX 1060 6GB', 40)],
  history: [
    { ts: 1, cpu: 10, memory: 25, swap: 0, disk: 40, net_rx: 100, net_tx: 50, temp: 50, gpus: [{ util: 1, mem: 40, temp: 50 }, { util: 30, mem: 50, temp: 45 }] },
    { ts: 2, cpu: 12, memory: 26, swap: 0, disk: 40, net_rx: 200, net_tx: 60, temp: 51, gpus: [{ util: 5, mem: 40, temp: 50 }, { util: 40, mem: 55, temp: 46 }] }
  ],
  agent: {
    uptime_s: 600, turns: 3, errors: 0, cancelled: 0,
    models: [{ model: 'qwen3:8b', turns: 3, generations: 4, prompt_tokens: 900, output_tokens: 300, tokens_estimated: false, tokens_per_sec: 31.4, prompt_tokens_per_sec: 400, avg_ttft_ms: 850, cold_starts: 1, load_ms: 3000 }],
    tools: [{ tool: 'host_status', calls: 2, ok: 2, failed: 0, avg_ms: 120, max_ms: 150 }],
    recall: { runs: 3, hits: 5, empty: 1 }, capture: { runs: 0, saved: 0 }, recent: []
  },
  models: { configured: 'qwen3:8b', installed: [], loaded: [{ name: 'qwen3:8b', size: 6.7e9, size_vram: 6.7e9, gpu_percent: 100, context_length: 8192, expires_at: null, parameter_size: '8.2B', quantization: 'Q4_K_M' }], error: null },
  memory_store: null
};

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/api/core', () => ({
  Channel: class {},
  invoke: vi.fn(async (cmd: string) => {
    switch (cmd) {
      case 'get_system_info':
        return { os: 'Ubuntu', kernel: '7.0', os_version: '26.04', hostname: 'mediaship', uptime: 7200, cpu_model: 'i7', cpu_cores: 24, total_memory: 48e9, used_memory: 12e9, total_swap: 0, used_swap: 0 };
      case 'get_performance':
        return perf;
      case 'get_system_control_data':
        return { available: true, alerts: [], automations: [], tasks: [], activity: [] };
      default:
        return [];
    }
  })
}));

import SystemControlView from './SystemControlView.svelte';

afterEach(cleanup);

describe('SystemControlView', () => {
  it('shows every GPU (including non-NVIDIA) with live data and no fake banner', async () => {
    render(SystemControlView);
    expect(await screen.findByText(/GPU 0 · AMD/)).toBeInTheDocument();
    expect(screen.getByText(/GPU 1 · NVIDIA/)).toBeInTheDocument();
    expect(screen.queryByText(/not implemented yet/i)).not.toBeInTheDocument();
    expect(screen.getByText('mediaship')).toBeInTheDocument();
  });

  it('shows agent and model metrics on the Performance tab', async () => {
    render(SystemControlView);
    await screen.findByText(/GPU 0 · AMD/);
    await fireEvent.click(screen.getByRole('button', { name: /Performance/ }));
    expect(await screen.findByText('31.4')).toBeInTheDocument(); // tokens/s
    expect(screen.getByText('AMD Radeon RX 580', { exact: false })).toBeInTheDocument();
    expect(screen.getByText('host_status')).toBeInTheDocument();
  });

  it('offers alert presets', async () => {
    render(SystemControlView);
    await fireEvent.click(screen.getByRole('button', { name: /Alerts/ }));
    expect(await screen.findByRole('button', { name: 'GPU running hot' })).toBeInTheDocument();
  });
});
