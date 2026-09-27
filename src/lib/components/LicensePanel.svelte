<script lang="ts">
  import { onMount } from 'svelte';
  import { call, errorMessage } from '$lib/api';
  import type { LicenseStatus, LicenseTier } from '$lib/types';

  let status = $state<LicenseStatus | null>(null);
  let key = $state('');
  let busy = $state(false);
  let message = $state<{ ok: boolean; text: string } | null>(null);

  const TIER_LABEL: Record<LicenseTier, string> = {
    community: 'Community',
    pro: 'Lifetime Pro',
    byok: 'BYOK (subscription)',
    enterprise: 'Enterprise Hardened'
  };

  onMount(refresh);

  async function refresh() {
    try {
      status = await call<LicenseStatus>('license_status');
    } catch (e) {
      message = { ok: false, text: errorMessage(e) };
    }
  }

  async function install() {
    if (!key.trim()) return;
    busy = true;
    message = null;
    try {
      status = await call<LicenseStatus>('license_install', { key: key.trim() });
      key = '';
      message = { ok: true, text: `License installed: ${TIER_LABEL[status.tier]}` };
    } catch (e) {
      message = { ok: false, text: errorMessage(e) };
    } finally {
      busy = false;
    }
  }

  async function remove() {
    busy = true;
    message = null;
    try {
      status = await call<LicenseStatus>('license_remove');
      message = { ok: true, text: 'License removed' };
    } catch (e) {
      message = { ok: false, text: errorMessage(e) };
    } finally {
      busy = false;
    }
  }
</script>

<div class="space-y-6">
  <h3 class="text-xl font-bold text-cosmic-cyan mb-4">License</h3>

  {#if status}
    <div class="glass-panel p-4 bg-white/5 space-y-2" data-testid="license-status">
      <div class="flex items-center justify-between">
        <span class="text-sm text-gray-400">Current tier</span>
        <span class="text-lg font-semibold text-cosmic-cyan">{TIER_LABEL[status.tier]}</span>
      </div>
      {#if status.licensee}
        <div class="text-sm text-gray-300">
          Licensed to <strong>{status.licensee}</strong>{#if status.email} ({status.email}){/if}
          {#if status.licensed_tier && status.licensed_tier !== status.tier}· key is for {TIER_LABEL[status.licensed_tier]}{/if}
        </div>
        <div class="text-xs text-gray-400">
          Issued {status.issued} · {status.expires ? `valid through ${status.expires}` : 'lifetime'}
          {#if status.seats} · {status.seats} seats{/if}
          {#if status.id} · id {status.id.slice(0, 8)}{/if}
        </div>
      {/if}
      {#if status.expired}
        <div class="text-sm text-yellow-300">This license has expired. Renew it to restore the {status.licensed_tier ? TIER_LABEL[status.licensed_tier] : ''} tier.</div>
      {/if}
      {#if status.error}
        <div class="text-sm text-red-300">Installed key not accepted: {status.error}</div>
      {/if}
      <ul class="text-sm text-gray-300 list-disc pl-5 pt-2">
        {#each status.entitlements as e (e)}<li>{e}</li>{/each}
      </ul>
      {#if !status.enforced}
        <p class="text-xs text-gray-500 pt-2">
          Tiers are informational in this version: every feature works on every tier. License keys are checked
          offline on this machine; nothing is sent anywhere.
        </p>
      {/if}
    </div>
  {/if}

  <div class="space-y-2">
    <label for="license-key" class="block text-sm font-medium">License key</label>
    <textarea
      id="license-key"
      bind:value={key}
      rows="3"
      spellcheck="false"
      placeholder="OMNIX1.…"
      class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 font-mono text-xs"
    ></textarea>
    <div class="flex gap-3">
      <button
        onclick={install}
        disabled={busy || !key.trim()}
        class="px-4 py-2 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg text-sm disabled:opacity-50"
      >Install license</button>
      {#if status?.licensee || status?.error}
        <button onclick={remove} disabled={busy} class="px-4 py-2 bg-white/10 hover:bg-white/20 rounded-lg text-sm disabled:opacity-50">
          Remove license
        </button>
      {/if}
    </div>
    {#if message}
      <p class="text-sm {message.ok ? 'text-green-400' : 'text-red-300'}" role="status">{message.text}</p>
    {/if}
  </div>

  <div class="glass-panel p-4 bg-white/5 text-sm text-gray-300 space-y-1">
    <div><strong>Lifetime Pro</strong>: one-time purchase for local power users.</div>
    <div><strong>BYOK</strong>: monthly subscription for using cloud models with your own API keys.</div>
    <div><strong>Enterprise Hardened</strong>: air-gapped installs with a custom support agreement.</div>
  </div>
</div>
