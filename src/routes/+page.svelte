<script lang="ts">
  import { onMount } from 'svelte';
  import { Channel } from '@tauri-apps/api/core';
  import { call, errorMessage } from '$lib/api';
  import { listen } from '@tauri-apps/api/event';
  import { renderMarkdown } from '$lib/markdown';
  import { speak, startRecording, transcribe, type Recording } from '$lib/voice';
  import type { Settings, SystemStatus, UiEvent } from '$lib/types';
  import Avatar from '$lib/components/Avatar.svelte';
  import SettingsView from '$lib/components/SettingsView.svelte';
  import KnowledgeView from '$lib/components/KnowledgeView.svelte';
  import SystemControlView from '$lib/components/SystemControlView.svelte';
  
  let currentView = $state('home');
  let userInput = $state('');
  type Message = { role: 'user' | 'assistant'; content: string; timestamp: Date; notes: string[] };
  let messages = $state<Message[]>([]);
  let isSpeaking = $state(false);
  let isProcessing = $state(false);
  let avatarEmotion = $state<'idle' | 'thinking' | 'speaking' | 'working' | 'happy' | 'excited' | 'focused' | 'confused' | 'success' | 'error' | 'listening' | 'processing'>('idle');
  let systemStatus = $state({
    cpu: 0,
    memory: 0,
    status: 'online',
    uptime: 0,
    processes: 0
  });
  let notifications = $state<Array<{id: number, message: string, type: 'info' | 'success' | 'error'}>>([]);
  let commandSuggestions = $state<string[]>([]);
  let showSuggestions = $state(false);
  let isTyping = $state(false);
  let typingTimeout: ReturnType<typeof setTimeout> | undefined;

  // Short-lived UI timers (notifications, avatar emotions) are tracked and
  // cancelled on unmount so nothing fires against a destroyed component.
  const timers = new Set<ReturnType<typeof setTimeout>>();
  function later(fn: () => void, ms: number) {
    const t = setTimeout(() => {
      timers.delete(t);
      fn();
    }, ms);
    timers.add(t);
  }

  // Performance: Debounced input handler
  function handleInputChange(value: string) {
    userInput = value;
    isTyping = true;
    clearTimeout(typingTimeout);
    
    typingTimeout = setTimeout(() => {
      isTyping = false;
      updateSuggestions(value);
    }, 300);
  }

  // UI Enhancement: Command suggestions
  function updateSuggestions(input: string) {
    if (!input.startsWith('/')) {
      showSuggestions = false;
      return;
    }
    
    // Only commands the backend actually implements are suggested.
    const allCommands = [
      '/execute - Run a shell command (changes need your approval)',
      '/file read - Read a text file',
      '/file list - List a directory',
      '/file write - Write a file (needs your approval)',
      '/monitor - System status and top processes'
    ];
    
    commandSuggestions = allCommands.filter(cmd => 
      cmd.toLowerCase().includes(input.toLowerCase())
    );
    showSuggestions = commandSuggestions.length > 0;
  }

  // UI Enhancement: Notification system
  function addNotification(message: string, type: 'info' | 'success' | 'error' = 'info') {
    const id = Date.now();
    notifications = [...notifications, { id, message, type }];
    
    later(() => {
      notifications = notifications.filter(n => n.id !== id);
    }, 5000);
  }

  let navItems = $derived([
    { id: 'home', label: 'Home', icon: '🏠', badge: null },
    { id: 'commands', label: 'Commands', icon: '⚡', badge: null },
    { id: 'history', label: 'History', icon: '📜', badge: messages.length || null },
    { id: 'settings', label: 'Settings', icon: '⚙️', badge: null },
    { id: 'knowledge', label: 'Knowledge', icon: '🧠', badge: null },
    { id: 'system', label: 'System Control', icon: '🎛️', badge: null }
  ]);

  // Performance: Memoized filtered messages
  let recentMessages = $derived(messages.slice(-50));

  onMount(() => {
    // Adaptive status polling. `alive` + the stored handle stop the
    // recursive timer when the component unmounts (no leaked timers).
    let alive = true;
    let statusTimer: ReturnType<typeof setTimeout> | undefined;
    let statusInterval = 5000;
    const updateSystemStatus = async () => {
      try {
        systemStatus = await call<SystemStatus>('get_system_status');
        statusInterval = systemStatus.cpu > 80 ? 2000 : 5000;
      } catch {
        // Backend not ready yet; retry on the next tick.
      }
      if (alive) statusTimer = setTimeout(updateSystemStatus, statusInterval);
    };
    updateSystemStatus();

    // Global Ctrl+Space push-to-talk (emitted by the Rust shortcut handler).
    let unlistenPtt: (() => void) | undefined;
    listen<string>('ptt', (e) => {
      if (e.payload === 'pressed') startListening();
      else stopListening();
    }).then((u) => {
      if (alive) unlistenPtt = u;
      else u();
    });

    return () => {
      unlistenPtt?.();
      alive = false;
      clearTimeout(statusTimer);
      clearTimeout(typingTimeout);
      timers.forEach(clearTimeout);
      timers.clear();
    };
  });

  async function sendMessage() {
    if (!userInput.trim() || isProcessing) return;

    const query = userInput;
    messages = [...messages, { role: 'user', content: query, timestamp: new Date(), notes: [] }];
    userInput = '';
    showSuggestions = false;
    isProcessing = true;
    isSpeaking = true;
    avatarEmotion = query.startsWith('/execute') ? 'working' : query.includes('?') ? 'thinking' : 'processing';

    try {
      if (query.startsWith('/')) {
        // Slash commands are handled locally by the backend.
        const response = await call<string>('process_command', { command: query });
        messages = [...messages, { role: 'assistant', content: response, timestamp: new Date(), notes: [] }];
      } else {
        await streamChat(query);
      }
      avatarEmotion = 'success';
      later(() => {
        avatarEmotion = 'happy';
        later(() => (avatarEmotion = 'idle'), 2000);
      }, 1000);
    } catch (error) {
      const msg = errorMessage(error);
      const last = messages[messages.length - 1];
      if (last?.role === 'assistant' && !query.startsWith('/')) {
        if (!last.notes.includes(`✗ ${msg}`)) last.notes.push(`✗ ${msg}`);
      } else {
        messages = [...messages, { role: 'assistant', content: `Error: ${msg}`, timestamp: new Date(), notes: [] }];
      }
      avatarEmotion = 'error';
      addNotification(msg, 'error');
      later(() => (avatarEmotion = 'idle'), 3000);
    } finally {
      isProcessing = false;
      isSpeaking = false;
    }
  }

  /** Stream a chat turn: tokens, tool activity and notices arrive over a Channel. */
  async function streamChat(query: string) {
    currentView = 'history';
    messages = [...messages, { role: 'assistant', content: '', timestamp: new Date(), notes: [] }];
    const reply = messages[messages.length - 1];
    const channel = new Channel<UiEvent>();
    channel.onmessage = (ev) => {
      switch (ev.type) {
        case 'token':
          reply.content += ev.text;
          break;
        case 'tool_call':
          reply.notes.push(`🔧 ${ev.name} requested`);
          break;
        case 'tool_result':
          reply.notes.push(`${ev.ok ? '✓' : '✗'} ${ev.name}: ${ev.summary}`);
          break;
        case 'notice':
          reply.notes.push(`ℹ ${ev.message}`);
          break;
        case 'error':
          reply.notes.push(`✗ ${ev.message}`);
          break;
      }
    };
    await call('chat_send', { message: query, onEvent: channel });
  }

  async function stopResponse() {
    try {
      await call('chat_cancel');
    } catch (e) {
      addNotification(errorMessage(e), 'error');
    }
  }

  async function clearConversation() {
    try {
      await call('chat_reset');
      messages = [];
    } catch (e) {
      addNotification(errorMessage(e), 'error');
    }
  }

  // Push-to-talk voice input: enabled only when voice + an STT URL are configured.
  let voiceAvailable = $state(false);
  let isListening = $state(false);
  let recording: Recording | null = null;

  async function refreshVoice() {
    try {
      const s = await call<Settings>('load_settings');
      voiceAvailable = s.voice.enabled && s.voice.stt_url.trim() !== '';
    } catch {
      voiceAvailable = false;
    }
  }

  async function startListening() {
    if (!voiceAvailable || recording || isProcessing) return;
    try {
      recording = await startRecording();
      isListening = true;
      avatarEmotion = 'listening';
    } catch (e) {
      recording = null;
      addNotification(`Microphone: ${errorMessage(e)}`, 'error');
    }
  }

  async function stopListening() {
    if (!recording) return;
    const r = recording;
    recording = null;
    isListening = false;
    avatarEmotion = 'processing';
    try {
      const { bytes, mime } = await r.stop();
      const text = await transcribe(bytes, mime);
      avatarEmotion = 'idle';
      if (text) {
        userInput = text;
        await sendMessage();
      } else {
        addNotification('No speech detected', 'info');
      }
    } catch (e) {
      avatarEmotion = 'error';
      addNotification(`Speech-to-text: ${errorMessage(e)}`, 'error');
      later(() => (avatarEmotion = 'idle'), 2000);
    }
  }

  async function speakMessage(text: string) {
    try {
      isSpeaking = true;
      await speak(text);
    } catch (e) {
      addNotification(`Text-to-speech: ${errorMessage(e)}`, 'error');
    } finally {
      isSpeaking = false;
    }
  }

  // Re-read voice settings whenever the user leaves the Settings view.
  $effect(() => {
    if (currentView !== 'settings') refreshVoice();
  });

  function handleKeyPress(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      sendMessage();
    } else if (event.key === 'Escape') {
      showSuggestions = false;
    }
  }

  function selectSuggestion(suggestion: string) {
    userInput = suggestion.split(' - ')[0];
    showSuggestions = false;
  }

  // UI Enhancement: Quick actions
  function quickAction(action: string) {
    userInput = action;
    sendMessage();
  }

  // Performance: Virtual scrolling for large message lists
  let messageContainer = $state<HTMLElement | undefined>();
  function scrollToBottom() {
    if (messageContainer) {
      messageContainer.scrollTop = messageContainer.scrollHeight;
    }
  }

  $effect(() => {
    if (messages.length > 0) {
      const t = setTimeout(scrollToBottom, 100);
      return () => clearTimeout(t);
    }
  });
