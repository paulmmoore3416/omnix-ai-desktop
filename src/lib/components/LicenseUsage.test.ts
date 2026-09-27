import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import LicensePanel from './LicensePanel.svelte';
import UsagePanel from './UsagePanel.svelte';
import HelpView from './HelpView.svelte';
import type { LicenseStatus, UsageSummary, UsageTotals } from '$lib/types';

afterEach(() => {
  cleanup();
  vi.mocked(invoke).mockReset();
});

const community: LicenseStatus = {
  tier: 'community', licensed_tier: null, licensee: null, email: null, id: null, issued: null, expires: null,
  expired: false, seats: null, error: null, entitlements: ['Local models'], enforced: false
};

function totals(p: Partial<UsageTotals> = {}): UsageTotals {
  return {
    turns: 0, local_turns: 0, cloud_turns: 0, errors: 0, cancelled: 0, prompt_tokens: 0, output_tokens: 0,
    local_prompt_tokens: 0, local_output_tokens: 0, tool_calls: 0, recalled: 0, active_ms: 0, models: {}, ...p
  };
}

describe('LicensePanel', () => {
  it('shows the community tier and says nothing is gated', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(community);
    render(LicensePanel);
    expect(await screen.findByText('Community')).toBeInTheDocument();
    expect(screen.getByText(/every feature works on every tier/)).toBeInTheDocument();
  });

  it('installs a pasted key and shows the licensee', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(community)
      .mockResolvedValueOnce({ ...community, tier: 'pro', licensed_tier: 'pro', licensee: 'Jane', issued: '2026-09-26' });
    render(LicensePanel);
    await screen.findByText('Community');
    await fireEvent.input(screen.getByLabelText('License key'), { target: { value: '  OMNIX1.a.b  ' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Install license' }));
    expect(invoke).toHaveBeenCalledWith('license_install', { key: 'OMNIX1.a.b' });
    expect(await screen.findByText('Jane')).toBeInTheDocument();
  });

  it('reports a rejected key', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(community)
      .mockRejectedValueOnce({ kind: 'invalid_input', message: 'license signature does not verify' });
    render(LicensePanel);
    await screen.findByText('Community');
    await fireEvent.input(screen.getByLabelText('License key'), { target: { value: 'OMNIX1.x.y' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Install license' }));
    expect(await screen.findByText('license signature does not verify')).toBeInTheDocument();
  });
});

describe('UsagePanel', () => {
  it('shows local work and the cloud-equivalent cost at the reference rates', async () => {
    const summary: UsageSummary = {
      days: [{ date: '2026-09-26', ...totals({ turns: 3 }) }],
      window: totals({ turns: 3, local_turns: 3, local_prompt_tokens: 1_000_000, local_output_tokens: 1_000_000, models: { 'qwen3:8b': 3 } }),
      all_time: totals({ turns: 3 }),
      since: '2026-09-26',
      window_days: 30
    };
    vi.mocked(invoke).mockResolvedValueOnce(summary);
    render(UsagePanel);
    // Defaults: $3 in + $15 out per million tokens.
    expect(await screen.findByText('$18.00')).toBeInTheDocument();
    expect(screen.getByText('qwen3:8b')).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith('usage_summary', { days: 30 });
  });

  it('asks twice before clearing', async () => {
    const empty: UsageSummary = { days: [], window: totals(), all_time: totals(), since: null, window_days: 30 };
    vi.mocked(invoke).mockResolvedValue(empty);
    render(UsagePanel);
    const btn = await screen.findByRole('button', { name: 'Clear usage history' });
    await fireEvent.click(btn);
    const cleared = () => vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === 'usage_clear');
    expect(cleared()).toBe(false);
    await fireEvent.click(screen.getByRole('button', { name: /Click again/ }));
    expect(cleared()).toBe(true);
  });
});

describe('HelpView', () => {
  it('renders the bundled local model guide', () => {
    render(HelpView);
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(/setting up local models/i);
    expect(screen.getAllByText(/ollama pull nomic-embed-text/).length).toBeGreaterThan(0);
    // Links in help are neutralised like chat output (no navigation away from the app).
    expect(document.querySelector('article a[href]')).toBeNull();
  });
});
