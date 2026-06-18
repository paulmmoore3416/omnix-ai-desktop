<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  
  let activeTab = $state('general');
  let settings = $state({
    general: {
      theme: 'cosmic',
      language: 'en',
      autoStart: false,
      notifications: true,
      soundEffects: true,
      minimizeToTray: true,
      checkUpdates: true
    },
    ai: {
      provider: 'ollama',
      ollamaHost: 'http://localhost:11434',
      ollamaModel: 'granite-code:8b',
      openaiKey: '',
      anthropicKey: '',
      geminiKey: '',
      xaiKey: '',
      temperature: 0.7,
      maxTokens: 2048,
      streamResponses: true,
      contextWindow: 8192
    },
    integrations: {
      github: {
        enabled: false,
        token: '',
        username: '',
        autoSync: false,
        syncInterval: 300
      },
      googleDrive: {
        enabled: false,
        apiKey: '',
        autoBackup: false,
        backupInterval: 3600
      },
      slack: {
        enabled: false,
        webhookUrl: '',
        notifyOnComplete: false
      },
      discord: {
        enabled: false,
        webhookUrl: '',
        notifyOnError: true
      },
      jira: {
        enabled: false,
        url: '',
        email: '',
        apiToken: '',
        autoCreateIssues: false
      },
      notion: {
        enabled: false,
        apiKey: '',
        databaseId: '',
        syncNotes: false
      }
    },
    memresort: {
      enabled: false,
      host: 'localhost',
      port: 8080,
      autoConnect: false
    },
    voice: {
      enabled: true,
      whisperModel: 'base',
      language: 'en',
      ttsEngine: 'piper',
      ttsVoice: 'en_US-lessac-medium',
      wakeWord: 'omnix',
      continuousListening: false
    },
    memory: {
      vectorDb: 'chroma',
      maxMemorySize: 1000,
      autoSummarize: true,
      retentionDays: 90,
      enableSemanticSearch: true,
      embeddingModel: 'all-MiniLM-L6-v2'
    },
    security: {
      requireConfirmation: true,
      enableSudo: true,
      logAllCommands: true,
      encryptMemory: true,
      auditLog: true,
      allowedCommands: [],
      blockedCommands: ['rm -rf /', 'dd if=', 'mkfs']
    },
    performance: {
      maxConcurrentTasks: 5,
      cacheEnabled: true,
      cacheSize: 500,
      monitoringInterval: 5000,
      adaptiveRefresh: true,
      lowPowerMode: false
    }
  });
  
  let isSaving = $state(false);
  let saveStatus = $state<{type: 'success' | 'error' | null, message: string}>({type: null, message: ''});
  let testingConnection = $state<string | null>(null);
  
  const tabs = [
    { id: 'general', label: 'General', icon: '⚙️' },
    { id: 'ai', label: 'AI Models', icon: '🤖' },
    { id: 'memresort', label: 'MemResort', icon: '🏰' },
    { id: 'integrations', label: 'Integrations', icon: '🔌' },
    { id: 'voice', label: 'Voice', icon: '🎤' },
    { id: 'memory', label: 'Memory', icon: '🧠' },
    { id: 'security', label: 'Security', icon: '🔒' },
    { id: 'performance', label: 'Performance', icon: '⚡' }
  ];
  
  onMount(async () => {
    await loadSettings();
  });
  
  async function loadSettings() {
    try {
      const loaded = await invoke('load_settings');
      settings = { ...settings, ...loaded as typeof settings };
    } catch (error) {
      console.error('Failed to load settings:', error);
    }
  }
  
  async function saveSettings() {
    isSaving = true;
    saveStatus = {type: null, message: ''};
    
    try {
      await invoke('save_settings', { settings });
      saveStatus = {type: 'success', message: 'Settings saved successfully!'};
      setTimeout(() => saveStatus = {type: null, message: ''}, 3000);
    } catch (error) {
      saveStatus = {type: 'error', message: `Failed to save: ${error}`};
    } finally {
      isSaving = false;
    }
  }
  
  async function testConnection(service: string) {
    testingConnection = service;
    try {
      if (service === 'memresort') {
        const result = await invoke('test_memresort_connection', { config: settings.memresort });
        saveStatus = {type: 'success', message: `MemResort connection successful!`};
      } else {
        const result = await invoke('test_integration', { service, config: settings.integrations[service as keyof typeof settings.integrations] });
        saveStatus = {type: 'success', message: `${service} connection successful!`};
      }
      setTimeout(() => saveStatus = {type: null, message: ''}, 3000);
    } catch (error) {
      saveStatus = {type: 'error', message: `${service} connection failed: ${error}`};
    } finally {
      testingConnection = null;
    }
  }
  
  async function testAIModel() {
    testingConnection = 'ai';
    try {
      const result = await invoke('test_ai_model', { config: settings.ai });
      saveStatus = {type: 'success', message: 'AI model connection successful!'};
      setTimeout(() => saveStatus = {type: null, message: ''}, 3000);
    } catch (error) {
      saveStatus = {type: 'error', message: `AI model test failed: ${error}`};
    } finally {
      testingConnection = null;
    }
  }
  
  async function exportSettings() {
    try {
      await invoke('export_settings', { settings });
      saveStatus = {type: 'success', message: 'Settings exported successfully!'};
      setTimeout(() => saveStatus = {type: null, message: ''}, 3000);
    } catch (error) {
      saveStatus = {type: 'error', message: `Export failed: ${error}`};
    }
  }
  
  async function importSettings() {
    try {
      const imported = await invoke('import_settings');
      settings = { ...settings, ...imported as typeof settings };
      saveStatus = {type: 'success', message: 'Settings imported successfully!'};
      setTimeout(() => saveStatus = {type: null, message: ''}, 3000);
    } catch (error) {
      saveStatus = {type: 'error', message: `Import failed: ${error}`};
    }
  }
  
  async function resetToDefaults() {
    if (confirm('Are you sure you want to reset all settings to defaults?')) {
      try {
        await invoke('reset_settings');
        await loadSettings();
        saveStatus = {type: 'success', message: 'Settings reset to defaults!'};
        setTimeout(() => saveStatus = {type: null, message: ''}, 3000);
      } catch (error) {
        saveStatus = {type: 'error', message: `Reset failed: ${error}`};
      }
    }
  }
