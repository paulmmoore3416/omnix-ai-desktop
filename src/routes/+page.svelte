<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Avatar from '$lib/components/Avatar.svelte';
  
  let currentView = $state('home');
  let userInput = $state('');
  let messages = $state<Array<{role: string, content: string, timestamp: Date}>>([]);
  let isListening = $state(false);
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
  let typingTimeout: number;

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
    
    const allCommands = [
      '/execute - Execute system command',
      '/file read - Read file content',
      '/file write - Write to file',
      '/search - Search files',
      '/monitor - System monitoring',
      '/automate - Create automation',
      '/analyze - Analyze code/logs',
      '/remember - Store memory',
      '/recall - Retrieve memory'
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
    
    setTimeout(() => {
      notifications = notifications.filter(n => n.id !== id);
    }, 5000);
  }

  const navItems = [
    { id: 'home', label: 'Home', icon: '🏠', badge: null },
    { id: 'commands', label: 'Commands', icon: '⚡', badge: null },
    { id: 'history', label: 'History', icon: '📜', badge: messages.length },
    { id: 'settings', label: 'Settings', icon: '⚙️', badge: null },
    { id: 'knowledge', label: 'Knowledge', icon: '🧠', badge: null },
    { id: 'system', label: 'System Control', icon: '🎛️', badge: null }
  ];

  // Performance: Memoized filtered messages
  let recentMessages = $derived(messages.slice(-50));

  onMount(() => {
    // Performance: Optimized system monitoring with adaptive intervals
    let statusInterval = 5000;
    const updateSystemStatus = async () => {
      try {
        const status = await invoke('get_system_status');
        systemStatus = { ...systemStatus, ...status as typeof systemStatus };
        
        // Adaptive interval based on CPU usage
        if (systemStatus.cpu > 80) {
          statusInterval = 2000; // More frequent updates under load
        } else {
          statusInterval = 5000;
        }
      } catch (e) {
        console.log('System status not available yet');
      }
      
      setTimeout(updateSystemStatus, statusInterval);
    };
    
    updateSystemStatus();

    // UI Enhancement: Welcome notification
    addNotification('OMNIX initialized successfully', 'success');

    // Performance: Cleanup
    return () => {
      clearTimeout(typingTimeout);
    };
  });

  async function sendMessage() {
    if (!userInput.trim() || isProcessing) return;
    
    const userMessage = {
      role: 'user',
      content: userInput,
      timestamp: new Date()
    };
    
    messages = [...messages, userMessage];
    const query = userInput;
    userInput = '';
    showSuggestions = false;

    // Determine emotion based on command
    isProcessing = true;
    if (query.startsWith('/execute')) {
      avatarEmotion = 'working';
    } else if (query.startsWith('/search')) {
      avatarEmotion = 'focused';
    } else if (query.includes('?')) {
      avatarEmotion = 'thinking';
    } else {
      avatarEmotion = 'processing';
    }

    isSpeaking = true;

    try {
      const response = await invoke('process_command', { command: query });
      
      messages = [...messages, {
        role: 'assistant',
        content: response as string,
        timestamp: new Date()
      }];
      
      avatarEmotion = 'success';
      addNotification('Command executed successfully', 'success');
      
      setTimeout(() => {
        avatarEmotion = 'happy';
        setTimeout(() => avatarEmotion = 'idle', 2000);
      }, 1000);
      
    } catch (error) {
      messages = [...messages, {
        role: 'assistant',
        content: `Error: ${error}`,
        timestamp: new Date()
      }];
      
      avatarEmotion = 'error';
      addNotification(`Error: ${error}`, 'error');
      
      setTimeout(() => avatarEmotion = 'idle', 3000);
    } finally {
      isProcessing = false;
      isSpeaking = false;
    }
  }

  function toggleListening() {
    isListening = !isListening;
    if (isListening) {
      avatarEmotion = 'listening';
      addNotification('Voice recognition started', 'info');
    } else {
      avatarEmotion = 'idle';
    }
  }

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
  let messageContainer: HTMLElement;
  function scrollToBottom() {
    if (messageContainer) {
      messageContainer.scrollTop = messageContainer.scrollHeight;
    }
  }

  $effect(() => {
    if (messages.length > 0) {
      setTimeout(scrollToBottom, 100);
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
        class="glass-panel px-4 py-3 min-w-64 animate-slide-in"
        class:bg-blue-500/20={notification.type === 'info'}
        class:bg-green-500/20={notification.type === 'success'}
        class:bg-red-500/20={notification.type === 'error'}
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
        <p class="text-xl text-gray-300 mb-8">Your God Mode AI Desktop Assistant</p>

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
            <div class="text-3xl font-bold text-cosmic-cyan">∞</div>
            <div class="text-sm text-gray-400 mt-1">Capabilities</div>
          </div>
          <div class="glass-panel p-4 text-center hover:scale-105 transition-transform cursor-pointer">
            <div class="text-3xl font-bold text-green-400">100%</div>
            <div class="text-sm text-gray-400 mt-1">Uptime</div>
          </div>
        </div>
      </div>
    {:else if currentView === 'commands'}
      <!-- Commands View -->
      <div class="flex-1 glass-panel p-6 overflow-auto animate-fade-in">
        <h2 class="text-2xl font-bold glow-text mb-6">Available Commands</h2>
        <div class="grid grid-cols-2 gap-4">
          {#each [
            { cmd: '/execute', desc: 'Execute system command with sudo privileges', icon: '⚡' },
            { cmd: '/file', desc: 'File operations (read, write, delete, move)', icon: '📁' },
            { cmd: '/search', desc: 'Search files and content across system', icon: '🔍' },
            { cmd: '/monitor', desc: 'Monitor system resources and processes', icon: '📊' },
            { cmd: '/automate', desc: 'Create and run automation scripts', icon: '🤖' },
            { cmd: '/analyze', desc: 'Analyze code, logs, or documents', icon: '🔬' },
            { cmd: '/remember', desc: 'Store information in long-term memory', icon: '💾' },
            { cmd: '/recall', desc: 'Retrieve stored memories and context', icon: '🧠' }
          ] as command}
            <div class="glass-panel p-4 hover:bg-white/10 transition-all hover:scale-105 cursor-pointer">
              <div class="flex items-center gap-2 mb-2">
                <span class="text-2xl">{command.icon}</span>
                <code class="text-cosmic-cyan font-mono font-bold">{command.cmd}</code>
              </div>
              <p class="text-sm text-gray-400">{command.desc}</p>
            </div>
          {/each}
        </div>
      </div>
    {:else if currentView === 'history'}
      <!-- History View with performance optimization -->
      <div class="flex-1 glass-panel p-6 overflow-auto animate-fade-in" bind:this={messageContainer}>
        <h2 class="text-2xl font-bold glow-text mb-6">Command History</h2>
        <div class="space-y-3">
          {#each recentMessages as message}
            <div class="glass-panel p-4 {message.role === 'user' ? 'bg-cosmic-blue/10' : 'bg-cosmic-purple/10'} animate-slide-in">
              <div class="flex items-start gap-3">
                <span class="text-2xl">{message.role === 'user' ? '👤' : '🤖'}</span>
                <div class="flex-1">
                  <div class="text-sm text-gray-400 mb-1">
                    {message.timestamp.toLocaleTimeString()}
                  </div>
                  <div class="text-white whitespace-pre-wrap">{message.content}</div>
                </div>
              </div>
            </div>
          {/each}
          {#if messages.length === 0}
            <p class="text-gray-400 text-center py-8">No commands executed yet</p>
          {/if}
        </div>
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
          class="p-3 rounded-full transition-all duration-200 {isListening ? 'bg-red-500 animate-pulse scale-110' : 'bg-cosmic-blue/20 hover:bg-cosmic-blue/30'}"
          onclick={toggleListening}
          title={isListening ? 'Stop listening' : 'Start voice input'}
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
          placeholder="Ask OMNIX anything... (God Mode enabled)"
          class="flex-1 bg-white/5 border border-white/10 rounded-lg px-4 py-3 text-white placeholder-gray-500 focus:outline-none focus:border-cosmic-cyan focus:ring-2 focus:ring-cosmic-cyan/20 transition-all"
          disabled={isProcessing}
        />

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
