<script lang="ts">
  /**
   * Editor for MCP server entries (Settings → Integrations). Secrets (env
   * vars marked secret, HTTP bearer tokens) are stored in the OS keychain via
   * SecretField and never appear in settings.json.
   */
  import { call, errorMessage } from '$lib/api';
  import type { McpServerConfig, McpServerStatus } from '$lib/types';
  import SecretField from './SecretField.svelte';

  let { servers = $bindable() }: { servers: McpServerConfig[] } = $props();

  let status = $state<Record<string, { ok: boolean; text: string }>>({});
  let testing = $state<string | null>(null);

  function addServer(kind: 'stdio' | 'http') {
    const base = `server${servers.length + 1}`;
    servers = [
      ...servers,
      {
        name: base,
        enabled: false,
        transport:
          kind === 'stdio'
            ? { type: 'stdio', command: '', args: [], env: {}, secret_env: [] }
            : { type: 'http', url: '', bearer_token: false },
        read_only_tools: []
      }
    ];
  }

  function remove(i: number) {
    servers = servers.filter((_, j) => j !== i);
  }

  function lines(v: string): string[] {
    return v.split('\n').map((l) => l.trim()).filter(Boolean);
  }

  function envText(env: Record<string, string>): string {
    return Object.entries(env).map(([k, v]) => `${k}=${v}`).join('\n');
  }

  function parseEnv(v: string): Record<string, string> {
    const out: Record<string, string> = {};
    for (const line of lines(v)) {
      const i = line.indexOf('=');
      if (i > 0) out[line.slice(0, i).trim()] = line.slice(i + 1);
    }
    return out;
  }

  async function test(name: string) {
    testing = name;
    try {
      const r = await call<McpServerStatus>('mcp_test_server', { name });
      status[name] = { ok: true, text: `Connected: ${r.tools.length} tools (${r.tools.join(', ') || 'none'})` };
    } catch (e) {
      status[name] = { ok: false, text: errorMessage(e) };
    } finally {
      testing = null;
    }
  }
</script>

<div class="space-y-4">
  <div class="glass-panel p-4 bg-white/5 text-sm text-gray-300 space-y-1">
    <p>Register <strong>MCP servers</strong> to give the assistant extra tools (GitHub, Drive, databases, …).</p>
    <p>Every tool call opens a native approval dialog unless you list the tool as read-only below, and every call is audited. Starting a local server is confirmed once per session. Save settings before testing.</p>
  </div>

  {#each servers as server, i (i)}
    <div class="glass-panel p-4 bg-white/5 space-y-3">
      <div class="flex items-center gap-3">
        <label class="text-sm flex items-center gap-2">
          <input type="checkbox" bind:checked={server.enabled} class="w-4 h-4" /> Enabled
        </label>
        <input
          aria-label="Server name"
          bind:value={server.name}
          class="flex-1 bg-white/5 border border-white/10 rounded-lg px-3 py-1 text-sm font-mono"
          placeholder="name (a-z, 0-9, _ -)"
        />
        <span class="text-xs text-gray-400">{server.transport.type === 'stdio' ? 'local process' : 'HTTP'}</span>
        <button onclick={() => test(server.name)} disabled={testing === server.name} class="glass-panel px-3 py-1 text-xs hover:bg-white/10 disabled:opacity-50">
          {testing === server.name ? '⏳' : '🧪 Test'}
        </button>
        <button onclick={() => remove(i)} class="text-red-400 text-sm" aria-label="Remove server">🗑️</button>
      </div>

      {#if server.transport.type === 'stdio'}
        {@const t = server.transport}
        <input aria-label="Command" bind:value={t.command} placeholder="command (e.g. npx)" class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono" />
        <textarea
          aria-label="Arguments"
          value={t.args.join('\n')}
          oninput={(e) => (t.args = lines(e.currentTarget.value))}
          rows="2"
          placeholder="arguments, one per line"
          class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono"
        ></textarea>
        <textarea
          aria-label="Environment"
          value={envText(t.env)}
          oninput={(e) => (t.env = parseEnv(e.currentTarget.value))}
          rows="2"
          placeholder="NON_SECRET_VAR=value (one per line); put tokens in secret variables below"
          class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono"
        ></textarea>
        <textarea
          aria-label="Secret environment variable names"
          value={t.secret_env.join('\n')}
          oninput={(e) => (t.secret_env = lines(e.currentTarget.value))}
          rows="1"
          placeholder="SECRET_VAR_NAMES (values go in the keychain)"
          class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono"
        ></textarea>
        {#each t.secret_env as envName}
          <SecretField provider={`mcp.${server.name}.${envName}`} label={`${envName} (keychain)`} />
        {/each}
      {:else}
        {@const t = server.transport}
        <input aria-label="URL" bind:value={t.url} placeholder="https://… or http://localhost:…" class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono" />
        <label class="text-sm flex items-center gap-2">
          <input type="checkbox" bind:checked={t.bearer_token} class="w-4 h-4" /> Send a bearer token
        </label>
        {#if t.bearer_token}
          <SecretField provider={`mcp.${server.name}.token`} label="Bearer token (keychain)" />
        {/if}
      {/if}

      <textarea
        aria-label="Read-only tools"
        value={server.read_only_tools.join('\n')}
        oninput={(e) => (server.read_only_tools = lines(e.currentTarget.value))}
        rows="2"
        placeholder="tool names that only read data (run without a dialog), one per line"
        class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono"
      ></textarea>

      {#if status[server.name]}
        <p class="text-xs {status[server.name].ok ? 'text-green-400' : 'text-red-400'}">{status[server.name].text}</p>
      {/if}
    </div>
  {/each}

  <div class="flex gap-2">
    <button onclick={() => addServer('stdio')} class="glass-panel px-4 py-2 text-sm hover:bg-white/10">➕ Local (stdio) server</button>
    <button onclick={() => addServer('http')} class="glass-panel px-4 py-2 text-sm hover:bg-white/10">➕ HTTP server</button>
  </div>
</div>
