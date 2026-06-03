<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  
  let currentView = $state('home');
  let userInput = $state('');
  let messages = $state<Array<{role: string, content: string, timestamp: Date}>>([]);
  let isListening = $state(false);
  let isSpeaking = $state(false);
  let systemStatus = $state({
    cpu: 0,
    memory: 0,
    status: 'online'
  });

  // Avatar animation state
  let mouthOpen = $state(0);
  let eyeGlow = $state(1);

  const navItems = [
    { id: 'home', label: 'Home', icon: '🏠' },
    { id: 'commands', label: 'Commands', icon: '⚡' },
    { id: 'history', label: 'History', icon: '📜' },
    { id: 'settings', label: 'Settings', icon: '⚙️' },
    { id: 'knowledge', label: 'Knowledge', icon: '🧠' },
    { id: 'system', label: 'System Control', icon: '🎛️' }
  ];

  onMount(() => {
    // Animate avatar idle state
    const avatarInterval = setInterval(() => {
      eyeGlow = 0.8 + Math.random() * 0.2;
    }, 2000);

    // Simulate system monitoring
    const statusInterval = setInterval(async () => {
      try {
        const status = await invoke('get_system_status');
        systemStatus = status as typeof systemStatus;
      } catch (e) {
        console.log('System status not available yet');
      }
    }, 5000);

    return () => {
      clearInterval(avatarInterval);
      clearInterval(statusInterval);
    };
  });

  async function sendMessage() {
    if (!userInput.trim()) return;
    
    const userMessage = {
      role: 'user',
      content: userInput,
      timestamp: new Date()
    };
    
    messages = [...messages, userMessage];
    const query = userInput;
    userInput = '';

    // Animate speaking
    isSpeaking = true;
    animateMouth();

    try {
      const response = await invoke('process_command', { command: query });
      
      messages = [...messages, {
        role: 'assistant',
        content: response as string,
        timestamp: new Date()
      }];
    } catch (error) {
      messages = [...messages, {
        role: 'assistant',
        content: `Error: ${error}`,
        timestamp: new Date()
      }];
    } finally {
      isSpeaking = false;
      mouthOpen = 0;
    }
  }

  function animateMouth() {
    if (!isSpeaking) return;
    
    const interval = setInterval(() => {
      if (!isSpeaking) {
        clearInterval(interval);
        mouthOpen = 0;
        return;
      }
      mouthOpen = Math.random() * 0.8 + 0.2;
    }, 100);
  }

  function toggleListening() {
    isListening = !isListening;
    if (isListening) {
      // Start voice recognition
      console.log('Voice recognition started');
    }
  }

  function handleKeyPress(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      sendMessage();
    }
  }
</script>