</script>

<div class="flex h-screen w-screen overflow-hidden cosmic-gradient">
  <!-- UI Enhancement: Animated background particles with performance optimization -->
  <div class="absolute inset-0 overflow-hidden pointer-events-none">
    {#each Array(15) as _, i}
      <div 
        class="particle animate-particle"
        style="
          left: {Math.random() * 100}%; 
          animation-delay: {Math.random() * 20}s; 
          animation-duration: {15 + Math.random() * 10}s;
          will-change: transform;
        "
      ></div>
    {/each}
  </div>

  <!-- UI Enhancement: Notification system -->
  <div class="fixed top-4 right-4 z-50 space-y-2">
    {#each notifications as notification (notification.id)}
      <div
        class="glass-panel px-4 py-3 min-w-64 animate-slide-in {notification.type === 'info' ? 'bg-blue-500/20' : notification.type === 'success' ? 'bg-green-500/20' : 'bg-red-500/20'}"
      >
        <div class="flex items-center gap-2">
          <span class="text-xl">
            {notification.type === 'success' ? '✓' : notification.type === 'error' ? '✗' : 'ℹ'}
          </span>
          <span class="text-sm">{notification.message}</span>
        </div>
      </div>
    {/each}
  </div>

  <!-- Sidebar Navigation with UI enhancements -->
  <aside class="w-64 glass-panel m-4 p-6 flex flex-col z-10 animate-slide-in-left">
    <div class="mb-8">
      <h1 class="text-3xl font-bold glow-text animate-glow-pulse">OMNIX</h1>
      <p class="text-xs text-cosmic-cyan mt-1">v1.0.0 Enhanced</p>
    </div>

    <nav class="flex-1 space-y-2">
      {#each navItems as item}
        <button
          class="w-full text-left px-4 py-3 rounded-lg transition-all duration-200 flex items-center gap-3 relative
                 {currentView === item.id ? 'bg-cosmic-blue/20 text-cosmic-cyan border border-cosmic-blue/50 scale-105' : 'hover:bg-white/5 text-gray-300 hover:scale-102'}"
          onclick={() => currentView = item.id}
        >
          <span class="text-xl">{item.icon}</span>
          <span class="font-medium">{item.label}</span>
          {#if item.badge}
            <span class="ml-auto bg-cosmic-cyan text-cosmic-dark text-xs px-2 py-1 rounded-full">
              {item.badge}
            </span>
          {/if}
        </button>
      {/each}
    </nav>

    <!-- UI Enhancement: Enhanced system status -->
    <div class="mt-auto pt-6 border-t border-white/10">
      <div class="text-xs space-y-2">
        <div class="flex justify-between items-center">
          <span class="text-gray-400">Status:</span>
          <span class="text-green-400 flex items-center gap-1">
            <span class="w-2 h-2 bg-green-400 rounded-full animate-pulse"></span>
            {systemStatus.status}
          </span>
        </div>
        <div class="flex justify-between items-center">
          <span class="text-gray-400">CPU:</span>
          <div class="flex items-center gap-2">
            <div class="w-16 h-1 bg-white/10 rounded-full overflow-hidden">
              <div 
                class="h-full bg-cosmic-cyan transition-all duration-500"
                style="width: {systemStatus.cpu}%"
              ></div>
            </div>
            <span class="text-cosmic-cyan w-10 text-right">{systemStatus.cpu.toFixed(1)}%</span>
          </div>
        </div>
        <div class="flex justify-between items-center">
          <span class="text-gray-400">Memory:</span>
          <div class="flex items-center gap-2">
            <div class="w-16 h-1 bg-white/10 rounded-full overflow-hidden">
              <div 
                class="h-full bg-cosmic-purple transition-all duration-500"
                style="width: {systemStatus.memory}%"
              ></div>
            </div>
            <span class="text-cosmic-purple w-10 text-right">{systemStatus.memory.toFixed(1)}%</span>
          </div>
        </div>
      </div>
    </div>
  </aside>

  <!-- Main Content Area -->
  <main class="flex-1 flex flex-col p-4 z-10">
    {#if currentView === 'home'}
      <!-- Home View with Enhanced Avatar -->
      <div class="flex-1 flex flex-col items-center justify-center">
        <!-- Enhanced AI Avatar -->
        <Avatar 
          emotion={avatarEmotion}
          isSpeaking={isSpeaking}
          isWorking={isProcessing}
        />

        <h2 class="text-4xl font-bold glow-text mb-4 mt-8">OMNIX</h2>
        <p class="text-xl text-gray-300 mb-8">Your local-first AI desktop assistant</p>

        <!-- UI Enhancement: Quick action buttons -->
        <div class="flex gap-3 mb-8">
          <button 
            onclick={() => quickAction('/monitor')}
            class="glass-panel px-4 py-2 hover:bg-white/10 transition-all hover:scale-105"
          >
            <span class="text-sm">📊 System Status</span>
          </button>
          <button 
            onclick={() => quickAction('/execute ls -la')}
            class="glass-panel px-4 py-2 hover:bg-white/10 transition-all hover:scale-105"
          >
            <span class="text-sm">📁 List Files</span>
          </button>
          <button 
            onclick={() => quickAction('What can you do?')}
            class="glass-panel px-4 py-2 hover:bg-white/10 transition-all hover:scale-105"
          >
            <span class="text-sm">❓ Help</span>
          </button>
        </div>

        <!-- Enhanced Quick Stats -->
        <div class="grid grid-cols-3 gap-4 w-full max-w-2xl">
          <div class="glass-panel p-4 text-center hover:scale-105 transition-transform cursor-pointer">
            <div class="text-3xl font-bold text-cosmic-cyan animate-count-up">{messages.length}</div>
            <div class="text-sm text-gray-400 mt-1">Commands</div>
          </div>
          <div class="glass-panel p-4 text-center hover:scale-105 transition-transform cursor-pointer">
            <div class="text-3xl font-bold text-cosmic-cyan">{systemStatus.processes}</div>
            <div class="text-sm text-gray-400 mt-1">Processes</div>
          </div>
          <div class="glass-panel p-4 text-center hover:scale-105 transition-transform cursor-pointer">
            <div class="text-3xl font-bold text-green-400">{Math.floor(systemStatus.uptime / 3600)}h</div>
            <div class="text-sm text-gray-400 mt-1">System uptime</div>
          </div>
        </div>
      </div>
    {:else if currentView === 'commands'}
      <!-- Commands View -->
      <div class="flex-1 glass-panel p-6 overflow-auto animate-fade-in">
        <h2 class="text-2xl font-bold glow-text mb-6">Available Commands</h2>
        <div class="grid grid-cols-2 gap-4">
          {#each [
            { cmd: '/execute', desc: 'Run a shell command. Commands are risk-classified; anything that changes your system opens a native approval dialog, and destructive patterns are always blocked.', icon: '⚡', planned: false },
            { cmd: '/file', desc: 'read <path>, list <path>, or write <path> <content> (writes need approval; credential files are off-limits)', icon: '📁', planned: false },
            { cmd: '/monitor', desc: 'System resources and top processes', icon: '📊', planned: false },
            { cmd: '/search', desc: 'Search files and content', icon: '🔍', planned: true },
            { cmd: '/remember', desc: 'Store information in long-term memory', icon: '💾', planned: true },
            { cmd: '/recall', desc: 'Retrieve stored memories', icon: '🧠', planned: true }
          ] as command}
            <div class="glass-panel p-4 transition-all {command.planned ? 'opacity-50' : 'hover:bg-white/10'}">
              <div class="flex items-center gap-2 mb-2">
                <span class="text-2xl">{command.icon}</span>
                <code class="text-cosmic-cyan font-mono font-bold">{command.cmd}</code>
                {#if command.planned}<span class="text-xs bg-yellow-500/20 text-yellow-300 px-2 py-0.5 rounded">Planned</span>{/if}
              </div>
              <p class="text-sm text-gray-400">{command.desc}</p>
            </div>
          {/each}
        </div>
      </div>
    {:else if currentView === 'history'}
      <!-- History View with performance optimization -->
      <div class="flex-1 glass-panel p-6 overflow-auto animate-fade-in" bind:this={messageContainer}>
        <div class="flex items-center justify-between mb-6">
          <h2 class="text-2xl font-bold glow-text">Conversation</h2>
          <button onclick={clearConversation} disabled={isProcessing} class="glass-panel px-3 py-1 text-sm hover:bg-white/10 disabled:opacity-50">🧹 Clear</button>
        </div>
        <div class="space-y-3">
          {#each recentMessages as message}
            <div class="glass-panel p-4 {message.role === 'user' ? 'bg-cosmic-blue/10' : 'bg-cosmic-purple/10'} animate-slide-in">
              <div class="flex items-start gap-3">
                <span class="text-2xl">{message.role === 'user' ? '👤' : '🤖'}</span>
                <div class="flex-1">
                  <div class="text-sm text-gray-400 mb-1">
                    {message.timestamp.toLocaleTimeString()}
                  </div>
                  {#if message.role === 'assistant'}
                    <!-- Model output is untrusted: always sanitized by renderMarkdown. -->
                    <div class="md-content text-white">{@html renderMarkdown(message.content)}</div>
                  {:else}
                    <div class="text-white whitespace-pre-wrap">{message.content}</div>
                  {/if}
                  {#if message.role === 'assistant' && voiceAvailable && message.content}
                    <button class="mt-1 text-xs text-gray-400 hover:text-white" onclick={() => speakMessage(message.content)} title="Read aloud (Piper)">🔊 Read aloud</button>
                  {/if}
                  {#if message.notes.length}
                    <ul class="mt-2 space-y-1 text-xs text-gray-400">
                      {#each message.notes as note}<li>{note}</li>{/each}
                    </ul>
                  {/if}
                </div>
              </div>
            </div>
          {/each}
          {#if messages.length === 0}
            <p class="text-gray-400 text-center py-8">No messages yet</p>
          {/if}
        </div>
      </div>
    {:else if currentView === 'settings'}
      <!-- Settings View -->
      <div class="flex-1 overflow-hidden animate-fade-in">
        <SettingsView />
      </div>
    {:else if currentView === 'knowledge'}
      <!-- Knowledge View -->
      <div class="flex-1 overflow-hidden animate-fade-in">
        <KnowledgeView />
      </div>
    {:else if currentView === 'system'}
      <!-- System Control View -->
      <div class="flex-1 overflow-hidden animate-fade-in">
        <SystemControlView />
      </div>
    {:else}
      <!-- Other views placeholder -->
      <div class="flex-1 glass-panel p-6 flex items-center justify-center animate-fade-in">
        <div class="text-center">
          <div class="text-6xl mb-4 animate-bounce">🚧</div>
          <h2 class="text-2xl font-bold glow-text mb-2">{currentView.charAt(0).toUpperCase() + currentView.slice(1)}</h2>
          <p class="text-gray-400">This section is under development</p>
        </div>
      </div>
    {/if}

    <!-- Enhanced Input Bar -->
    <div class="glass-panel p-4 mt-4 relative">
      <!-- Command suggestions -->
      {#if showSuggestions}
        <div class="absolute bottom-full left-0 right-0 mb-2 glass-panel p-2 max-h-48 overflow-auto">
          {#each commandSuggestions as suggestion}
            <button
              onclick={() => selectSuggestion(suggestion)}
              class="w-full text-left px-3 py-2 hover:bg-white/10 rounded transition-all text-sm"
            >
              {suggestion}
            </button>
          {/each}
        </div>
      {/if}

      <div class="flex items-center gap-3">
        <button
          class="p-3 rounded-full transition-all duration-200 disabled:opacity-40 disabled:cursor-not-allowed {isListening ? 'bg-red-500 animate-pulse scale-110' : 'bg-cosmic-blue/20 hover:bg-cosmic-blue/30'}"
          disabled={!voiceAvailable || isProcessing}
          onpointerdown={startListening}
          onpointerup={stopListening}
          onpointerleave={stopListening}
          title={voiceAvailable ? 'Hold to talk (or hold Ctrl+Space)' : 'Voice input is off: configure it in Settings → Voice'}
          aria-label="Push to talk"
        >
          <svg class="w-6 h-6" fill="currentColor" viewBox="0 0 20 20">
            <path d="M7 4a3 3 0 016 0v6a3 3 0 11-6 0V4zm4 10.93A7.001 7.001 0 0017 8a1 1 0 10-2 0A5 5 0 015 8a1 1 0 00-2 0 7.001 7.001 0 006 6.93V17H6a1 1 0 100 2h8a1 1 0 100-2h-3v-2.07z"/>
          </svg>
        </button>

        <input
          type="text"
          value={userInput}
          oninput={(e) => handleInputChange(e.currentTarget.value)}
          onkeypress={handleKeyPress}
          placeholder="Ask OMNIX anything, or type / for commands"
          class="flex-1 bg-white/5 border border-white/10 rounded-lg px-4 py-3 text-white placeholder-gray-500 focus:outline-none focus:border-cosmic-cyan focus:ring-2 focus:ring-cosmic-cyan/20 transition-all"
          disabled={isProcessing}
        />

        {#if isProcessing}
          <button onclick={stopResponse} class="px-4 py-3 glass-panel hover:bg-white/10 text-sm" title="Stop the current response">⏹ Stop</button>
        {/if}
        <button
          onclick={sendMessage}
          disabled={isProcessing || !userInput.trim()}
          class="px-6 py-3 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg font-medium transition-all duration-200 flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed hover:scale-105 active:scale-95"
        >
          {#if isProcessing}
            <svg class="animate-spin h-5 w-5" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
          {:else}
            <span>Send</span>
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 7l5 5m0 0l-5 5m5-5H6"/>
            </svg>
          {/if}
        </button>
      </div>
    </div>
  </main>
</div>

<style>
  @keyframes slide-in {
    from {
      opacity: 0;
      transform: translateY(-10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @keyframes slide-in-left {
    from {
      opacity: 0;
      transform: translateX(-20px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  @keyframes glow-pulse {
    0%, 100% {
      text-shadow: 0 0 20px rgba(0, 212, 255, 0.5);
    }
    50% {
      text-shadow: 0 0 30px rgba(0, 212, 255, 0.8);
    }
  }

  /* Rendered (sanitized) markdown in assistant messages. */
  .md-content :global(pre) {
    background: rgba(0, 0, 0, 0.35);
    padding: 0.75rem;
    border-radius: 0.5rem;
    overflow-x: auto;
    margin: 0.5rem 0;
  }
  .md-content :global(code) {
    font-family: ui-monospace, monospace;
    font-size: 0.875em;
  }
  .md-content :global(p) {
    margin: 0.25rem 0;
  }
  .md-content :global(ul),
  .md-content :global(ol) {
    padding-left: 1.25rem;
    list-style: disc;
  }
  .md-content :global(.md-link) {
    text-decoration: underline;
    color: #7dd3fc;
    cursor: help;
  }

  .animate-slide-in {
    animation: slide-in 0.3s ease-out;
  }

  .animate-slide-in-left {
    animation: slide-in-left 0.4s ease-out;
  }

  .animate-fade-in {
    animation: fade-in 0.3s ease-out;
  }

  .animate-glow-pulse {
    animation: glow-pulse 2s ease-in-out infinite;
  }

  .hover\:scale-102:hover {
    transform: scale(1.02);
  }

  .hover\:scale-105:hover {
    transform: scale(1.05);
  }

  .active\:scale-95:active {
    transform: scale(0.95);
  }
</style>
