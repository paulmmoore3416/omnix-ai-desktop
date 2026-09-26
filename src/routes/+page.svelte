<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { Channel } from '@tauri-apps/api/core';
  import { call, errorMessage, isAppError } from '$lib/api';
  import { listen } from '@tauri-apps/api/event';
  import { renderMarkdown } from '$lib/markdown';
  import { speak, startRecording, transcribe, type Recording } from '$lib/voice';
  import type { Settings, SystemStatus, UiEvent, OpsEvent, RecalledMemory } from '$lib/types';
  import { MOODS, conditionForError, rgba, type Condition, type Emotion, type Signal, type SignalKind } from '$lib/avatar';
  import Avatar from '$lib/components/Avatar.svelte';
  import SettingsView from '$lib/components/SettingsView.svelte';
  import KnowledgeView from '$lib/components/KnowledgeView.svelte';
  import SystemControlView from '$lib/components/SystemControlView.svelte';
  import WorkspaceDock from '$lib/components/workspace/WorkspaceDock.svelte';
  import { REPLY_ACTIONS, copyText, type SideTask } from '$lib/workspace';
  
  let currentView = $state('home');
  let userInput = $state('');
  type Recall = RecalledMemory & { vote?: 'helpful' | 'wrong' };
  type Message = {
    role: 'user' | 'assistant';
    content: string;
    timestamp: Date;
    notes: string[];
    recalled?: Recall[];
  };
  let messages = $state<Message[]>([]);
  let isSpeaking = $state(false);
  let isProcessing = $state(false);
  let avatarEmotion = $state<Emotion>('idle');
  let systemStatus = $state({
    cpu: 0,
    memory: 0,
    status: 'online',
    uptime: 0,
    processes: 0
  });
  // Avatar condition: backend reachability, system load, and recent
  // security/network failures. Signals ripple on individual agent events.
  let statusFailures = $state(0);
  let recentIssue = $state<Condition | null>(null);
  let issueGen = 0;
  let avatarSignal = $state<Signal | null>(null);
  let signalId = 0;
  // Bumped per streamed chunk so the avatar sprays sparks toward the chat.
  let streamTick = $state(0);
  // The home panel's fluid glass is tinted with the avatar's current mood.
  let moodColor = $derived(MOODS[avatarEmotion === 'idle' && isProcessing ? 'working' : avatarEmotion].color);
  let avatarCondition = $derived<Condition>(
    statusFailures >= 2
      ? 'offline'
      : (recentIssue ??
          (systemStatus.cpu > 85 || systemStatus.memory > 90 ? 'strain' : 'nominal'))
  );
  function flagIssue(error: unknown) {
    const c = conditionForError(isAppError(error) ? error.kind : undefined);
    if (!c) return;
    recentIssue = c;
    const gen = ++issueGen;
    later(() => {
      if (gen === issueGen) recentIssue = null;
    }, 6000);
  }
  function pulse(kind: SignalKind, label?: string, ref?: string) {
    avatarSignal = { kind, id: ++signalId, label, ref };
  }

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
      '/monitor - System status and top processes',
      '/remember - Save a fact to long-term memory (#tags at the end)',
      '/recall - Look up memories (no query: most recent)',
      '/search - Search memories and indexed documents'
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

  // Workspace dock (metrics, side tasks, prompts, pins, ops) beside Home and
  // the conversation. It stays mounted while hidden so running side tasks
  // survive view switches.
  const DOCK_KEY = 'omnix.workspace.open';
  let dockOpen = $state(readDockOpen());
  let dock = $state<ReturnType<typeof WorkspaceDock> | undefined>();
  let opsTick = $state(0);
  let chatView = $derived(currentView === 'home' || currentView === 'history');
  function readDockOpen() {
    try {
      return localStorage.getItem(DOCK_KEY) !== '0';
    } catch {
      return true;
    }
  }
  $effect(() => {
    try {
      localStorage.setItem(DOCK_KEY, dockOpen ? '1' : '0');
    } catch {
      /* storage unavailable */
    }
  });

  /** Run text as a side task (opens the dock). */
  async function sideTask(text: string, opts: { title?: string; context?: string } = {}) {
    dockOpen = true;
    await tick();
    dock?.runTask(text, opts);
  }

  function onSideTaskDone(t: SideTask) {
    pulse(t.status === 'error' ? 'fail' : 'ok', 'side task');
    if (!dockOpen || !chatView) {
      addNotification(`⚡ Side task ${t.status === 'done' ? 'finished' : t.status}: ${t.title}`, t.status === 'error' ? 'error' : 'success');
    }
  }

  /** Put text in the chat box (appending to anything already typed). */
  function insertIntoInput(text: string) {
    userInput = userInput.trim() ? `${userInput.trim()}\n\n${text}` : text;
    if (!chatView) currentView = 'home';
    tick().then(() => chatInput?.focus());
  }

  async function pinText(text: string, source: string) {
    dockOpen = true;
    await tick();
    dock?.pin(text, source);
  }

  async function copyMessage(text: string) {
    if (await copyText(text)) addNotification('Copied', 'success');
    else addNotification('Copy is not allowed here: select the text instead', 'error');
  }

  /** Ask the last question again. */
  function retryLast() {
    const last = [...messages].reverse().find((m) => m.role === 'user');
    if (!last || isProcessing) return;
    userInput = last.content;
    sendMessage();
  }

  // ↑/↓ in an empty chat box walks back through what you've sent.
  let historyIndex = -1;
  let chatInput = $state<HTMLInputElement | undefined>();
  function handleHistoryKeys(event: KeyboardEvent) {
    if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;
    const sent = messages.filter((m) => m.role === 'user').map((m) => m.content);
    if (!sent.length) return;
    const browsing = historyIndex >= 0 && userInput === sent[sent.length - 1 - historyIndex];
    if (!browsing && userInput.trim()) return;
    if (!browsing) historyIndex = -1;
    event.preventDefault();
    historyIndex = event.key === 'ArrowUp' ? Math.min(historyIndex + 1, sent.length - 1) : historyIndex - 1;
    userInput = historyIndex >= 0 ? sent[sent.length - 1 - historyIndex] : '';
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
        statusFailures = 0;
      } catch {
        // Backend not ready yet; retry on the next tick. Two misses in a row
        // mark it offline (one miss at startup is normal).
        statusFailures++;
      }
      if (alive) statusTimer = setTimeout(updateSystemStatus, statusInterval);
    };
    updateSystemStatus();

    // Global Ctrl+Space push-to-talk (emitted by the Rust shortcut handler).
    let unlistenPtt: (() => void) | undefined;
    // Key auto-repeat can deliver several "pressed" events; startListening()
    // ignores all but the first because micState leaves 'idle' synchronously.
    listen<string>('ptt', (e) => {
      if (e.payload === 'pressed') {
        tapMode = false;
        startListening();
      } else {
        stopListening();
      }
    }).then((u) => {
      if (alive) unlistenPtt = u;
      else u();
    });

    // Alerts, automation and schedule results from the ops engine.
    let unlistenOps: (() => void) | undefined;
    listen<OpsEvent>('ops://event', (e) => {
      const ev = e.payload;
      const type = ev.kind === 'alert_resolved' || (ev.ok && ev.kind !== 'alert_fired') ? 'success' : ev.ok ? 'info' : 'error';
      addNotification(`${ev.kind === 'alert_fired' ? '⚠' : ev.ok ? '✓' : '✗'} ${ev.name}: ${ev.summary}`, type);
      pulse(ev.kind === 'alert_fired' || !ev.ok ? 'fail' : 'notice', ev.name);
      opsTick++;
    }).then((u) => {
      if (alive) unlistenOps = u;
      else u();
    });

    return () => {
      unlistenPtt?.();
      unlistenOps?.();
      recording?.cancel();
      clearInterval(micTicker);
      alive = false;
      clearTimeout(statusTimer);
      clearTimeout(typingTimeout);
      timers.forEach(clearTimeout);
      timers.clear();
    };
  });

  async function sendMessage() {
    if (!userInput.trim()) return;
    if (isProcessing) {
      // The chat is busy: don't make the user wait, run it beside the chat.
      const text = userInput;
      userInput = '';
      showSuggestions = false;
      addNotification('The chat is busy, so this runs as a side task in the Workspace', 'info');
      await sideTask(text);
      return;
    }

    historyIndex = -1;
    const query = userInput;
    messages = [...messages, { role: 'user', content: query, timestamp: new Date(), notes: [] }];
    userInput = '';
    showSuggestions = false;
    isProcessing = true;
    avatarEmotion = moodForQuery(query);
    const started = performance.now();

    try {
      if (query.startsWith('/')) {
        // Slash commands are handled locally by the backend.
        const response = await call<string>('process_command', { command: query });
        messages = [...messages, { role: 'assistant', content: response, timestamp: new Date(), notes: [] }];
      } else {
        await streamChat(query);
      }
      avatarEmotion = 'success';
      // A long job that finished cleanly earns a little celebration.
      const long = performance.now() - started > 8000;
      later(() => {
        avatarEmotion = long ? 'excited' : 'happy';
        later(() => (avatarEmotion = 'idle'), long ? 2500 : 2000);
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
      flagIssue(error);
      addNotification(msg, 'error');
      later(() => (avatarEmotion = 'idle'), 3000);
    } finally {
      isProcessing = false;
      isSpeaking = false;
    }
  }

  /** Which mood the avatar shows while a request runs. */
  function moodForQuery(query: string): Emotion {
    const q = query.trim().toLowerCase();
    if (q.startsWith('/remember')) return 'remembering';
    if (/^\/(search|recall|monitor|file (read|list))\b/.test(q)) return 'searching';
    if (q.startsWith('/')) return 'working';
    return q.includes('?') ? 'thinking' : 'processing';
  }

  /**
   * Stream a chat turn: tokens, tool activity and notices arrive over a Channel.
   * The reply stays in the current view, so on Home you can watch the avatar
   * react (searching memory, running tools, replying) while the text arrives.
   */
  async function streamChat(query: string) {
    messages = [...messages, { role: 'assistant', content: '', timestamp: new Date(), notes: [] }];
    const reply = messages[messages.length - 1];
    const channel = new Channel<UiEvent>();
    channel.onmessage = (ev) => {
      switch (ev.type) {
        case 'token':
          reply.content += ev.text;
          avatarEmotion = 'speaking';
          isSpeaking = true;
          streamTick++;
          break;
        case 'tool_call':
          reply.notes.push(`🔧 ${ev.name} requested`);
          pulse('tool', ev.name, ev.id);
          avatarEmotion = 'working';
          isSpeaking = false;
          break;
        case 'tool_result':
          reply.notes.push(`${ev.ok ? '✓' : '✗'} ${ev.name}: ${ev.summary}`);
          pulse(ev.ok ? 'ok' : 'fail', ev.name, ev.id);
          // Back to thinking while the model reads the result.
          avatarEmotion = 'thinking';
          break;
        case 'recalled':
          reply.recalled = ev.memories.map((m) => ({ ...m }));
          if (ev.memories.length) avatarEmotion = 'searching';
          break;
        case 'notice':
          reply.notes.push(`ℹ ${ev.message}`);
          pulse('notice');
          break;
        case 'error':
          reply.notes.push(`✗ ${ev.message}`);
          pulse('fail');
          break;
      }
    };
    await call('chat_send', { message: query, onEvent: channel });
  }

  /** Recall feedback: 👎 ranks the memory lower from now on, 👍 undoes one flag. */
  async function recallFeedback(message: Message, m: Recall, helpful: boolean) {
    try {
      await call('memory_feedback', { id: m.id, helpful });
      m.vote = helpful ? 'helpful' : 'wrong';
    } catch (e) {
      message.notes.push(`✗ memory feedback failed: ${errorMessage(e)}`);
    }
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

  // Push-to-talk voice input. Hold the mic (or Ctrl+Space) to talk, or tap once
  // to start and tap again to send. Esc cancels. Enabled only when voice + an
  // STT URL are configured; otherwise the button explains how to set it up.
  type MicState = 'idle' | 'starting' | 'recording' | 'transcribing';
  let voiceAvailable = $state(false);
  let voiceSetupHint = $state('Voice input is off: configure it in Settings → Voice');
  let micState = $state<MicState>('idle');
  let micLevel = $state(0);
  let micElapsed = $state(0);
  let tapMode = $state(false);
  let isListening = $derived(micState === 'recording' || micState === 'starting');
  let settingsTab = $state('general');
  let recording: Recording | null = null;
  // Set when the user releases before the microphone finished opening.
  let stopRequested = false;
  let pressedAt = 0;
  let micTicker: ReturnType<typeof setInterval> | undefined;
  /** A press shorter than this is a tap: keep recording until the next tap. */
  const TAP_MS = 350;
  /** Clips shorter than this almost never contain a full word. */
  const MIN_CLIP_MS = 400;
  /** Safety cap so a forgotten tap-mode recording can't run forever. */
  const MAX_CLIP_MS = 120_000;

  async function refreshVoice() {
    try {
      const s = await call<Settings>('load_settings');
      const hasUrl = s.voice.stt_url.trim() !== '';
      voiceAvailable = s.voice.enabled && hasUrl;
      voiceSetupHint = !s.voice.enabled
        ? 'Voice is turned off: enable it in Settings → Voice'
        : 'Add a speech-to-text server URL in Settings → Voice';
    } catch {
      voiceAvailable = false;
    }
  }

  function openVoiceSettings() {
    addNotification(voiceSetupHint, 'info');
    settingsTab = 'voice';
    currentView = 'settings';
  }

  function clearMicTicker() {
    clearInterval(micTicker);
    micTicker = undefined;
    micElapsed = 0;
  }

  async function startListening() {
    if (micState !== 'idle' || isProcessing) return;
    if (!voiceAvailable) {
      openVoiceSettings();
      return;
    }
    micState = 'starting';
    stopRequested = false;
    avatarEmotion = 'listening';
    try {
      const r = await startRecording({ onLevel: (l) => (micLevel = l) });
      recording = r;
      micState = 'recording';
      const began = Date.now();
      micTicker = setInterval(() => {
        micElapsed = Date.now() - began;
        if (micElapsed >= MAX_CLIP_MS) {
          addNotification('Recording stopped at the 2-minute limit', 'info');
          stopListening();
        }
      }, 200);
      if (stopRequested) await stopListening();
    } catch (e) {
      recording = null;
      micState = 'idle';
      tapMode = false;
      avatarEmotion = 'error';
      addNotification(`Microphone: ${errorMessage(e)}`, 'error');
      later(() => (avatarEmotion = 'idle'), 2000);
    }
  }

  async function stopListening() {
    if (micState === 'starting') {
      stopRequested = true;
      return;
    }
    if (micState !== 'recording' || !recording) return;
    const r = recording;
    recording = null;
    tapMode = false;
    clearMicTicker();
    micState = 'transcribing';
    avatarEmotion = 'processing';
    try {
      const { bytes, mime, durationMs } = await r.stop();
      if (durationMs < MIN_CLIP_MS) {
        avatarEmotion = 'confused';
        addNotification('That was too short: hold the mic while you speak, or tap once to start and again to send', 'info');
        later(() => (avatarEmotion = 'idle'), 2000);
        return;
      }
      const text = await transcribe(bytes, mime);
      avatarEmotion = 'idle';
      if (text) {
        micState = 'idle';
        userInput = text;
        await sendMessage();
      } else {
        avatarEmotion = 'confused';
        addNotification('No speech detected. Try speaking a little louder or closer to the mic', 'info');
        later(() => (avatarEmotion = 'idle'), 2000);
      }
    } catch (e) {
      avatarEmotion = 'error';
      flagIssue(e);
      addNotification(`Speech-to-text: ${errorMessage(e)}`, 'error');
      later(() => (avatarEmotion = 'idle'), 2000);
    } finally {
      micState = 'idle';
    }
  }

  function cancelListening() {
    recording?.cancel();
    recording = null;
    tapMode = false;
    clearMicTicker();
    if (micState === 'recording') micState = 'idle';
    avatarEmotion = 'idle';
  }

  // Mouse/touch/pen: hold-to-talk, or tap to toggle.
  function onMicPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    // Keep receiving pointerup even if the pointer drifts off the button.
    try {
      (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    } catch {
      // Pointer already released (e.g. a very fast tap); nothing to capture.
    }
    if (tapMode) {
      stopListening();
      return;
    }
    pressedAt = performance.now();
    startListening();
  }

  function onMicPointerUp() {
    if (!pressedAt) return;
    const held = performance.now() - pressedAt;
    pressedAt = 0;
    if (held < TAP_MS && micState !== 'idle') {
      tapMode = true; // quick tap: keep recording until the next tap
      stopRequested = false;
    } else {
      stopListening();
    }
  }

  // Keyboard: Enter/Space on the focused button toggles recording.
  function onMicKeyDown(e: KeyboardEvent) {
    if (e.key !== 'Enter' && e.key !== ' ') return;
    e.preventDefault();
    if (e.repeat) return;
    if (micState === 'idle') {
      tapMode = true;
      startListening();
    } else {
      stopListening();
    }
  }

  function formatElapsed(ms: number) {
    const s = Math.floor(ms / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
  }

  async function speakMessage(text: string) {
    try {
      isSpeaking = true;
      if (!isProcessing) avatarEmotion = 'speaking';
      await speak(text);
    } catch (e) {
      addNotification(`Text-to-speech: ${errorMessage(e)}`, 'error');
    } finally {
      isSpeaking = false;
      if (avatarEmotion === 'speaking' && !isProcessing) avatarEmotion = 'idle';
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

  // Esc cancels an in-progress recording from anywhere in the window.
  function handleWindowKeyDown(event: KeyboardEvent) {
    if (event.key === 'Escape' && micState === 'recording') cancelListening();
    // Ctrl+. shows/hides the workspace dock.
    if (event.key === '.' && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      dockOpen = !dockOpen;
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
    // Follow new messages and streamed reply text.
    void streamTick;
    if (messages.length > 0) {
      const t = setTimeout(scrollToBottom, 100);
      return () => clearTimeout(t);
    }
  });
</script>

<svelte:window onkeydown={handleWindowKeyDown} />

{#snippet conversation()}
  <div class="space-y-3">
    {#each recentMessages as message}
      <div class="group glass-panel p-4 {message.role === 'user' ? 'bg-cosmic-blue/10' : 'bg-cosmic-purple/10'} animate-slide-in">
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
            {#if message.role === 'assistant' && message.content}
              <div class="msg-actions opacity-60 group-hover:opacity-100 focus-within:opacity-100 transition-opacity mt-1.5 flex flex-wrap items-center gap-1 text-xs text-gray-400">
                {#if voiceAvailable}
                  <button class="msg-act" onclick={() => speakMessage(message.content)} title="Read aloud (Piper)">🔊 Read aloud</button>
                {/if}
                <button class="msg-act" onclick={() => copyMessage(message.content)} title="Copy the reply">📋 Copy</button>
                <button class="msg-act" onclick={() => pinText(message.content, `Reply · ${message.timestamp.toLocaleString()}`)} title="Pin to the workspace board">📌 Pin</button>
                {#each REPLY_ACTIONS as a (a.label)}
                  <button class="msg-act" onclick={() => sideTask(a.instruction, { title: `${a.icon} ${a.label}`, context: message.content })} title="{a.label}: runs as a side task, the chat stays free">{a.icon} {a.label}</button>
                {/each}
                {#if message === messages[messages.length - 1] && !isProcessing}
                  <button class="msg-act" onclick={retryLast} title="Ask the last question again">↻ Ask again</button>
                {/if}
              </div>
            {:else if message.role === 'user'}
              <div class="msg-actions opacity-60 group-hover:opacity-100 focus-within:opacity-100 transition-opacity mt-1 flex gap-1 text-xs text-gray-400">
                <button class="msg-act" onclick={() => insertIntoInput(message.content)} title="Edit and send again">✎ Edit</button>
                <button class="msg-act" onclick={() => sideTask(message.content)} title="Run this as a side task (no tools), next to the chat">⚡ Side task</button>
              </div>
            {/if}
            {#if message.notes.length}
              <ul class="mt-2 space-y-1 text-xs text-gray-400">
                {#each message.notes as note}<li>{note}</li>{/each}
              </ul>
            {/if}
            {#if message.recalled?.length}
              <details class="mt-2 text-xs text-gray-400">
                <summary class="cursor-pointer hover:text-white">🧠 Memories used ({message.recalled.length})</summary>
                <ul class="mt-1 space-y-1">
                  {#each message.recalled as m (m.id)}
                    <li class="flex items-start gap-2">
                      <!-- Memory text is data: plain-text interpolation only. -->
                      <span class="flex-1">{m.preview}</span>
                      {#if m.vote}
                        <span class="shrink-0">{m.vote === 'wrong' ? 'Flagged: ranked lower from now on' : 'Thanks'}</span>
                      {:else}
                        <button class="shrink-0 hover:text-white" title="This memory helped" onclick={() => recallFeedback(message, m, true)}>👍</button>
                        <button class="shrink-0 hover:text-white" title="Wrong or not relevant: rank it lower (it is not deleted)" onclick={() => recallFeedback(message, m, false)}>👎</button>
                      {/if}
                    </li>
                  {/each}
                </ul>
              </details>
            {/if}
          </div>
        </div>
      </div>
    {/each}
  </div>
{/snippet}

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
      <p class="text-xs text-cosmic-cyan mt-1">v1.1.0</p>
    </div>

    <nav class="flex-1 space-y-2">
      {#each navItems as item}
        <button
          class="w-full text-left px-4 py-3 rounded-lg transition-all duration-200 flex items-center gap-3 relative
                 {currentView === item.id ? 'bg-cosmic-blue/20 text-cosmic-cyan border border-cosmic-blue/50 scale-105' : 'hover:bg-white/5 text-gray-300 hover:scale-102'}"
          onclick={() => {
            settingsTab = 'general';
            currentView = item.id;
          }}
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
      <!-- Home: one fluid-glass panel. The avatar sits near the top and the
           conversation streams in below it, so you can chat and watch it work. -->
      <div
        class="fluid-glass flex-1 min-h-0 flex flex-col animate-fade-in"
        class:busy={isProcessing}
        style="--fluid: {moodColor}; --fluid-soft: {rgba(moodColor, 0.35)};"
      >
        <div class="fluid-blobs" aria-hidden="true">
          <span class="blob b1"></span><span class="blob b2"></span><span class="blob b3"></span><span class="blob b4"></span>
        </div>

        <div class="relative z-10 flex-shrink-0 flex justify-center pt-3">
          <Avatar
            emotion={avatarEmotion}
            isSpeaking={isSpeaking}
            isWorking={isProcessing}
            micLevel={micLevel}
            condition={avatarCondition}
            signal={avatarSignal}
            cpu={systemStatus.cpu}
            memory={systemStatus.memory}
            scale={messages.length ? 0.82 : 1}
            {streamTick}
          />
        </div>

        {#if messages.length === 0}
          <div class="relative z-10 flex-1 min-h-0 overflow-auto flex flex-col items-center px-6 pb-6">
            <h2 class="text-4xl font-bold glow-text mb-2 mt-4">OMNIX</h2>
            <p class="text-lg text-gray-300 mb-6">Your local-first AI desktop assistant</p>

            <div class="flex flex-wrap justify-center gap-3 mb-6">
              <button onclick={() => quickAction('/monitor')} class="glass-chip">📊 System Status</button>
              <button onclick={() => quickAction('/execute ls -la')} class="glass-chip">📁 List Files</button>
              <button onclick={() => quickAction('What can you do?')} class="glass-chip">❓ Help</button>
              <button onclick={async () => { dockOpen = true; await tick(); dock?.brief(); }} class="glass-chip" title="AI brief of alerts, load, GPUs, models and schedules; runs beside the chat">📋 Situation brief</button>
              <button onclick={() => sideTask('/recall')} class="glass-chip" title="Your most recent memories, as a side task">🧠 Recent memories</button>
            </div>
            <p class="text-xs text-gray-400 mb-6 -mt-3">Tip: while OMNIX is replying you can keep typing. New requests run as side tasks in the Workspace (Ctrl+.).</p>

            <div class="grid grid-cols-3 gap-4 w-full max-w-2xl">
              <div class="glass-chip p-4 text-center">
                <div class="text-3xl font-bold text-cosmic-cyan">{messages.length}</div>
                <div class="text-sm text-gray-400 mt-1">Commands</div>
              </div>
              <div class="glass-chip p-4 text-center">
                <div class="text-3xl font-bold text-cosmic-cyan">{systemStatus.processes}</div>
                <div class="text-sm text-gray-400 mt-1">Processes</div>
              </div>
              <div class="glass-chip p-4 text-center">
                <div class="text-3xl font-bold text-green-400">{Math.floor(systemStatus.uptime / 3600)}h</div>
                <div class="text-sm text-gray-400 mt-1">System uptime</div>
              </div>
            </div>
          </div>
        {:else}
          <div class="relative z-10 flex items-center justify-end px-5 pt-1">
            <button onclick={clearConversation} disabled={isProcessing} class="glass-chip px-3 py-1 text-xs disabled:opacity-50">🧹 Clear</button>
          </div>
          <div class="relative z-10 flex-1 min-h-0 overflow-auto px-5 pb-5 pt-2 chat-fade" bind:this={messageContainer}>
            {@render conversation()}
          </div>
        {/if}
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
            { cmd: '/remember', desc: 'Save a fact to long-term memory. Trailing #words become tags: /remember I prefer metric units #prefs', icon: '💾', planned: false },
            { cmd: '/recall', desc: 'Look up your memories by meaning: /recall units. With no query, lists the most recent ones.', icon: '🧠', planned: false },
            { cmd: '/search', desc: 'Search everything in memory: memories plus indexed notes, documents and watched folders', icon: '🔍', planned: false }
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
        {@render conversation()}
        {#if messages.length === 0}
          <p class="text-gray-400 text-center py-8">No messages yet</p>
        {/if}
      </div>
    {:else if currentView === 'settings'}
      <!-- Settings View -->
      <div class="flex-1 overflow-hidden animate-fade-in">
        <SettingsView initialTab={settingsTab} />
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
        <div class="relative flex items-center">
          <button
            class="mic-btn relative p-3 rounded-full select-none touch-none disabled:opacity-40 disabled:cursor-not-allowed
                   {isListening ? 'bg-red-500 text-white' : micState === 'transcribing' ? 'bg-cosmic-purple/40' : voiceAvailable ? 'bg-cosmic-blue/20 hover:bg-cosmic-blue/30' : 'bg-white/5 text-gray-400 hover:bg-white/10'}"
            style="--level: {micLevel};"
            class:recording={micState === 'recording'}
            disabled={isProcessing || micState === 'transcribing'}
            onpointerdown={onMicPointerDown}
            onpointerup={onMicPointerUp}
            onpointercancel={onMicPointerUp}
            onkeydown={onMicKeyDown}
            oncontextmenu={(e) => e.preventDefault()}
            title={!voiceAvailable
              ? `${voiceSetupHint} (click to open)`
              : tapMode
                ? 'Recording: tap again to send, Esc to cancel'
                : 'Hold to talk, or tap to start/stop (Ctrl+Space works anywhere)'}
            aria-label={isListening ? 'Stop recording and send' : 'Push to talk'}
            aria-pressed={isListening}
          >
            {#if micState === 'recording'}
              <span class="mic-ring" aria-hidden="true"></span>
            {/if}
            {#if micState === 'transcribing' || micState === 'starting'}
              <svg class="animate-spin w-6 h-6" viewBox="0 0 24 24" aria-hidden="true">
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none"></circle>
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
              </svg>
            {:else}
              <svg class="relative w-6 h-6" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                <path d="M7 4a3 3 0 016 0v6a3 3 0 11-6 0V4zm4 10.93A7.001 7.001 0 0017 8a1 1 0 10-2 0A5 5 0 015 8a1 1 0 00-2 0 7.001 7.001 0 006 6.93V17H6a1 1 0 100 2h8a1 1 0 100-2h-3v-2.07z"/>
              </svg>
            {/if}
            {#if !voiceAvailable}
              <span class="absolute -top-0.5 -right-0.5 w-3 h-3 rounded-full bg-yellow-400 border-2 border-cosmic-dark" aria-hidden="true"></span>
            {/if}
          </button>
          {#if micState === 'recording'}
            <div class="ml-2 flex items-center gap-2 text-xs text-red-300 font-mono" aria-live="polite">
              <span class="w-2 h-2 rounded-full bg-red-500 animate-pulse"></span>
              {formatElapsed(micElapsed)}
              <span class="flex items-end gap-0.5 h-4" aria-hidden="true">
                {#each [0.5, 0.8, 1, 0.8, 0.5] as w}
                  <span class="w-1 rounded-sm bg-red-400 transition-[height] duration-75" style="height: {Math.max(15, micLevel * w * 100)}%"></span>
                {/each}
              </span>
              <span class="text-gray-400 font-sans">{tapMode ? 'tap to send · Esc cancels' : 'release to send'}</span>
            </div>
          {:else if micState === 'transcribing'}
            <span class="ml-2 text-xs text-cosmic-cyan" aria-live="polite">Transcribing…</span>
          {/if}
        </div>

        <input
          type="text"
          bind:this={chatInput}
          value={userInput}
          oninput={(e) => handleInputChange(e.currentTarget.value)}
          onkeypress={handleKeyPress}
          onkeydown={handleHistoryKeys}
          placeholder={isProcessing ? 'OMNIX is replying. Anything you send now runs as a side task' : 'Ask OMNIX anything, or type / for commands (↑ for history)'}
          class="flex-1 bg-white/5 border border-white/10 rounded-lg px-4 py-3 text-white placeholder-gray-500 focus:outline-none focus:border-cosmic-cyan focus:ring-2 focus:ring-cosmic-cyan/20 transition-all"
        />

        {#if isProcessing}
          <button onclick={stopResponse} class="px-4 py-3 glass-panel hover:bg-white/10 text-sm" title="Stop the current response">⏹ Stop</button>
        {/if}
        {#if !dockOpen && chatView}
          <button onclick={() => (dockOpen = true)} class="px-3 py-3 glass-panel hover:bg-white/10 text-sm" title="Show the workspace: live metrics, side tasks, prompts, pins (Ctrl+.)">🧰</button>
        {/if}
        <button
          onclick={sendMessage}
          disabled={!userInput.trim()}
          title={isProcessing ? 'Run as a side task beside the current reply' : 'Send'}
          class="px-6 py-3 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg font-medium transition-all duration-200 flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed hover:scale-105 active:scale-95"
        >
          {#if isProcessing}
            <svg class="animate-spin h-5 w-5" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            {#if userInput.trim()}<span>⚡ Side task</span>{/if}
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

  <!-- Workspace dock: mounted once, shown beside Home and the conversation. -->
  <div class="z-10 py-4 pr-4 flex min-h-0" class:hidden={!dockOpen || !chatView}>
    <WorkspaceDock
      bind:this={dock}
      {opsTick}
      notify={addNotification}
      onChat={(text) => {
        if (!chatView) currentView = 'home';
        userInput = text;
        sendMessage();
      }}
      onInsert={insertIntoInput}
      onOpenRules={() => (currentView = 'system')}
      onTaskDone={onSideTaskDone}
      onClose={() => (dockOpen = false)}
    />
  </div>
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

  /*
   * Home panel: frosted glass over slow, blurred colour blobs tinted by the
   * avatar's mood. Blobs drift faster while a request runs.
   */
  @property --fluid { syntax: '<color>'; inherits: true; initial-value: #29d8ff; }
  @property --fluid-soft { syntax: '<color>'; inherits: true; initial-value: rgba(41, 216, 255, 0.35); }
  .fluid-glass {
    position: relative;
    overflow: hidden;
    border-radius: 28px;
    background:
      linear-gradient(145deg, rgba(255, 255, 255, 0.09), rgba(255, 255, 255, 0.02) 40%, rgba(255, 255, 255, 0.05));
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow:
      0 30px 80px -20px rgba(0, 0, 0, 0.6),
      inset 0 1px 0 rgba(255, 255, 255, 0.18),
      inset 0 0 60px -30px var(--fluid-soft);
    -webkit-backdrop-filter: blur(24px) saturate(160%);
    backdrop-filter: blur(24px) saturate(160%);
    transition: --fluid 1s ease, --fluid-soft 1s ease;
  }
  /* A soft specular sheen across the top of the glass. */
  .fluid-glass::before {
    content: '';
    position: absolute;
    inset: 0 0 60% 0;
    background: linear-gradient(to bottom, rgba(255, 255, 255, 0.07), transparent);
    pointer-events: none;
    z-index: 1;
  }
  .fluid-blobs {
    position: absolute;
    inset: -20%;
    filter: blur(70px) saturate(140%);
    opacity: 0.55;
    pointer-events: none;
  }
  .blob {
    position: absolute;
    width: 45%;
    aspect-ratio: 1;
    border-radius: 42% 58% 63% 37% / 45% 40% 60% 55%;
    mix-blend-mode: screen;
    animation: blob-drift 26s ease-in-out infinite, blob-morph 14s ease-in-out infinite;
  }
  .blob.b1 { top: 5%; left: 25%; background: var(--fluid); }
  .blob.b2 { top: 40%; left: 5%; background: #3b5bff; animation-delay: -7s, -3s; animation-duration: 31s, 17s; }
  .blob.b3 { top: 45%; left: 50%; background: #9b5cff; animation-delay: -15s, -9s; animation-duration: 29s, 12s; }
  .blob.b4 { top: 0%; left: 60%; width: 30%; background: var(--fluid-soft); animation-delay: -4s, -6s; animation-duration: 22s, 10s; }
  .fluid-glass.busy .blob { animation-duration: 9s, 5s; }
  .fluid-glass.busy .fluid-blobs { opacity: 0.75; }
  @keyframes blob-drift {
    0%, 100% { transform: translate(0, 0) rotate(0deg) scale(1); }
    33% { transform: translate(18%, 12%) rotate(60deg) scale(1.15); }
    66% { transform: translate(-14%, 8%) rotate(-40deg) scale(0.9); }
  }
  @keyframes blob-morph {
    0%, 100% { border-radius: 42% 58% 63% 37% / 45% 40% 60% 55%; }
    50% { border-radius: 60% 40% 35% 65% / 55% 62% 38% 45%; }
  }
  /* Messages fade out under the avatar instead of hitting a hard edge. */
  .chat-fade {
    -webkit-mask-image: linear-gradient(to bottom, transparent 0, #000 28px);
    mask-image: linear-gradient(to bottom, transparent 0, #000 28px);
  }
  .glass-chip {
    padding: 0.5rem 1rem;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    -webkit-backdrop-filter: blur(12px);
    backdrop-filter: blur(12px);
    font-size: 0.875rem;
    transition: background-color 0.2s ease, transform 0.2s ease, border-color 0.2s ease;
  }
  button.glass-chip:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
    border-color: var(--fluid-soft);
    transform: translateY(-1px);
  }
  @media (prefers-reduced-motion: reduce) {
    .blob { animation: none; }
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

  /* Per-message actions: quiet until you hover the message. */
  .msg-act {
    padding: 0.1rem 0.45rem;
    border-radius: 8px;
    transition: background-color 0.15s ease, color 0.15s ease;
  }
  .msg-act:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }

  /* Push-to-talk: a ring that swells with the live input level. */
  .mic-btn {
    transition: background-color 0.2s ease, transform 0.15s ease, box-shadow 0.2s ease;
  }
  .mic-btn.recording {
    transform: scale(1.08);
    box-shadow: 0 0 calc(8px + var(--level) * 28px) rgba(239, 68, 68, 0.75);
  }
  .mic-ring {
    position: absolute;
    inset: 0;
    border-radius: 9999px;
    border: 2px solid rgba(248, 113, 113, 0.8);
    transform: scale(calc(1 + var(--level) * 0.6));
    opacity: calc(0.35 + var(--level) * 0.65);
    transition: transform 0.08s linear, opacity 0.08s linear;
    pointer-events: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .mic-ring { transition: none; }
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
