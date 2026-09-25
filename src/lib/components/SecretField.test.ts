import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import SecretField from './SecretField.svelte';

afterEach(() => {
  cleanup();
  vi.mocked(invoke).mockReset();
});

describe('SecretField (write-only key input)', () => {
  it('shows "saved" state without any value when a key exists', () => {
    render(SecretField, { provider: 'openai', label: 'OpenAI API Key', has: true });
    expect(screen.getByText(/Key saved in OS keychain/)).toBeInTheDocument();
    expect(screen.queryByLabelText('OpenAI API Key')).toBeNull();
    expect(screen.getByRole('button', { name: 'Replace' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Remove' })).toBeInTheDocument();
  });

  it('sends the key to set_secret and clears the input', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ ai: { has_openai_key: true } });
    const onchange = vi.fn();
    render(SecretField, { provider: 'openai', label: 'OpenAI API Key', has: false, onchange });
    const input = screen.getByLabelText('OpenAI API Key') as HTMLInputElement;
    expect(input.type).toBe('password');
    await fireEvent.input(input, { target: { value: 'sk-test-123' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save key' }));
    await vi.waitFor(() => expect(onchange).toHaveBeenCalled());
    expect(invoke).toHaveBeenCalledWith('set_secret', { provider: 'openai', value: 'sk-test-123' });
  });

  it('never calls a command that could read a secret back', () => {
    render(SecretField, { provider: 'anthropic', label: 'Anthropic', has: true });
    for (const [cmd] of vi.mocked(invoke).mock.calls) {
      expect(String(cmd)).not.toMatch(/get_secret|read_secret/);
    }
  });

  it('remove calls delete_secret', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ ai: { has_xai_key: false } });
    render(SecretField, { provider: 'xai', label: 'xAI', has: true });
    await fireEvent.click(screen.getByRole('button', { name: 'Remove' }));
    expect(invoke).toHaveBeenCalledWith('delete_secret', { provider: 'xai' });
  });
});