<div class="flex h-screen w-screen overflow-hidden cosmic-gradient">
  <!-- Animated background particles -->
  <div class="absolute inset-0 overflow-hidden pointer-events-none">
    {#each Array(20) as _, i}
      <div 
        class="particle animate-particle"
        style="left: {Math.random() * 100}%; animation-delay: {Math.random() * 20}s; animation-duration: {15 + Math.random() * 10}s;"
      ></div>
    {/each}
  </div>

  <!-- Sidebar Navigation -->
  <aside class="w-64 glass-panel m-4 p-6 flex flex-col z-10">
    <div class="mb-8">
      <h1 class="text-3xl font-bold glow-text">OMNIX</h1>
      <p class="text-xs text-cosmic-cyan mt-1">v1.0.0</p>
    </div>

    <nav class="flex-1 space-y-2">
      {#each navItems as item}
        <button
          class="w-full text-left px-4 py-3 rounded-lg transition-all duration-200 flex items-center gap-3
                 {currentView === item.id ? 'bg-cosmic-blue/20 text-cosmic-cyan border border-cosmic-blue/50' : 'hover:bg-white/5 text-gray-300'}"
          onclick={() => currentView = item.id}
        >
          <span class="text-xl">{item.icon}</span>
          <span class="font-medium">{item.label}</span>
        </button>
      {/each}
    </nav>

    <div class="mt-auto pt-6 border-t border-white/10">
      <div class="text-xs space-y-2">
        <div class="flex justify-between">
          <span class="text-gray-400">Status:</span>
          <span class="text-green-400 flex items-center gap-1">
            <span class="w-2 h-2 bg-green-400 rounded-full animate-pulse"></span>
            {systemStatus.status}
          </span>
        </div>
        <div class="flex justify-between">
          <span class="text-gray-400">CPU:</span>
          <span class="text-cosmic-cyan">{systemStatus.cpu}%</span>
        </div>
        <div class="flex justify-between">
          <span class="text-gray-400">Memory:</span>
          <span class="text-cosmic-cyan">{systemStatus.memory}%</span>
        </div>
      </div>
    </div>
  </aside>

  <!-- Main Content Area -->
  <main class="flex-1 flex flex-col p-4 z-10">
    {#if currentView === 'home'}
      <!-- Home View with Avatar -->
      <div class="flex-1 flex flex-col items-center justify-center">
        <!-- AI Avatar -->
        <div class="relative mb-8 animate-float">
          <div class="w-64 h-64 relative">
            <!-- Outer glow ring -->
            <div class="absolute inset-0 rounded-full bg-cosmic-blue/20 blur-3xl animate-pulse-glow"></div>
            
            <!-- Main avatar circle -->
            <div class="absolute inset-4 rounded-full bg-gradient-to-br from-cosmic-blue to-cosmic-purple border-4 border-cosmic-cyan/50 shadow-2xl flex items-center justify-center">
              <!-- Face -->
              <div class="relative w-full h-full flex items-center justify-center">
                <!-- Eyes -->
                <div class="absolute top-1/3 left-1/3 w-8 h-8 rounded-full bg-cosmic-cyan shadow-lg shadow-cosmic-cyan/50"
                     style="opacity: {eyeGlow}; transform: translateX(-50%)"></div>
                <div class="absolute top-1/3 right-1/3 w-8 h-8 rounded-full bg-cosmic-cyan shadow-lg shadow-cosmic-cyan/50"
                     style="opacity: {eyeGlow}; transform: translateX(50%)"></div>
                
                <!-- Mouth -->
                <div class="absolute bottom-1/3 left-1/2 -translate-x-1/2">
                  <div class="w-16 h-2 bg-cosmic-cyan rounded-full transition-all duration-100"
                       style="transform: scaleY({1 + mouthOpen * 3})"></div>
                </div>

                <!-- Neural network pattern -->
                <svg class="absolute inset-0 w-full h-full opacity-20" viewBox="0 0 100 100">
                  <circle cx="50" cy="50" r="40" fill="none" stroke="currentColor" stroke-width="0.5" class="text-cosmic-cyan"/>
                  <circle cx="50" cy="50" r="30" fill="none" stroke="currentColor" stroke-width="0.5" class="text-cosmic-cyan"/>
                  <circle cx="50" cy="50" r="20" fill="none" stroke="currentColor" stroke-width="0.5" class="text-cosmic-cyan"/>
                </svg>
              </div>
            </div>
          </div>
        </div>

        <h2 class="text-4xl font-bold glow-text mb-4">OMNIX</h2>
        <p class="text-xl text-gray-300 mb-8">Your God Mode AI for all things digital</p>

        <!-- Quick Stats -->
        <div class="grid grid-cols-3 gap-4 w-full max-w-2xl">
          <div class="glass-panel p-4 text-center">
            <div class="text-3xl font-bold text-cosmic-cyan">{messages.length}</div>
            <div class="text-sm text-gray-400 mt-1">Commands</div>
          </div>
          <div class="glass-panel p-4 text-center">
            <div class="text-3xl font-bold text-cosmic-cyan">∞</div>
            <div class="text-sm text-gray-400 mt-1">Capabilities</div>
          </div>
          <div class="glass-panel p-4 text-center">
            <div class="text-3xl font-bold text-green-400">100%</div>
            <div class="text-sm text-gray-400 mt-1">Uptime</div>
          </div>
        </div>
      </div>
    {:else if currentView === 'commands'}
      <!-- Commands View -->
      <div class="flex-1 glass-panel p-6 overflow-auto">
        <h2 class="text-2xl font-bold glow-text mb-6">Available Commands</h2>
        <div class="grid grid-cols-2 gap-4">
          {#each [
            { cmd: '/execute', desc: 'Execute system command with sudo privileges' },
            { cmd: '/file', desc: 'File operations (read, write, delete, move)' },
            { cmd: '/search', desc: 'Search files and content across system' },
            { cmd: '/monitor', desc: 'Monitor system resources and processes' },
            { cmd: '/automate', desc: 'Create and run automation scripts' },
            { cmd: '/analyze', desc: 'Analyze code, logs, or documents' },
            { cmd: '/remember', desc: 'Store information in long-term memory' },
            { cmd: '/recall', desc: 'Retrieve stored memories and context' }
          ] as command}
            <div class="glass-panel p-4 hover:bg-white/10 transition-all cursor-pointer">
              <code class="text-cosmic-cyan font-mono">{command.cmd}</code>
              <p class="text-sm text-gray-400 mt-2">{command.desc}</p>
            </div>
          {/each}
        </div>
      </div>
    {:else if currentView === 'history'}
      <!-- History View -->
      <div class="flex-1 glass-panel p-6 overflow-auto">
        <h2 class="text-2xl font-bold glow-text mb-6">Command History</h2>
        <div class="space-y-3">
          {#each messages as message}
            <div class="glass-panel p-4 {message.role === 'user' ? 'bg-cosmic-blue/10' : 'bg-cosmic-purple/10'}">
              <div class="flex items-start gap-3">
                <span class="text-2xl">{message.role === 'user' ? '👤' : '🤖'}</span>
                <div class="flex-1">
                  <div class="text-sm text-gray-400 mb-1">
                    {message.timestamp.toLocaleTimeString()}
                  </div>
                  <div class="text-white">{message.content}</div>
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
      <div class="flex-1 glass-panel p-6 flex items-center justify-center">
        <div class="text-center">
          <div class="text-6xl mb-4">🚧</div>
          <h2 class="text-2xl font-bold glow-text mb-2">{currentView.charAt(0).toUpperCase() + currentView.slice(1)}</h2>
          <p class="text-gray-400">This section is under development</p>
        </div>
      </div>
    {/if}

    <!-- Input Bar -->
    <div class="glass-panel p-4 mt-4">
      <div class="flex items-center gap-3">
        <button
          class="p-3 rounded-full transition-all duration-200 {isListening ? 'bg-red-500 animate-pulse' : 'bg-cosmic-blue/20 hover:bg-cosmic-blue/30'}"
          onclick={toggleListening}
          title={isListening ? 'Stop listening' : 'Start voice input'}
        >
          <svg class="w-6 h-6" fill="currentColor" viewBox="0 0 20 20">
            <path d="M7 4a3 3 0 016 0v6a3 3 0 11-6 0V4zm4 10.93A7.001 7.001 0 0017 8a1 1 0 10-2 0A5 5 0 015 8a1 1 0 00-2 0 7.001 7.001 0 006 6.93V17H6a1 1 0 100 2h8a1 1 0 100-2h-3v-2.07z"/>
          </svg>
        </button>

        <input
          type="text"
          bind:value={userInput}
          onkeypress={handleKeyPress}
          placeholder="Ask OMNIX anything... (God Mode enabled)"
          class="flex-1 bg-white/5 border border-white/10 rounded-lg px-4 py-3 text-white placeholder-gray-500 focus:outline-none focus:border-cosmic-cyan transition-all"
        />

        <button
          onclick={sendMessage}
          class="px-6 py-3 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg font-medium transition-all duration-200 flex items-center gap-2"
        >
          <span>Send</span>
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 7l5 5m0 0l-5 5m5-5H6"/>
          </svg>
        </button>
      </div>
    </div>
  </main>
</div>
