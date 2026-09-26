<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
  import { call, errorMessage } from '$lib/api';
  import type { Settings, VerifyReport } from '$lib/types';
  import { startRecording, transcribe } from '$lib/voice';
  import SecretField from './SecretField.svelte';
  import McpServers from './McpServers.svelte';

  let { initialTab = 'general' }: { initialTab?: string } = $props();
  // Seeded once from the prop so the page can deep-link (e.g. mic → Voice tab).
  let activeTab = $state(untrack(() => initialTab));
  let settings = $state<Settings | null>(null);
  let loadError = $state('');
  let isSaving = $state(false);
  let saveStatus = $state<{ type: 'success' | 'error' | null; message: string }>({ type: null, message: '' });
  let testingConnection = $state<string | null>(null);
  let statusTimer: ReturnType<typeof setTimeout> | undefined;
  let models = $state<string[]>([]);
  let modelsError = $state('');

  const tabs = [
    { id: 'general', label: 'General', icon: '⚙️' },
    { id: 'ai', label: 'AI Models', icon: '🤖' },
    { id: 'memresort', label: 'MemResort', icon: '🏰' },
    { id: 'integrations', label: 'MCP Servers', icon: '🔌' },
    { id: 'voice', label: 'Voice', icon: '🎤' },
    { id: 'memory', label: 'Memory', icon: '🧠' },
    { id: 'security', label: 'Security', icon: '🔒' },
    { id: 'performance', label: 'Performance', icon: '⚡' }
  ];

  onMount(loadSettings);
  onDestroy(() => clearTimeout(statusTimer));

  /** Show a status banner that clears itself after `ms`. */
  function flash(type: 'success' | 'error', message: string, ms = 4000) {
    saveStatus = { type, message };
    clearTimeout(statusTimer);
    statusTimer = setTimeout(() => (saveStatus = { type: null, message: '' }), ms);
  }

  async function loadSettings() {
    try {
      settings = await call<Settings>('load_settings');
      loadError = '';
      await refreshModels();
    } catch (e) {
      loadError = errorMessage(e);
    }
  }

  /** Discover available models (no model ids are hardcoded). */
  async function refreshModels() {
    if (!settings) return;
    modelsError = '';
    try {
      models =
        settings.ai.provider === 'ollama'
          ? await call<string[]>('list_ollama_models', { host: settings.ai.ollama_host })
          : await call<string[]>('list_models', { provider: settings.ai.provider });
    } catch (e) {
      models = [];
      modelsError = errorMessage(e);
    }
  }

  async function testMemory() {
    testingConnection = 'memory';
    try {
      flash('success', await call<string>('test_memory_backend'));
    } catch (e) {
      flash('error', errorMessage(e), 8000);
    } finally {
      testingConnection = null;
    }
  }

  // Microphone self-test: records a short clip against the *saved* settings
  // (the OS-level mic grant follows them), then transcribes it.
  let micTest = $state<{ phase: 'idle' | 'recording' | 'transcribing'; level: number; peak: number; result: string }>({
    phase: 'idle',
    level: 0,
    peak: 0,
    result: ''
  });
  const MIC_TEST_MS = 4000;

  async function testMicrophone() {
    if (micTest.phase !== 'idle') return;
    micTest = { phase: 'recording', level: 0, peak: 0, result: '' };
    try {
      const rec = await startRecording({
        onLevel: (l) => {
          micTest.level = l;
          micTest.peak = Math.max(micTest.peak, l);
        }
      });
      await new Promise((r) => setTimeout(r, MIC_TEST_MS));
      const { bytes, mime } = await rec.stop();
      if (micTest.peak < 0.05) {
        micTest.result = '⚠ The microphone opened but picked up almost no sound. Check the input device and its volume.';
        return;
      }
      micTest.phase = 'transcribing';
      const text = await transcribe(bytes, mime);
      micTest.result = text ? `✓ Heard: “${text}”` : '⚠ Audio reached the server but no words were recognized.';
    } catch (e) {
      micTest.result = `✗ ${errorMessage(e)}`;
    } finally {
      micTest.phase = 'idle';
      micTest.level = 0;
    }
  }

  async function saveSettings() {
    if (!settings) return;
    isSaving = true;
    try {
      settings = await call<Settings>('save_settings', { settings });
      flash('success', 'Settings saved.');
    } catch (e) {
      flash('error', `Not saved: ${errorMessage(e)}`, 8000);
    } finally {
      isSaving = false;
    }
  }

  async function testMemResort() {
    if (!settings) return;
    testingConnection = 'memresort';
    try {
      flash('success', await call<string>('test_memresort_connection', { config: settings.memresort }));
    } catch (e) {
      flash('error', errorMessage(e), 8000);
    } finally {
      testingConnection = null;
    }
  }

  async function testAIModel() {
    if (!settings) return;
    testingConnection = 'ai';
    try {
      flash('success', await call<string>('test_ai_model', { config: settings.ai }), 8000);
    } catch (e) {
      flash('error', errorMessage(e), 8000);
    } finally {
      testingConnection = null;
    }
  }

  async function exportSettings() {
    try {
      const path = await call<string | null>('export_settings');
      if (path) flash('success', `Exported to ${path} (API keys are never exported).`);
    } catch (e) {
      flash('error', `Export failed: ${errorMessage(e)}`, 8000);
    }
  }

  async function importSettings() {
    try {
      const imported = await call<Settings | null>('import_settings');
      if (imported) {
        settings = imported;
        flash('success', 'Settings imported.');
      }
    } catch (e) {
      flash('error', `Import failed: ${errorMessage(e)}`, 8000);
    }
  }

  async function resetToDefaults() {
    // UX confirmation only; any security-relevant change is also confirmed natively by the backend.
    if (!confirm('Reset all settings to defaults? API keys in the keychain are kept.')) return;
    try {
      settings = await call<Settings>('reset_settings');
      flash('success', 'Settings reset to defaults.');
    } catch (e) {
      flash('error', `Reset failed: ${errorMessage(e)}`, 8000);
    }
  }

  async function verifyAudit() {
    testingConnection = 'audit';
    try {
      const r = await call<VerifyReport>('verify_audit_log');
      if (r.valid) flash('success', `Audit log intact: ${r.entries} entries, hash chain verified.`, 8000);
      else flash('error', `Audit log FAILED verification at line ${r.first_bad_line}: ${r.reason}`, 15000);
    } catch (e) {
      flash('error', errorMessage(e), 8000);
    } finally {
      testingConnection = null;
    }
  }

  function lines(v: string): string[] {
    return v.split('\n').map((c) => c.trim()).filter(Boolean);
  }
