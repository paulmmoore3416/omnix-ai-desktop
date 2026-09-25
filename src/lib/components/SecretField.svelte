<script lang="ts">
  /**
   * Write-only secret input. The stored value is never read back from the
   * backend; the component only knows whether a secret exists (`has`).
   */
  import { onMount } from 'svelte';
  import { call, errorMessage } from '$lib/api';
  import type { SecretProvider, Settings } from '$lib/types';

  let {
    provider,
    label,
    placeholder = '',
    has,
    onchange
  }: {
    provider: SecretProvider;
    label: string;
    placeholder?: string;
    /** Whether a secret exists; when omitted the keychain is queried. */
    has?: boolean;
    onchange?: (s: Settings) => void;
  } = $props();

  let checked = $state<boolean | null>(null);
  let saved = $derived(has ?? checked ?? false);

  onMount(async () => {
    if (has === undefined) {
      checked = await call<boolean>('has_secret', { provider }).catch(() => false);
    }
  });

  let editing = $state(false);
  let value = $state('');
  let busy = $state(false);
  let error = $state('');

  const inputId = $derived(`secret-${provider}`);

  async function save() {
    if (!value.trim()) return;
    busy = true;
    error = '';
    try {
      const s = await call<Settings>('set_secret', { provider, value });
      value = '';
      editing = false;
      checked = true;
      onchange?.(s);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function remove() {
    busy = true;
    error = '';
    try {
      const s = await call<Settings>('delete_secret', { provider });
      checked = false;
      onchange?.(s);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }
</script>

<div>
  <label for={inputId} class="block text-sm font-medium mb-2">{label}</label>
  {#if saved && !editing}
    <div class="flex items-center gap-2">
      <span class="text-green-400 text-sm flex-1">Key saved in OS keychain ✓</span>
      <button class="glass-panel px-3 py-1 text-xs hover:bg-white/10" onclick={() => (editing = true)} disabled={busy}>Replace</button>
      <button class="glass-panel px-3 py-1 text-xs text-red-400 hover:bg-white/10" onclick={remove} disabled={busy}>Remove</button>
    </div>
  {:else}
    <div class="flex items-center gap-2">
      <input
        id={inputId}
        type="password"
        autocomplete="off"
        bind:value
        {placeholder}
        class="flex-1 bg-white/5 border border-white/10 rounded-lg px-4 py-2"
      />
      <button class="glass-panel px-3 py-2 text-xs hover:bg-white/10 disabled:opacity-50" onclick={save} disabled={busy || !value.trim()}>Save key</button>
      {#if saved}
        <button class="glass-panel px-3 py-2 text-xs hover:bg-white/10" onclick={() => { editing = false; value = ''; }}>Cancel</button>
      {/if}
    </div>
    <p class="text-xs text-gray-400 mt-1">Stored in the OS keychain. It is never shown again or written to settings.json.</p>
  {/if}
  {#if error}<p class="text-xs text-red-400 mt-1">{error}</p>{/if}
</div>
