import { beforeEach, describe, expect, it } from 'vitest';
import {
  DEFAULT_PROMPTS,
  MAX_PINS,
  addPin,
  briefContext,
  fillPrompt,
  loadPins,
  loadPrompts,
  newTaskId,
  savePins,
  savePrompts,
  stripThinking,
  taskTitle
} from './workspace';
import type { PerformanceData, SystemControlData } from './types';

beforeEach(() => localStorage.clear());

describe('side tasks', () => {
  it('makes ids the backend accepts', () => {
    const a = newTaskId(1_700_000_000_000);
    const b = newTaskId(1_700_000_000_000);
    expect(a).toMatch(/^[A-Za-z0-9_-]{1,64}$/);
    expect(a).not.toBe(b);
  });

  it('titles are one clipped line', () => {
    expect(taskTitle('  hello \n world ')).toBe('hello world');
    expect(taskTitle('x'.repeat(100), 10)).toHaveLength(10);
  });

  it('hides model reasoning, including an unfinished block', () => {
    expect(stripThinking('<think>hmm</think>\nAnswer')).toBe('Answer');
    expect(stripThinking('<think>still going')).toBe('');
    expect(stripThinking('plain')).toBe('plain');
  });
});

describe('prompt library', () => {
  it('fills {{input}} or appends the input', () => {
    expect(fillPrompt('Outline {{input}} now', 'x')).toBe('Outline x now');
    expect(fillPrompt('Rewrite:', 'text')).toBe('Rewrite:\n\ntext');
    expect(fillPrompt('Just this', '  ')).toBe('Just this');
  });

  it('starts with the defaults and round-trips edits', () => {
    expect(loadPrompts().map((p) => p.id)).toEqual(DEFAULT_PROMPTS.map((p) => p.id));
    savePrompts([{ id: 'u1', title: 'Mine', icon: '⭐', text: 'Do {{input}}', target: 'task' }]);
    expect(loadPrompts()).toHaveLength(1);
  });

  it('falls back to the defaults on corrupt storage', () => {
    localStorage.setItem('omnix.workspace.prompts.v1', '{not json');
    expect(loadPrompts()).toHaveLength(DEFAULT_PROMPTS.length);
  });
});

describe('pinboard', () => {
  it('adds to the front, dedupes and caps', () => {
    let pins = addPin([], 'one', 'a', 1);
    pins = addPin(pins, 'two', 'b', 2);
    pins = addPin(pins, 'one', 'c', 3);
    expect(pins.map((p) => p.text)).toEqual(['one', 'two']);
    expect(addPin(pins, '   ', 'x')).toBe(pins);
    for (let i = 0; i < MAX_PINS + 5; i++) pins = addPin(pins, `p${i}`, 's', i);
    expect(pins).toHaveLength(MAX_PINS);
    savePins(pins);
    expect(loadPins()).toHaveLength(MAX_PINS);
  });
});

describe('situation brief', () => {
  it('summarizes host, GPUs, models and ops without chat content', () => {
    const perf = {
      host: {
        cpu: 12, per_core: [], cpu_freq_mhz: null, load: [0.5, 0.4, 0.3], memory_total: 48e9, memory_used: 12e9,
        memory_available: 36e9, swap_total: 0, swap_used: 0,
        disks: [{ name: 'sda', mount: '/', fs: 'ext4', total: 100, used: 95, removable: false }],
        network: [], sensors: [], uptime: 7200, processes: 400
      },
      gpus: [{ index: 1, vendor: 'nvidia', name: 'GTX 1060', driver: null, pci: null, utilization: 40, memory_used: 3e9, memory_total: 6e9, temperature: 60, power_w: null, power_limit_w: null, fan_percent: null, clock_mhz: null, clock_max_mhz: null, processes: [], note: null }],
      history: [],
      agent: { uptime_s: 1, turns: 2, errors: 1, cancelled: 0, models: [], tools: [], recall: { runs: 0, hits: 0, empty: 0 }, capture: { runs: 0, saved: 0 }, recent: [] },
      models: { configured: 'qwen3:8b', installed: null, loaded: [{ name: 'qwen3:8b', size: 5e9, size_vram: 5e9, gpu_percent: 100, context_length: null, expires_at: null, parameter_size: null, quantization: null }], error: null },
      memory_store: null
    } as unknown as PerformanceData;
    const ops = {
      available: true,
      alerts: [{ alert: { id: 'a', name: 'Disk full', enabled: true, state: { firing: true } }, description: 'disk above 90%', value: 95, unit: '%' }],
      automations: [],
      tasks: [{ task: { id: 't', name: 'Nightly backup', enabled: true, next_run: '2026-09-27T02:00:00Z' }, when: 'daily', action: 'x', approved: true }],
      activity: [{ ts: '2026-09-26T10:00:00Z', kind: 'schedule', name: 'Sync', ok: false, summary: 'exit 1' }]
    } as unknown as SystemControlData;
    const text = briefContext(perf, ops, new Date('2026-09-26T12:00:00Z'));
    expect(text).toContain('CPU 12%');
    expect(text).toContain('Disk /: 95% used');
    expect(text).toContain('GPU 1 nvidia GTX 1060');
    expect(text).toContain('Loaded models: qwen3:8b');
    expect(text).toContain('Alerts firing: Disk full');
    expect(text).toContain('Next scheduled: Nightly backup');
    expect(text).toContain('Recent failures: Sync');
  });

  it('still works with nothing available', () => {
    expect(briefContext(null, null)).toContain('Host metrics: unavailable');
  });
});