</script>

<div class="h-full flex flex-col">
  <!-- Header -->
  <div class="flex items-center justify-between mb-6">
    <div>
      <h2 class="text-3xl font-bold glow-text">Settings</h2>
      <p class="text-gray-400 mt-1">Configure OMNIX to your preferences</p>
    </div>
    <div class="flex gap-2">
      <button
        onclick={exportSettings}
        class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm"
      >
        📤 Export
      </button>
      <button
        onclick={importSettings}
        class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm"
      >
        📥 Import
      </button>
      <button
        onclick={resetToDefaults}
        class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm text-red-400"
      >
        🔄 Reset
      </button>
    </div>
  </div>
  
  <!-- Status Message -->
  {#if saveStatus.type}
    <div class="mb-4 glass-panel px-4 py-3 {saveStatus.type === 'success' ? 'bg-green-500/20' : 'bg-red-500/20'} animate-slide-in">
      <div class="flex items-center gap-2">
        <span class="text-xl">{saveStatus.type === 'success' ? '✓' : '✗'}</span>
        <span class="text-sm">{saveStatus.message}</span>
      </div>
    </div>
  {/if}
  
  <div class="flex-1 flex gap-4 overflow-hidden">
    <!-- Tabs Sidebar -->
    <div class="w-48 glass-panel p-4 space-y-2">
      {#each tabs as tab}
        <button
          onclick={() => activeTab = tab.id}
          class="w-full text-left px-3 py-2 rounded-lg transition-all flex items-center gap-2
                 {activeTab === tab.id ? 'bg-cosmic-blue/20 text-cosmic-cyan border border-cosmic-blue/50' : 'hover:bg-white/5 text-gray-300'}"
        >
          <span class="text-lg">{tab.icon}</span>
          <span class="text-sm font-medium">{tab.label}</span>
        </button>
      {/each}
    </div>
    
    <!-- Settings Content -->
    <div class="flex-1 glass-panel p-6 overflow-auto">
      {#if activeTab === 'general'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan mb-4">General Settings</h3>
          
          <div class="space-y-4">
            <div>
              <label class="block text-sm font-medium mb-2">Theme</label>
              <select bind:value={settings.general.theme} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2">
                <option value="cosmic">Cosmic (Default)</option>
                <option value="dark">Dark</option>
                <option value="light">Light</option>
                <option value="cyberpunk">Cyberpunk</option>
              </select>
            </div>
            
            <div>
              <label class="block text-sm font-medium mb-2">Language</label>
              <select bind:value={settings.general.language} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2">
                <option value="en">English</option>
                <option value="es">Spanish</option>
                <option value="fr">French</option>
                <option value="de">German</option>
                <option value="ja">Japanese</option>
                <option value="zh">Chinese</option>
              </select>
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm">Auto-start on system boot</span>
              <input type="checkbox" bind:checked={settings.general.autoStart} class="w-5 h-5" />
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm">Enable notifications</span>
              <input type="checkbox" bind:checked={settings.general.notifications} class="w-5 h-5" />
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm">Sound effects</span>
              <input type="checkbox" bind:checked={settings.general.soundEffects} class="w-5 h-5" />
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm">Minimize to system tray</span>
              <input type="checkbox" bind:checked={settings.general.minimizeToTray} class="w-5 h-5" />
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm">Check for updates automatically</span>
              <input type="checkbox" bind:checked={settings.general.checkUpdates} class="w-5 h-5" />
            </div>
          </div>
        </div>
        
      {:else if activeTab === 'ai'}
        <div class="space-y-6">
          <div class="flex items-center justify-between mb-4">
            <h3 class="text-xl font-bold text-cosmic-cyan">AI Model Configuration</h3>
            <button
              onclick={testAIModel}
              disabled={testingConnection === 'ai'}
              class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm disabled:opacity-50"
            >
              {testingConnection === 'ai' ? '⏳ Testing...' : '🧪 Test Connection'}
            </button>
          </div>
          
          <div class="space-y-4">
            <div>
              <label class="block text-sm font-medium mb-2">AI Provider</label>
              <select bind:value={settings.ai.provider} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2">
                <option value="ollama">Ollama (Local)</option>
                <option value="openai">OpenAI</option>
                <option value="anthropic">Anthropic (Claude)</option>
                <option value="gemini">Google Gemini</option>
                <option value="xai">xAI (Grok)</option>
              </select>
            </div>
            
            {#if settings.ai.provider === 'ollama'}
              <div>
                <label class="block text-sm font-medium mb-2">Ollama Host</label>
                <input
                  type="text"
                  bind:value={settings.ai.ollamaHost}
                  placeholder="http://localhost:11434"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
                />
              </div>
              
              <div>
                <label class="block text-sm font-medium mb-2">Model</label>
                <input
                  type="text"
                  bind:value={settings.ai.ollamaModel}
                  placeholder="granite-code:8b"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
                />
              </div>
            {:else if settings.ai.provider === 'openai'}
              <div>
                <label class="block text-sm font-medium mb-2">OpenAI API Key</label>
                <input
                  type="password"
                  bind:value={settings.ai.openaiKey}
                  placeholder="sk-..."
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
                />
              </div>
            {:else if settings.ai.provider === 'anthropic'}
              <div>
                <label class="block text-sm font-medium mb-2">Anthropic API Key</label>
                <input
                  type="password"
                  bind:value={settings.ai.anthropicKey}
                  placeholder="sk-ant-..."
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
                />
              </div>
            {:else if settings.ai.provider === 'gemini'}
              <div>
                <label class="block text-sm font-medium mb-2">Gemini API Key</label>
                <input
                  type="password"
                  bind:value={settings.ai.geminiKey}
                  placeholder="AIza..."
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
                />
              </div>
            {:else if settings.ai.provider === 'xai'}
              <div>
                <label class="block text-sm font-medium mb-2">xAI API Key</label>
                <input
                  type="password"
                  bind:value={settings.ai.xaiKey}
                  placeholder="xai-..."
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
                />
              </div>
            {/if}
            
            <div>
              <label class="block text-sm font-medium mb-2">Temperature: {settings.ai.temperature}</label>
              <input
                type="range"
                bind:value={settings.ai.temperature}
                min="0"
                max="2"
                step="0.1"
                class="w-full"
              />
              <div class="flex justify-between text-xs text-gray-400 mt-1">
                <span>Precise</span>
                <span>Creative</span>
              </div>
            </div>
            
            <div>
              <label class="block text-sm font-medium mb-2">Max Tokens</label>
              <input
                type="number"
                bind:value={settings.ai.maxTokens}
                min="256"
                max="32768"
                step="256"
                class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
              />
            </div>
            
            <div>
              <label class="block text-sm font-medium mb-2">Context Window</label>
              <input
                type="number"
                bind:value={settings.ai.contextWindow}
                min="2048"
                max="128000"
                step="1024"
                class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
              />
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm">Stream responses</span>
              <input type="checkbox" bind:checked={settings.ai.streamResponses} class="w-5 h-5" />
            </div>
          </div>
        </div>
        
      {:else if activeTab === 'memresort'}
        <div class="space-y-6">
          <div class="flex items-center justify-between mb-4">
            <h3 class="text-xl font-bold text-cosmic-cyan">MemResort Memory Palace</h3>
            <button
              onclick={() => testConnection('memresort')}
              disabled={testingConnection === 'memresort'}
              class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm disabled:opacity-50"
            >
              {testingConnection === 'memresort' ? '⏳ Testing...' : '🧪 Test Connection'}
            </button>
          </div>
          
          <div class="glass-panel p-4 bg-cosmic-blue/10 border border-cosmic-blue/30">
            <div class="flex items-start gap-3">
              <span class="text-2xl">ℹ️</span>
              <div class="text-sm text-gray-300">
                <p class="font-medium mb-2">Connect to MemResort Memory Palace</p>
                <p class="text-gray-400">
                  MemResort provides an OpenAI-compatible endpoint for AI models to access your memory palace.
                  Configure the connection below to enable remote AI models to use your knowledge base.
                </p>
              </div>
            </div>
          </div>
          
          <div class="space-y-4">
            <div class="flex items-center justify-between">
              <span class="text-sm font-medium">Enable MemResort Connection</span>
              <input type="checkbox" bind:checked={settings.memresort.enabled} class="w-5 h-5" />
            </div>
            
            {#if settings.memresort.enabled}
              <div>
                <label class="block text-sm font-medium mb-2">MemResort Host IP</label>
                <input
                  type="text"
                  bind:value={settings.memresort.host}
                  placeholder="localhost or IP address"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
                />
                <p class="text-xs text-gray-400 mt-1">IP address of the machine running MemResort</p>
              </div>
              
              <div>
                <label class="block text-sm font-medium mb-2">Port</label>
                <input
                  type="number"
                  bind:value={settings.memresort.port}
                  placeholder="8080"
                  min="1"
                  max="65535"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
                />
                <p class="text-xs text-gray-400 mt-1">Default: 8080</p>
              </div>
              
              <div class="flex items-center justify-between">
                <span class="text-sm">Auto-connect on startup</span>
                <input type="checkbox" bind:checked={settings.memresort.autoConnect} class="w-5 h-5" />
              </div>
              
              <div class="glass-panel p-4 bg-white/5 mt-4">
                <h4 class="font-medium mb-3 text-cosmic-cyan">Connection Information</h4>
                <div class="space-y-2 text-sm font-mono">
                  <div class="flex justify-between">
                    <span class="text-gray-400">Endpoint URL:</span>
                    <span class="text-cosmic-cyan">http://{settings.memresort.host}:{settings.memresort.port}/v1/chat/completions</span>
                  </div>
                  <div class="flex justify-between">
                    <span class="text-gray-400">API Base:</span>
                    <span class="text-cosmic-cyan">http://{settings.memresort.host}:{settings.memresort.port}/v1</span>
                  </div>
                  <div class="flex justify-between">
                    <span class="text-gray-400">API Key:</span>
                    <span class="text-gray-500">not-needed</span>
                  </div>
                </div>
              </div>
              
              <div class="glass-panel p-4 bg-white/5 mt-4">
                <h4 class="font-medium mb-3">📋 Usage Examples</h4>
                <div class="space-y-3 text-xs">
                  <div>
                    <p class="text-gray-400 mb-1">Environment Variables:</p>
                    <pre class="bg-black/30 p-2 rounded overflow-x-auto">export OPENAI_API_BASE="http://{settings.memresort.host}:{settings.memresort.port}/v1"
export OPENAI_API_KEY="not-needed"</pre>
                  </div>
                  
                  <div>
                    <p class="text-gray-400 mb-1">cURL Test:</p>
                    <pre class="bg-black/30 p-2 rounded overflow-x-auto">curl http://{settings.memresort.host}:{settings.memresort.port}/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '&#123;"model": "llama2", "messages": [&#123;"role": "user", "content": "Hello"&#125;]&#125;'</pre>
                  </div>
                </div>
              </div>
            {/if}
          </div>
        </div>
        
      {:else if activeTab === 'integrations'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan mb-4">External Integrations</h3>
          
          <!-- GitHub -->
          <div class="glass-panel p-4 bg-white/5">
            <div class="flex items-center justify-between mb-4">
              <div class="flex items-center gap-3">
                <span class="text-2xl">🐙</span>
                <div>
                  <h4 class="font-bold">GitHub</h4>
                  <p class="text-xs text-gray-400">Repository management and automation</p>
                </div>
              </div>
              <div class="flex items-center gap-2">
                <input type="checkbox" bind:checked={settings.integrations.github.enabled} class="w-5 h-5" />
                {#if settings.integrations.github.enabled}
                  <button
                    onclick={() => testConnection('github')}
                    disabled={testingConnection === 'github'}
                    class="glass-panel px-3 py-1 hover:bg-white/10 transition-all text-xs"
                  >
                    {testingConnection === 'github' ? '⏳' : '🧪 Test'}
                  </button>
                {/if}
              </div>
            </div>
            
            {#if settings.integrations.github.enabled}
              <div class="space-y-3">
                <input
                  type="password"
                  bind:value={settings.integrations.github.token}
                  placeholder="GitHub Personal Access Token"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                />
                <input
                  type="text"
                  bind:value={settings.integrations.github.username}
                  placeholder="GitHub Username"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                />
                <div class="flex items-center justify-between text-sm">
                  <span>Auto-sync repositories</span>
                  <input type="checkbox" bind:checked={settings.integrations.github.autoSync} class="w-4 h-4" />
                </div>
                {#if settings.integrations.github.autoSync}
                  <div>
                    <label class="block text-xs text-gray-400 mb-1">Sync interval (seconds)</label>
                    <input
                      type="number"
                      bind:value={settings.integrations.github.syncInterval}
                      min="60"
                      class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                    />
                  </div>
                {/if}
              </div>
            {/if}
          </div>
          
          <!-- Google Drive -->
          <div class="glass-panel p-4 bg-white/5">
            <div class="flex items-center justify-between mb-4">
              <div class="flex items-center gap-3">
                <span class="text-2xl">📁</span>
                <div>
                  <h4 class="font-bold">Google Drive</h4>
                  <p class="text-xs text-gray-400">Cloud storage and backup</p>
                </div>
              </div>
              <div class="flex items-center gap-2">
                <input type="checkbox" bind:checked={settings.integrations.googleDrive.enabled} class="w-5 h-5" />
                {#if settings.integrations.googleDrive.enabled}
                  <button
                    onclick={() => testConnection('googleDrive')}
                    disabled={testingConnection === 'googleDrive'}
                    class="glass-panel px-3 py-1 hover:bg-white/10 transition-all text-xs"
                  >
                    {testingConnection === 'googleDrive' ? '⏳' : '🧪 Test'}
                  </button>
                {/if}
              </div>
            </div>
            
            {#if settings.integrations.googleDrive.enabled}
              <div class="space-y-3">
                <input
                  type="password"
                  bind:value={settings.integrations.googleDrive.apiKey}
                  placeholder="Google Drive API Key"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                />
                <div class="flex items-center justify-between text-sm">
                  <span>Auto-backup</span>
                  <input type="checkbox" bind:checked={settings.integrations.googleDrive.autoBackup} class="w-4 h-4" />
                </div>
                {#if settings.integrations.googleDrive.autoBackup}
                  <div>
                    <label class="block text-xs text-gray-400 mb-1">Backup interval (seconds)</label>
                    <input
                      type="number"
                      bind:value={settings.integrations.googleDrive.backupInterval}
                      min="300"
                      class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                    />
                  </div>
                {/if}
              </div>
            {/if}
          </div>
          
          <!-- Slack -->
          <div class="glass-panel p-4 bg-white/5">
            <div class="flex items-center justify-between mb-4">
              <div class="flex items-center gap-3">
                <span class="text-2xl">💬</span>
                <div>
                  <h4 class="font-bold">Slack</h4>
                  <p class="text-xs text-gray-400">Team notifications</p>
                </div>
              </div>
              <input type="checkbox" bind:checked={settings.integrations.slack.enabled} class="w-5 h-5" />
            </div>
            
            {#if settings.integrations.slack.enabled}
              <div class="space-y-3">
                <input
                  type="text"
                  bind:value={settings.integrations.slack.webhookUrl}
                  placeholder="Slack Webhook URL"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                />
                <div class="flex items-center justify-between text-sm">
                  <span>Notify on task completion</span>
                  <input type="checkbox" bind:checked={settings.integrations.slack.notifyOnComplete} class="w-4 h-4" />
                </div>
              </div>
            {/if}
          </div>
          
          <!-- Discord -->
          <div class="glass-panel p-4 bg-white/5">
            <div class="flex items-center justify-between mb-4">
              <div class="flex items-center gap-3">
                <span class="text-2xl">🎮</span>
                <div>
                  <h4 class="font-bold">Discord</h4>
                  <p class="text-xs text-gray-400">Community notifications</p>
                </div>
              </div>
              <input type="checkbox" bind:checked={settings.integrations.discord.enabled} class="w-5 h-5" />
            </div>
            
            {#if settings.integrations.discord.enabled}
              <div class="space-y-3">
                <input
                  type="text"
                  bind:value={settings.integrations.discord.webhookUrl}
                  placeholder="Discord Webhook URL"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                />
                <div class="flex items-center justify-between text-sm">
                  <span>Notify on errors</span>
                  <input type="checkbox" bind:checked={settings.integrations.discord.notifyOnError} class="w-4 h-4" />
                </div>
              </div>
            {/if}
          </div>
          
          <!-- Jira -->
          <div class="glass-panel p-4 bg-white/5">
            <div class="flex items-center justify-between mb-4">
              <div class="flex items-center gap-3">
                <span class="text-2xl">📋</span>
                <div>
                  <h4 class="font-bold">Jira</h4>
                  <p class="text-xs text-gray-400">Project management integration</p>
                </div>
              </div>
              <input type="checkbox" bind:checked={settings.integrations.jira.enabled} class="w-5 h-5" />
            </div>
            
            {#if settings.integrations.jira.enabled}
              <div class="space-y-3">
                <input
                  type="text"
                  bind:value={settings.integrations.jira.url}
                  placeholder="Jira URL (e.g., https://yourcompany.atlassian.net)"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                />
                <input
                  type="email"
                  bind:value={settings.integrations.jira.email}
                  placeholder="Email"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                />
                <input
                  type="password"
                  bind:value={settings.integrations.jira.apiToken}
                  placeholder="API Token"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                />
                <div class="flex items-center justify-between text-sm">
                  <span>Auto-create issues from errors</span>
                  <input type="checkbox" bind:checked={settings.integrations.jira.autoCreateIssues} class="w-4 h-4" />
                </div>
              </div>
            {/if}
          </div>
          
          <!-- Notion -->
          <div class="glass-panel p-4 bg-white/5">
            <div class="flex items-center justify-between mb-4">
              <div class="flex items-center gap-3">
                <span class="text-2xl">📝</span>
                <div>
                  <h4 class="font-bold">Notion</h4>
                  <p class="text-xs text-gray-400">Knowledge base sync</p>
                </div>
              </div>
              <input type="checkbox" bind:checked={settings.integrations.notion.enabled} class="w-5 h-5" />
            </div>
            
            {#if settings.integrations.notion.enabled}
              <div class="space-y-3">
                <input
                  type="password"
                  bind:value={settings.integrations.notion.apiKey}
                  placeholder="Notion API Key"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                />
                <input
                  type="text"
                  bind:value={settings.integrations.notion.databaseId}
                  placeholder="Database ID"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                />
                <div class="flex items-center justify-between text-sm">
                  <span>Sync notes automatically</span>
                  <input type="checkbox" bind:checked={settings.integrations.notion.syncNotes} class="w-4 h-4" />
                </div>
              </div>
            {/if}
          </div>
        </div>
        
      {:else if activeTab === 'voice'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan mb-4">Voice Configuration</h3>
          
          <div class="space-y-4">
            <div class="flex items-center justify-between">
              <span class="text-sm font-medium">Enable voice input/output</span>
              <input type="checkbox" bind:checked={settings.voice.enabled} class="w-5 h-5" />
            </div>
            
            {#if settings.voice.enabled}
              <div>
                <label class="block text-sm font-medium mb-2">Whisper Model</label>
                <select bind:value={settings.voice.whisperModel} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2">
                  <option value="tiny">Tiny (Fastest)</option>
                  <option value="base">Base (Balanced)</option>
                  <option value="small">Small (Better)</option>
                  <option value="medium">Medium (Best)</option>
                </select>
              </div>
              
              <div>
                <label class="block text-sm font-medium mb-2">Language</label>
                <select bind:value={settings.voice.language} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2">
                  <option value="en">English</option>
                  <option value="es">Spanish</option>
                  <option value="fr">French</option>
                  <option value="de">German</option>
                  <option value="ja">Japanese</option>
                  <option value="zh">Chinese</option>
                </select>
              </div>
              
              <div>
                <label class="block text-sm font-medium mb-2">TTS Engine</label>
                <select bind:value={settings.voice.ttsEngine} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2">
                  <option value="piper">Piper (Local)</option>
                  <option value="elevenlabs">ElevenLabs (Cloud)</option>
                  <option value="google">Google TTS</option>
                </select>
              </div>
              
              <div>
                <label class="block text-sm font-medium mb-2">Voice</label>
                <input
                  type="text"
                  bind:value={settings.voice.ttsVoice}
                  placeholder="en_US-lessac-medium"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
                />
              </div>
              
              <div>
                <label class="block text-sm font-medium mb-2">Wake Word</label>
                <input
                  type="text"
                  bind:value={settings.voice.wakeWord}
                  placeholder="omnix"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
                />
              </div>
              
              <div class="flex items-center justify-between">
                <span class="text-sm">Continuous listening mode</span>
                <input type="checkbox" bind:checked={settings.voice.continuousListening} class="w-5 h-5" />
              </div>
            {/if}
          </div>
        </div>
        
      {:else if activeTab === 'memory'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan mb-4">Memory & Knowledge Base</h3>
          
          <div class="space-y-4">
            <div>
              <label class="block text-sm font-medium mb-2">Vector Database</label>
              <select bind:value={settings.memory.vectorDb} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2">
                <option value="chroma">ChromaDB (Local)</option>
                <option value="pinecone">Pinecone (Cloud)</option>
                <option value="weaviate">Weaviate</option>
                <option value="qdrant">Qdrant</option>
              </select>
            </div>
            
            <div>
              <label class="block text-sm font-medium mb-2">Max Memory Size (entries)</label>
              <input
                type="number"
                bind:value={settings.memory.maxMemorySize}
                min="100"
                max="10000"
                step="100"
                class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
              />
            </div>
            
            <div>
              <label class="block text-sm font-medium mb-2">Retention Period (days)</label>
              <input
                type="number"
                bind:value={settings.memory.retentionDays}
                min="7"
                max="365"
                class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
              />
            </div>
            
            <div>
              <label class="block text-sm font-medium mb-2">Embedding Model</label>
              <select bind:value={settings.memory.embeddingModel} class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2">
                <option value="all-MiniLM-L6-v2">all-MiniLM-L6-v2 (Fast)</option>
                <option value="all-mpnet-base-v2">all-mpnet-base-v2 (Balanced)</option>
                <option value="text-embedding-ada-002">OpenAI Ada-002 (Best)</option>
              </select>
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm">Auto-summarize conversations</span>
              <input type="checkbox" bind:checked={settings.memory.autoSummarize} class="w-5 h-5" />
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm">Enable semantic search</span>
              <input type="checkbox" bind:checked={settings.memory.enableSemanticSearch} class="w-5 h-5" />
            </div>
          </div>
        </div>
        
      {:else if activeTab === 'security'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan mb-4">Security Settings</h3>
          
          <div class="space-y-4">
            <div class="flex items-center justify-between">
              <span class="text-sm font-medium">Require confirmation for commands</span>
              <input type="checkbox" bind:checked={settings.security.requireConfirmation} class="w-5 h-5" />
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm font-medium">Enable sudo privileges</span>
              <input type="checkbox" bind:checked={settings.security.enableSudo} class="w-5 h-5" />
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm font-medium">Log all commands</span>
              <input type="checkbox" bind:checked={settings.security.logAllCommands} class="w-5 h-5" />
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm font-medium">Encrypt memory storage</span>
              <input type="checkbox" bind:checked={settings.security.encryptMemory} class="w-5 h-5" />
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm font-medium">Enable audit logging</span>
              <input type="checkbox" bind:checked={settings.security.auditLog} class="w-5 h-5" />
            </div>
            
            <div>
              <label class="block text-sm font-medium mb-2">Blocked Commands (one per line)</label>
              <textarea
                value={settings.security.blockedCommands.join('\n')}
                oninput={(e) => settings.security.blockedCommands = e.currentTarget.value.split('\n').filter(c => c.trim())}
                rows="5"
                placeholder="rm -rf /&#10;dd if=&#10;mkfs"
                class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 font-mono text-sm"
              ></textarea>
              <p class="text-xs text-gray-400 mt-1">Commands that will be blocked from execution</p>
            </div>
          </div>
        </div>
        
      {:else if activeTab === 'performance'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan mb-4">Performance Optimization</h3>
          
          <div class="space-y-4">
            <div>
              <label class="block text-sm font-medium mb-2">Max Concurrent Tasks</label>
              <input
                type="number"
                bind:value={settings.performance.maxConcurrentTasks}
                min="1"
                max="20"
                class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
              />
            </div>
            
            <div>
              <label class="block text-sm font-medium mb-2">Cache Size (MB)</label>
              <input
                type="number"
                bind:value={settings.performance.cacheSize}
                min="100"
                max="5000"
                step="100"
                class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
              />
            </div>
            
            <div>
              <label class="block text-sm font-medium mb-2">Monitoring Interval (ms)</label>
              <input
                type="number"
                bind:value={settings.performance.monitoringInterval}
                min="1000"
                max="60000"
                step="1000"
                class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2"
              />
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm">Enable caching</span>
              <input type="checkbox" bind:checked={settings.performance.cacheEnabled} class="w-5 h-5" />
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm">Adaptive refresh rates</span>
              <input type="checkbox" bind:checked={settings.performance.adaptiveRefresh} class="w-5 h-5" />
            </div>
            
            <div class="flex items-center justify-between">
              <span class="text-sm">Low power mode</span>
              <input type="checkbox" bind:checked={settings.performance.lowPowerMode} class="w-5 h-5" />
            </div>
          </div>
        </div>
      {/if}
    </div>
  </div>
  
  <!-- Save Button -->
  <div class="mt-4 flex justify-end gap-3">
    <button
      onclick={saveSettings}
      disabled={isSaving}
      class="px-6 py-3 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg font-medium transition-all duration-200 flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed hover:scale-105 active:scale-95"
    >
      {#if isSaving}
        <svg class="animate-spin h-5 w-5" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
        </svg>
        <span>Saving...</span>
      {:else}
        <span>💾 Save Settings</span>
      {/if}
    </button>
  </div>
</div>

<style>
  input[type="checkbox"] {
    accent-color: #00d4ff;
  }
  
  input[type="range"] {
    accent-color: #00d4ff;
  }
</style>