</script>

<div class="h-full flex flex-col">
  <div class="flex items-center justify-between mb-6">
    <div>
      <h2 class="text-3xl font-bold glow-text">Settings</h2>
      <p class="text-gray-400 mt-1">Configure OMNIX to your preferences</p>
    </div>
    <div class="flex gap-2">
      <button onclick={exportSettings} class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm">📤 Export</button>
      <button onclick={importSettings} class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm">📥 Import</button>
      <button onclick={resetToDefaults} class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm text-red-400">🔄 Reset</button>
    </div>
  </div>

  {#if saveStatus.type}
    <div class="mb-4 glass-panel px-4 py-3 {saveStatus.type === 'success' ? 'bg-green-500/20' : 'bg-red-500/20'}" role="status">
      <div class="flex items-center gap-2">
        <span class="text-xl">{saveStatus.type === 'success' ? '✓' : '✗'}</span>
        <span class="text-sm">{saveStatus.message}</span>
      </div>
    </div>
  {/if}

  {#if loadError}
    <div class="glass-panel p-6 bg-red-500/20">Failed to load settings: {loadError}</div>
  {:else if !settings}
    <div class="glass-panel p-6 text-gray-400">Loading settings…</div>
  {:else}
    <div class="flex-1 flex gap-4 overflow-hidden">
      <div class="w-48 glass-panel p-4 space-y-2">
        {#each tabs as tab}
          <button
            onclick={() => (activeTab = tab.id)}
            class="w-full text-left px-3 py-2 rounded-lg transition-all flex items-center gap-2
                   {activeTab === tab.id ? 'bg-cosmic-blue/20 text-cosmic-cyan border border-cosmic-blue/50' : 'hover:bg-white/5 text-gray-300'}"
          >
            <span class="text-lg">{tab.icon}</span>
            <span class="text-sm font-medium">{tab.label}</span>
          </button>
        {/each}
      </div>

      <div class="flex-1 glass-panel p-6 overflow-auto">
        {#if activeTab === 'general'}
          <div class="space-y-6">
            <h3 class="text-xl font-bold text-cosmic-cyan mb-4">General Settings</h3>
            <div class="space-y-4">
              <div>
                <label for="theme" class="block text-sm font-medium mb-2">Theme</label>
                <select id="theme" bind:value={settings.general.theme} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2">
                  <option value="cosmic">Cosmic (Default)</option>
                  <option value="dark">Dark</option>
                  <option value="light">Light</option>
                  <option value="cyberpunk">Cyberpunk</option>
                </select>
              </div>
              <div>
                <label for="language" class="block text-sm font-medium mb-2">Language</label>
                <select id="language" bind:value={settings.general.language} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2">
                  <option value="en">English</option>
                  <option value="es">Spanish</option>
                  <option value="fr">French</option>
                  <option value="de">German</option>
                  <option value="ja">Japanese</option>
                  <option value="zh">Chinese</option>
                </select>
              </div>
              <label class="flex items-center justify-between">
                <span class="text-sm">Auto-start on login</span>
                <input type="checkbox" bind:checked={settings.general.auto_start} class="w-5 h-5" />
              </label>
              <label class="flex items-center justify-between">
                <span class="text-sm">Enable notifications</span>
                <input type="checkbox" bind:checked={settings.general.notifications} class="w-5 h-5" />
              </label>
              <label class="flex items-center justify-between">
                <span class="text-sm">Sound effects</span>
                <input type="checkbox" bind:checked={settings.general.sound_effects} class="w-5 h-5" />
              </label>
              <label class="flex items-center justify-between">
                <span class="text-sm">Minimize to system tray</span>
                <input type="checkbox" bind:checked={settings.general.minimize_to_tray} class="w-5 h-5" />
              </label>
              <label class="flex items-center justify-between">
                <span class="text-sm">Check for updates automatically</span>
                <input type="checkbox" bind:checked={settings.general.check_updates} class="w-5 h-5" />
              </label>
            </div>
          </div>

        {:else if activeTab === 'ai'}
          <div class="space-y-6">
            <div class="flex items-center justify-between mb-4">
              <h3 class="text-xl font-bold text-cosmic-cyan">AI Model Configuration</h3>
              <button onclick={testAIModel} disabled={testingConnection === 'ai'} class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm disabled:opacity-50">
                {testingConnection === 'ai' ? '⏳ Testing...' : '🧪 Test Connection'}
              </button>
            </div>

            {#if settings.security.local_only}
              <div class="glass-panel p-3 bg-cosmic-blue/10 border border-cosmic-blue/30 text-sm text-gray-300">
                🔒 Local-only mode is on: only Ollama on this machine or your private network can be used. Cloud providers are blocked (Settings → Security).
              </div>
            {/if}

            <div class="space-y-4">
              <div>
                <label for="provider" class="block text-sm font-medium mb-2">AI Provider</label>
                <select id="provider" bind:value={settings.ai.provider} onchange={() => { models = []; refreshModels(); }} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2">
                  <option value="ollama">Ollama (Local)</option>
                  <option value="openai" disabled={settings.security.local_only}>OpenAI</option>
                  <option value="anthropic" disabled={settings.security.local_only}>Anthropic (Claude)</option>
                  <option value="gemini" disabled={settings.security.local_only}>Google Gemini</option>
                  <option value="xai" disabled={settings.security.local_only}>xAI (Grok)</option>
                </select>
              </div>

              {#if settings.ai.provider === 'ollama'}
                <div>
                  <label for="ollama-host" class="block text-sm font-medium mb-2">Ollama Host</label>
                  <input id="ollama-host" type="text" bind:value={settings.ai.ollama_host} placeholder="http://localhost:11434" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
                </div>
                <div>
                  <label for="ollama-model" class="block text-sm font-medium mb-2">Model</label>
                  <div class="flex gap-2">
                    <select id="ollama-model" bind:value={settings.ai.ollama_model} class="flex-1 bg-white/5 border border-white/10 rounded-lg px-4 py-2">
                      <option value="">Select an installed model…</option>
                      {#each models as m}
                        <option value={m}>{m}</option>
                      {/each}
                      {#if settings.ai.ollama_model && !models.includes(settings.ai.ollama_model)}
                        <option value={settings.ai.ollama_model}>{settings.ai.ollama_model} (not found on server)</option>
                      {/if}
                    </select>
                    <button onclick={refreshModels} class="glass-panel px-3 py-2 text-sm hover:bg-white/10" title="Refresh installed models">↻</button>
                  </div>
                  {#if modelsError}<p class="text-xs text-red-400 mt-1">{modelsError}</p>{/if}
                  {#if !modelsError && models.length === 0}<p class="text-xs text-gray-400 mt-1">No models found. Run <code>ollama pull &lt;model&gt;</code>.</p>{/if}
                </div>
              {:else if settings.ai.provider === 'openai'}
                <SecretField provider="openai" label="OpenAI API Key" placeholder="sk-..." has={settings.ai.has_openai_key} onchange={(s) => { settings = s; refreshModels(); }} />
              {:else if settings.ai.provider === 'anthropic'}
                <SecretField provider="anthropic" label="Anthropic API Key" placeholder="sk-ant-..." has={settings.ai.has_anthropic_key} onchange={(s) => { settings = s; refreshModels(); }} />
                <p class="text-xs text-gray-400">Temperature is not sent to Anthropic: current Claude models reject sampling parameters.</p>
              {:else if settings.ai.provider === 'gemini'}
                <SecretField provider="gemini" label="Gemini API Key" placeholder="AIza..." has={settings.ai.has_gemini_key} onchange={(s) => { settings = s; refreshModels(); }} />
              {:else if settings.ai.provider === 'xai'}
                <SecretField provider="xai" label="xAI API Key" placeholder="xai-..." has={settings.ai.has_xai_key} onchange={(s) => { settings = s; refreshModels(); }} />
              {/if}

              {#if settings.ai.provider !== 'ollama'}
                <div>
                  <label for="cloud-model" class="block text-sm font-medium mb-2">Model</label>
                  <div class="flex gap-2">
                    <select id="cloud-model" bind:value={settings.ai.cloud_model} class="flex-1 bg-white/5 border border-white/10 rounded-lg px-4 py-2">
                      <option value="">Select a model…</option>
                      {#each models as m}
                        <option value={m}>{m}</option>
                      {/each}
                      {#if settings.ai.cloud_model && !models.includes(settings.ai.cloud_model)}
                        <option value={settings.ai.cloud_model}>{settings.ai.cloud_model}</option>
                      {/if}
                    </select>
                    <button onclick={refreshModels} class="glass-panel px-3 py-2 text-sm hover:bg-white/10" title="Refresh available models">↻</button>
                  </div>
                  {#if modelsError}<p class="text-xs text-red-400 mt-1">{modelsError}</p>{/if}
                  <p class="text-xs text-gray-400 mt-1">Models are listed from the provider's API after you save a key.</p>
                </div>
              {/if}

              <div>
                <label for="temperature" class="block text-sm font-medium mb-2">Temperature: {settings.ai.temperature}</label>
                <input id="temperature" type="range" bind:value={settings.ai.temperature} min="0" max="2" step="0.1" class="w-full" />
                <div class="flex justify-between text-xs text-gray-400 mt-1"><span>Precise</span><span>Creative</span></div>
              </div>
              <div>
                <label for="max-tokens" class="block text-sm font-medium mb-2">Max Tokens</label>
                <input id="max-tokens" type="number" bind:value={settings.ai.max_tokens} min="256" max="32768" step="256" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
              </div>
              <div>
                <label for="context-window" class="block text-sm font-medium mb-2">Context Window</label>
                <input id="context-window" type="number" bind:value={settings.ai.context_window} min="2048" max="128000" step="1024" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
              </div>
              <label class="flex items-center justify-between">
                <span class="text-sm">Stream responses</span>
                <input type="checkbox" bind:checked={settings.ai.stream_responses} class="w-5 h-5" />
              </label>
            </div>
          </div>

        {:else if activeTab === 'memresort'}
          <div class="space-y-6">
            <div class="flex items-center justify-between mb-4">
              <h3 class="text-xl font-bold text-cosmic-cyan">MemResort Memory Palace</h3>
              <button onclick={testMemResort} disabled={testingConnection === 'memresort'} class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm disabled:opacity-50">
                {testingConnection === 'memresort' ? '⏳ Testing...' : '🧪 Test Connection'}
              </button>
            </div>
            <div class="glass-panel p-4 bg-cosmic-blue/10 border border-cosmic-blue/30 text-sm text-gray-300">
              MemResort provides an OpenAI-compatible endpoint. OMNIX currently only checks connectivity to it.
            </div>
            <div class="space-y-4">
              <label class="flex items-center justify-between">
                <span class="text-sm font-medium">Enable MemResort Connection</span>
                <input type="checkbox" bind:checked={settings.memresort.enabled} class="w-5 h-5" />
              </label>
              {#if settings.memresort.enabled}
                <div>
                  <label for="mr-host" class="block text-sm font-medium mb-2">MemResort Host</label>
                  <input id="mr-host" type="text" bind:value={settings.memresort.host} placeholder="localhost or IP address" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
                </div>
                <div>
                  <label for="mr-port" class="block text-sm font-medium mb-2">Port</label>
                  <input id="mr-port" type="number" bind:value={settings.memresort.port} min="1" max="65535" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
                </div>
                <label class="flex items-center justify-between">
                  <span class="text-sm">Auto-connect on startup</span>
                  <input type="checkbox" bind:checked={settings.memresort.auto_connect} class="w-5 h-5" />
                </label>
                <div class="glass-panel p-4 bg-white/5 text-sm font-mono">
                  <div class="flex justify-between"><span class="text-gray-400">API Base:</span><span class="text-cosmic-cyan">http://{settings.memresort.host}:{settings.memresort.port}/v1</span></div>
                </div>
              {/if}
            </div>
          </div>

        {:else if activeTab === 'integrations'}
          <div class="space-y-6">
            <h3 class="text-xl font-bold text-cosmic-cyan mb-4">MCP Servers</h3>
            <McpServers bind:servers={settings.mcp.servers} />
          </div>

        {:else if activeTab === 'voice'}
          <div class="space-y-6">
            <h3 class="text-xl font-bold text-cosmic-cyan mb-4">Voice</h3>
            <div class="glass-panel p-4 bg-white/5 text-sm text-gray-300">
              Push-to-talk only: hold the mic button (or <kbd>Ctrl</kbd>+<kbd>Space</kbd>) to record, or tap it once to start and again to send. <kbd>Esc</kbd> cancels. Speech-to-text uses a faster-whisper server with an OpenAI-compatible <code>/v1/audio/transcriptions</code> endpoint; text-to-speech runs Piper locally.
            </div>
            <div class="space-y-4">
              <label class="flex items-center justify-between">
                <span class="text-sm font-medium">Enable voice input/output</span>
                <input type="checkbox" bind:checked={settings.voice.enabled} class="w-5 h-5" />
              </label>
              <div>
                <label for="stt-url" class="block text-sm font-medium mb-2">Speech-to-text server URL</label>
                <input id="stt-url" type="text" bind:value={settings.voice.stt_url} placeholder="http://localhost:8000 or http://whisper.tailnet:8000" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
                <p class="text-xs text-gray-400 mt-1">Recorded audio is sent only here (local-only mode requires a local/private address).</p>
              </div>
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label for="whisper" class="block text-sm font-medium mb-2">Whisper model</label>
                  <input id="whisper" type="text" bind:value={settings.voice.whisper_model} placeholder="base" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
                </div>
                <div>
                  <label for="voice-lang" class="block text-sm font-medium mb-2">Language</label>
                  <select id="voice-lang" bind:value={settings.voice.language} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2">
                    <option value="en">English</option>
                    <option value="es">Spanish</option>
                    <option value="fr">French</option>
                    <option value="de">German</option>
                    <option value="ja">Japanese</option>
                    <option value="zh">Chinese</option>
                  </select>
                </div>
              </div>
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label for="piper-path" class="block text-sm font-medium mb-2">Piper program</label>
                  <input id="piper-path" type="text" bind:value={settings.voice.piper_path} placeholder="piper" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 font-mono" />
                </div>
                <div>
                  <label for="tts-voice" class="block text-sm font-medium mb-2">Piper voice / model</label>
                  <input id="tts-voice" type="text" bind:value={settings.voice.tts_voice} placeholder="en_US-lessac-medium" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 font-mono" />
                </div>
              </div>
              <div class="glass-panel p-4 bg-white/5 space-y-3">
                <div class="flex items-center justify-between gap-4">
                  <div>
                    <div class="text-sm font-medium">Test microphone</div>
                    <p class="text-xs text-gray-400">Save first, then say a short sentence. Records {MIC_TEST_MS / 1000} seconds and transcribes it.</p>
                  </div>
                  <button
                    onclick={testMicrophone}
                    disabled={micTest.phase !== 'idle'}
                    class="px-4 py-2 bg-cosmic-blue/30 hover:bg-cosmic-blue/50 rounded-lg text-sm font-medium transition-all disabled:opacity-50 whitespace-nowrap"
                  >
                    {micTest.phase === 'recording' ? '🎙 Listening…' : micTest.phase === 'transcribing' ? '⏳ Transcribing…' : '🎙 Test microphone'}
                  </button>
                </div>
                <div class="h-2 bg-white/10 rounded-full overflow-hidden" aria-hidden="true">
                  <div class="h-full bg-gradient-to-r from-green-400 via-cosmic-cyan to-red-400 transition-[width] duration-75" style="width: {micTest.level * 100}%"></div>
                </div>
                {#if micTest.result}
                  <p class="text-sm {micTest.result.startsWith('✓') ? 'text-green-300' : micTest.result.startsWith('✗') ? 'text-red-300' : 'text-yellow-300'}">{micTest.result}</p>
                {/if}
              </div>
              <div class="flex items-center justify-between opacity-60">
                <span class="text-sm">Continuous listening</span>
                <span class="text-xs text-gray-400">Not offered (push-to-talk only)</span>
              </div>
            </div>
          </div>

        {:else if activeTab === 'memory'}
          <div class="space-y-6">
            <div class="flex items-center justify-between mb-4">
              <h3 class="text-xl font-bold text-cosmic-cyan">Memory & Knowledge Base</h3>
              <button onclick={testMemory} disabled={testingConnection === 'memory' || !settings.memory.backend_url} class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm disabled:opacity-50">
                {testingConnection === 'memory' ? '⏳ Testing...' : '🧪 Test Connection'}
              </button>
            </div>
            <div class="glass-panel p-4 bg-white/5 text-sm text-gray-300">
              Long-term memory uses the local <strong>kb-core</strong> service (set up with <code>./scripts/setup-memory.sh</code>, default <code>http://127.0.0.1:8100</code>). Leave the URL empty to keep memory disabled. Save settings before testing.
            </div>
            <div class="space-y-4">
              <div>
                <label for="kb-url" class="block text-sm font-medium mb-2">kb-core URL</label>
                <input id="kb-url" type="text" bind:value={settings.memory.backend_url} placeholder="http://127.0.0.1:8100 (empty = disabled)" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
              </div>
              <SecretField provider="kb_core" label="kb-core bearer token (only for a kb-core on another machine)" placeholder="token" />
              <label class="flex items-center justify-between gap-4">
                <span class="text-sm font-medium">Recall automatically <span class="block text-xs text-gray-400 font-normal">Before each reply, look up memories and notes relevant to your message and give them to the assistant</span></span>
                <input type="checkbox" bind:checked={settings.memory.auto_recall} disabled={!settings.memory.backend_url} class="w-5 h-5" />
              </label>
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label for="recall-limit" class="block text-sm font-medium mb-2">Entries recalled per message</label>
                  <input id="recall-limit" type="number" min="1" max="10" bind:value={settings.memory.recall_limit} disabled={!settings.memory.auto_recall} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 disabled:opacity-50" />
                </div>
                <div>
                  <label for="recall-min" class="block text-sm font-medium mb-2">Minimum relevance (0–1)</label>
                  <input id="recall-min" type="number" min="0" max="1" step="0.05" bind:value={settings.memory.recall_min_score} disabled={!settings.memory.auto_recall} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 disabled:opacity-50" />
                </div>
              </div>
              <label class="flex items-center justify-between gap-4">
                <span class="text-sm font-medium">Learn from conversations <span class="block text-xs text-gray-400 font-normal">After each reply, kb-core's local model saves lasting facts from <em>your</em> message (preferences, projects, people). Duplicates merge; outdated facts are superseded. Nothing leaves this machine. Off by default.</span></span>
                <input type="checkbox" bind:checked={settings.memory.auto_capture} disabled={!settings.memory.backend_url} class="w-5 h-5" />
              </label>
              <label class="flex items-center justify-between gap-4">
                <span class="text-sm font-medium">Archive conversations <span class="block text-xs text-gray-400 font-normal">Keep a searchable copy of your chats (your messages and OMNIX's replies, never tool output) in the "conversations" knowledge base, so you can ask "what did we discuss about …". Not used for automatic recall.</span></span>
                <input type="checkbox" bind:checked={settings.memory.archive_conversations} disabled={!settings.memory.backend_url} class="w-5 h-5" />
              </label>
              <label class="flex items-center justify-between gap-4">
                <span class="text-sm font-medium">Summarize conversations <span class="block text-xs text-gray-400 font-normal">When you clear a conversation (two or more messages), the chat model writes a short summary and saves it as a memory, so later chats can recall it. Marked as saved by the assistant.</span></span>
                <input type="checkbox" bind:checked={settings.memory.auto_summarize} disabled={!settings.memory.backend_url} class="w-5 h-5" />
              </label>
              <label class="flex items-center justify-between gap-4">
                <span class="text-sm font-medium">Semantic search <span class="block text-xs text-gray-400 font-normal">Find memories and notes by meaning as well as by words. Off: keyword matching only (no embedding model needed).</span></span>
                <input type="checkbox" bind:checked={settings.memory.enable_semantic_search} disabled={!settings.memory.backend_url} class="w-5 h-5" />
              </label>
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label for="max-memories" class="block text-sm font-medium mb-2">Memory limit <span class="text-xs text-gray-400 font-normal">(0 = unlimited)</span></label>
                  <input id="max-memories" type="number" min="0" max="1000000" bind:value={settings.memory.max_memory_size} disabled={!settings.memory.backend_url} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 disabled:opacity-50" />
                  <p class="text-xs text-gray-400 mt-1">When full, new memories are refused. Nothing is deleted for you.</p>
                </div>
                <div>
                  <label for="retention" class="block text-sm font-medium mb-2">Keep archived conversations (days) <span class="text-xs text-gray-400 font-normal">(0 = forever)</span></label>
                  <input id="retention" type="number" min="0" max="36500" bind:value={settings.memory.retention_days} disabled={!settings.memory.backend_url} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 disabled:opacity-50" />
                  <p class="text-xs text-gray-400 mt-1">Older chat transcripts are deleted daily. Memories and documents are never removed by age.</p>
                </div>
              </div>
            </div>
          </div>

        {:else if activeTab === 'security'}
          <div class="space-y-6">
            <div class="flex items-center justify-between mb-4">
              <h3 class="text-xl font-bold text-cosmic-cyan">Security Settings</h3>
              <button onclick={verifyAudit} disabled={testingConnection === 'audit'} class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm disabled:opacity-50">
                {testingConnection === 'audit' ? '⏳ Verifying...' : '🔏 Verify audit log'}
              </button>
            </div>
            <div class="glass-panel p-4 bg-white/5 text-sm text-gray-300">
              Commands that change your system always open a native confirmation dialog. Changes that weaken these settings are also confirmed natively when you save.
            </div>
            <div class="space-y-4">
              <label class="flex items-center justify-between gap-4">
                <span class="text-sm font-medium">Local-only mode <span class="block text-xs text-gray-400 font-normal">Blocks cloud AI providers and non-local endpoints (required on healthcare networks: no PHI egress)</span></span>
                <input type="checkbox" bind:checked={settings.security.local_only} class="w-5 h-5" />
              </label>
              <label class="flex items-center justify-between gap-4">
                <span class="text-sm font-medium">Allow privileged commands <span class="block text-xs text-gray-400 font-normal">Runs sudo-style commands through the OS password prompt (pkexec / macOS admin / UAC)</span></span>
                <input type="checkbox" bind:checked={settings.security.enable_sudo} class="w-5 h-5" />
              </label>
              <label class="flex items-center justify-between gap-4">
                <span class="text-sm font-medium">Also confirm read-only commands <span class="block text-xs text-gray-400 font-normal">Commands that modify anything are always confirmed</span></span>
                <input type="checkbox" bind:checked={settings.security.require_confirmation} class="w-5 h-5" />
              </label>
              <div class="flex items-center justify-between">
                <span class="text-sm font-medium">Audit log (hash-chained)</span>
                <span class="text-xs text-green-400">Always on</span>
              </div>
              <div class="flex items-center justify-between opacity-60">
                <span class="text-sm font-medium">Encrypt memory storage</span>
                <span class="text-xs bg-yellow-500/20 text-yellow-300 px-2 py-1 rounded">Planned</span>
              </div>
              <div>
                <label for="blocked" class="block text-sm font-medium mb-2">Blocked command prefixes (one per line)</label>
                <textarea
                  id="blocked"
                  value={settings.security.blocked_commands.join('\n')}
                  oninput={(e) => settings && (settings.security.blocked_commands = lines(e.currentTarget.value))}
                  rows="4"
                  placeholder="git push&#10;docker rm"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 font-mono text-sm"
                ></textarea>
                <p class="text-xs text-gray-400 mt-1">Added on top of the built-in rules (recursive root deletes, disk wipes, fork bombs, curl | sh, credential reads are always denied).</p>
              </div>
              <div>
                <label for="allowed" class="block text-sm font-medium mb-2">Allowlist (one per line; leave empty to disable)</label>
                <textarea
                  id="allowed"
                  value={settings.security.allowed_commands.join('\n')}
                  oninput={(e) => settings && (settings.security.allowed_commands = lines(e.currentTarget.value))}
                  rows="3"
                  placeholder="git status&#10;ls"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 font-mono text-sm"
                ></textarea>
                <p class="text-xs text-gray-400 mt-1">When set, only commands starting with one of these prefixes can run.</p>
              </div>
              <label class="flex items-center justify-between gap-4">
                <span class="text-sm font-medium">Autonomous agent mode <span class="block text-xs text-gray-400 font-normal">Let the AI chain several rounds of tool calls per message (each change still needs your approval)</span></span>
                <input type="checkbox" bind:checked={settings.security.autonomous_mode} class="w-5 h-5" />
              </label>
              {#if settings.security.autonomous_mode}
                <div>
                  <label for="max-steps" class="block text-sm font-medium mb-2">Max tool rounds per message</label>
                  <input id="max-steps" type="number" bind:value={settings.security.max_autonomous_steps} min="1" max="50" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
                </div>
              {/if}
              <div>
                <label for="loki" class="block text-sm font-medium mb-2">Ship audit log to Loki (optional)</label>
                <input id="loki" type="text" bind:value={settings.observability.loki_url} placeholder="http://loki:3100 (empty = off)" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
                <p class="text-xs text-gray-400 mt-1">Sends the already-redacted audit lines; anchors the hash chain off-machine.</p>
              </div>
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label for="cmd-timeout" class="block text-sm font-medium mb-2">Command timeout (s)</label>
                  <input id="cmd-timeout" type="number" bind:value={settings.security.command_timeout_secs} min="1" max="3600" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
                </div>
                <div>
                  <label for="confirm-timeout" class="block text-sm font-medium mb-2">Confirmation timeout (s)</label>
                  <input id="confirm-timeout" type="number" bind:value={settings.security.confirmation_timeout_secs} min="5" max="600" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
                </div>
              </div>
            </div>
          </div>

        {:else if activeTab === 'performance'}
          <div class="space-y-6">
            <h3 class="text-xl font-bold text-cosmic-cyan mb-4">Performance Optimization</h3>
            <div class="space-y-4">
              <div>
                <label for="concurrent" class="block text-sm font-medium mb-2">Max Concurrent Tasks</label>
                <input id="concurrent" type="number" bind:value={settings.performance.max_concurrent_tasks} min="1" max="20" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
              </div>
              <div>
                <label for="cache-size" class="block text-sm font-medium mb-2">Cache Size (MB)</label>
                <input id="cache-size" type="number" bind:value={settings.performance.cache_size} min="100" max="5000" step="100" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
              </div>
              <div>
                <label for="mon-interval" class="block text-sm font-medium mb-2">Monitoring Interval (ms)</label>
                <input id="mon-interval" type="number" bind:value={settings.performance.monitoring_interval} min="1000" max="60000" step="1000" class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2" />
              </div>
              <label class="flex items-center justify-between">
                <span class="text-sm">Enable caching</span>
                <input type="checkbox" bind:checked={settings.performance.cache_enabled} class="w-5 h-5" />
              </label>
              <label class="flex items-center justify-between">
                <span class="text-sm">Adaptive refresh rates</span>
                <input type="checkbox" bind:checked={settings.performance.adaptive_refresh} class="w-5 h-5" />
              </label>
              <label class="flex items-center justify-between">
                <span class="text-sm">Low power mode</span>
                <input type="checkbox" bind:checked={settings.performance.low_power_mode} class="w-5 h-5" />
              </label>
            </div>
          </div>
        {/if}
      </div>
    </div>

    <div class="mt-4 flex justify-end gap-3">
      <button
        onclick={saveSettings}
        disabled={isSaving}
        class="px-6 py-3 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg font-medium transition-all duration-200 flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {isSaving ? 'Saving…' : '💾 Save Settings'}
      </button>
    </div>
  {/if}
</div>

<style>
  input[type='checkbox'],
  input[type='range'] {
    accent-color: #00d4ff;
  }
</style>
