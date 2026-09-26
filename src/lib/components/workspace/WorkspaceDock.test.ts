import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';

const invoke = vi.fn();
// A fake Channel the test can push events through.
let lastChannel: { onmessage?: (e: unknown) => void } | null = null;
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/api/core', () => ({
  Channel: class {
    onmessage?: (e: unknown) => void;
    constructor() {
      lastChannel = this;
    }
  },
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args)
}));

import WorkspaceDock from './WorkspaceDock.svelte';

const props = () => ({
  notify: vi.fn(),
  onChat: vi.fn(),
  onInsert: vi.fn(),
  onOpenRules: vi.fn(),
  onClose: vi.fn()
});

beforeEach(() => {
  localStorage.clear();
  localStorage.setItem('omnix.workspace.tab', 'tasks');
  invoke.mockReset();
});
afterEach(cleanup);

describe('WorkspaceDock', () => {
  it('streams a side task and renders its output', async () => {
    let finish: () => void = () => {};
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'task_run') return new Promise<void>((r) => (finish = r));
      return Promise.resolve(null);
    });
    render(WorkspaceDock, props());
    const box = screen.getByPlaceholderText(/beside the chat/);
    await fireEvent.input(box, { target: { value: 'Summarize the week' } });
    await fireEvent.click(screen.getByText('⚡ Run side task'));
    expect(invoke).toHaveBeenCalledWith('task_run', expect.objectContaining({ instruction: 'Summarize the week', context: null }));
    lastChannel?.onmessage?.({ type: 'token', text: '**Done** <script>x</script>' });
    finish();
    expect(await screen.findByText('Done')).toBeInTheDocument();
    // Output is sanitized.
    expect(document.querySelector('script')).toBeNull();
  });

  it('runs a slash command through process_command', async () => {
    invoke.mockImplementation((cmd: string) => Promise.resolve(cmd === 'process_command' ? 'System status ok' : null));
    render(WorkspaceDock, props());
    await fireEvent.input(screen.getByPlaceholderText(/beside the chat/), { target: { value: '/monitor' } });
    await fireEvent.click(screen.getByText('⚡ Run side task'));
    expect(invoke).toHaveBeenCalledWith('process_command', { command: '/monitor' });
    expect(await screen.findByText('System status ok')).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith('task_run', expect.anything());
  });

  it('sends a library prompt to the chat', async () => {
    localStorage.setItem('omnix.workspace.tab', 'prompts');
    const p = props();
    render(WorkspaceDock, p);
    await fireEvent.input(screen.getByPlaceholderText(/Fill-in/), { target: { value: 'visitor badges' } });
    await fireEvent.click(screen.getAllByText('💬 Chat')[2]); // "SOP outline"
    expect(p.onChat).toHaveBeenCalledWith(expect.stringContaining('standard operating procedure for: visitor badges'));
  });
});
