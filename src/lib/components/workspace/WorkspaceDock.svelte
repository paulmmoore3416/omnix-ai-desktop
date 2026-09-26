<script lang="ts">
  /**
   * Workspace dock beside Home and the conversation: live metrics, side
   * tasks that run in parallel with the chat, a prompt library, a pinboard
   * and the ops feed.
   *
   * Side tasks are tool-less (`task_run`): they can't run commands or touch
   * files, so they need no approvals and never wait on the chat. A task that
   * starts with `/` runs as a slash command (`process_command`) through the
   * normal policy → approval → audit path.
   */
  import { Channel } from '@tauri-apps/api/core';
  import { call, errorMessage } from '$lib/api';
  import { ago } from '$lib/format';
  import { renderMarkdown } from '$lib/markdown';
  import type { PerformanceData, SystemControlData, UiEvent } from '$lib/types';
  import {
    BRIEF_INSTRUCTION,
    DEFAULT_PROMPTS,
    addPin,
    briefContext,
    copyText,
    fillPrompt,
    loadPins,
    loadPrompts,
    newTaskId,
    savePins,
    savePrompts,
    stripThinking,
    taskTitle,
    type Pin,
    type SavedPrompt,
    type SideTask
  } from '$lib/workspace';
  import LiveMetrics from './LiveMetrics.svelte';
  import ActivityPanel from './ActivityPanel.svelte';

  type Tab = 'live' | 'tasks' | 'prompts' | 'pins' | 'activity';

  let {
    opsTick = 0,
    notify,
    onChat,
    onInsert,
    onOpenRules,
    onTaskDone,
    onClose
  }: {
    /** Bumped by the page on every ops event. */
    opsTick?: number;
    notify: (msg: string, type?: 'info' | 'success' | 'error') => void;
    /** Send text to the main chat now. */
    onChat: (text: string) => void;
    /** Put text in the chat input for editing. */
    onInsert: (text: string) => void;
    onOpenRules: () => void;
    onTaskDone?: (task: SideTask) => void;
    onClose: () => void;
  } = $props();

  const TAB_KEY = 'omnix.workspace.tab';
  function initialTab(): Tab {
    try {
      const t = localStorage.getItem(TAB_KEY);
      if (t && ['live', 'tasks', 'prompts', 'pins', 'activity'].includes(t)) return t as Tab;
    } catch {
      /* storage unavailable */
    }
    return 'live';
  }
  let tab = $state<Tab>(initialTab());
  $effect(() => {
    try {
      localStorage.setItem(TAB_KEY, tab);
    } catch {
      /* storage unavailable */
    }
  });

  // ------------------------------------------------------------ side tasks
  let tasks = $state<SideTask[]>([]);
  let taskInput = $state('');
  let running = $derived(tasks.filter((t) => t.status === 'running').length);
  let expanded = $state<Record<string, boolean>>({});
  let briefing = $state(false);

  /** Start a side task. Returns once it has finished. */
  export async function runTask(instruction: string, opts: { title?: string; context?: string } = {}) {
    const text = instruction.trim();
    if (!text) return;
    const kind = text.startsWith('/') ? 'command' : 'ai';
    tasks.unshift({
      id: newTaskId(),
      kind,
      title: opts.title ?? taskTitle(text),
      instruction: text,
      context: opts.context,
      output: '',
      notes: [],
      status: 'running',
      started: Date.now()
    });
    const task = tasks[0];
    expanded[task.id] = true;
    tab = 'tasks';
    try {
      if (kind === 'command') {
        task.output = await call<string>('process_command', { command: text });
      } else {
        const channel = new Channel<UiEvent>();
        channel.onmessage = (ev) => {
          if (ev.type === 'token') task.output += ev.text;
          else if (ev.type === 'notice') task.notes.push(`ℹ ${ev.message}`);
          else if (ev.type === 'error' && !task.notes.includes(`✗ ${ev.message}`)) task.notes.push(`✗ ${ev.message}`);
        };
        await call('task_run', { id: task.id, instruction: text, context: opts.context ?? null, onEvent: channel });
      }
      if (task.status === 'running') task.status = 'done';
    } catch (e) {
      if (task.status === 'running') task.status = 'error';
      const msg = `✗ ${errorMessage(e)}`;
      if (!task.notes.includes(msg)) task.notes.push(msg);
    } finally {
      task.finished = Date.now();
      onTaskDone?.(task);
    }
  }

  async function cancelTask(t: SideTask) {
    try {
      await call('task_cancel', { id: t.id });
      t.status = 'cancelled';
    } catch (e) {
      notify(errorMessage(e), 'error');
    }
  }

  function submitTask() {
    const text = taskInput;
    taskInput = '';
    runTask(text);
  }

  function taskKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      submitTask();
    }
  }

  async function situationBrief() {
    if (briefing) return;
    briefing = true;
    try {
      const [perf, ops] = await Promise.all([
        call<PerformanceData>('get_performance', { historySecs: 300 }).catch(() => null),
        call<SystemControlData>('get_system_control_data').catch(() => null)
      ]);
      if (!perf && !ops) throw new Error('metrics are unavailable');
      briefing = false; // the snapshot is taken; the task card shows progress
      await runTask(BRIEF_INSTRUCTION, { title: '📋 Situation brief', context: briefContext(perf, ops) });
    } catch (e) {
      notify(`Situation brief: ${errorMessage(e)}`, 'error');
    } finally {
      briefing = false;
    }
  }
  /** Exposed so Home can offer the brief as a quick action. */
  export function brief() {
    situationBrief();
  }

  function taskText(t: SideTask) {
    return stripThinking(t.output).trim();
  }

  function elapsed(t: SideTask) {
    const s = Math.round(((t.finished ?? now) - t.started) / 1000);
    return s < 60 ? `${s}s` : `${Math.floor(s / 60)}m ${s % 60}s`;
  }
  // Ticks the elapsed time of running tasks.
  let now = $state(Date.now());
  $effect(() => {
    if (!running) return;
    const t = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(t);
  });

  // ------------------------------------------------------------ pins
  let pins = $state<Pin[]>(loadPins());
  let pinOpen = $state<Record<string, boolean>>({});

  /** Pin text (a reply, a task result) to the board. */
  export function pin(text: string, source: string) {
    const clean = stripThinking(text).trim();
    if (!clean) return;
    pins = addPin(pins, clean, source);
    savePins(pins);
    notify('Pinned to the workspace board', 'success');
  }

  function unpin(id: string) {
    pins = pins.filter((p) => p.id !== id);
    savePins(pins);
  }

  async function pinToMemory(p: Pin) {
    try {
      await call('save_memory', {
        content: p.text.slice(0, 20_000),
        tags: ['pinned'],
        importance: 6,
        category: 'general',
        collection: null
      });
      notify('Saved to long-term memory', 'success');
    } catch (e) {
      notify(`Save to memory: ${errorMessage(e)}`, 'error');
    }
  }

  async function copy(text: string) {
    if (await copyText(text)) notify('Copied', 'success');
    else notify('Copy is not allowed here: select the text instead', 'error');
  }

  // ------------------------------------------------------------ prompts
  let prompts = $state<SavedPrompt[]>(loadPrompts());
  let promptInput = $state('');
  let editing = $state<SavedPrompt | null>(null);

  function usePrompt(p: SavedPrompt, target: 'chat' | 'task') {
    const text = fillPrompt(p.text, promptInput);
    if (p.text.includes('{{input}}') && !promptInput.trim()) {
      notify(`“${p.title}” needs something in the box above`, 'info');
      return;
    }
    promptInput = '';
    if (target === 'chat') onChat(text);
    else runTask(text, { title: `${p.icon} ${p.title}` });
  }

  function newPrompt() {
    editing = { id: `u${Date.now().toString(36)}`, title: '', icon: '⭐', text: '', target: 'task' };
  }

  function savePrompt() {
    if (!editing || !editing.title.trim() || !editing.text.trim()) {
      notify('A prompt needs a title and text', 'info');
      return;
    }
    const p = { ...editing, title: editing.title.trim(), text: editing.text.trim(), builtin: false };
    const i = prompts.findIndex((x) => x.id === p.id);
    prompts = i >= 0 ? prompts.map((x) => (x.id === p.id ? p : x)) : [...prompts, p];
    savePrompts(prompts);
    editing = null;
  }

  function deletePrompt(id: string) {
    prompts = prompts.filter((p) => p.id !== id);
    savePrompts(prompts);
  }

  function resetPrompts() {
    prompts = DEFAULT_PROMPTS.map((p) => ({ ...p }));
    savePrompts(prompts);
  }

  const tabs: { id: Tab; label: string; icon: string }[] = [
    { id: 'live', label: 'Live', icon: '📈' },
    { id: 'tasks', label: 'Tasks', icon: '⚡' },
    { id: 'prompts', label: 'Prompts', icon: '📚' },
    { id: 'pins', label: 'Pins', icon: '📌' },
    { id: 'activity', label: 'Ops', icon: '🛰️' }
  ];
</script>

<aside class="dock glass-panel flex flex-col min-h-0 animate-dock" aria-label="Workspace">
  <header class="flex items-center gap-1 px-2 pt-2 pb-1 border-b border-white/10">
    <div class="flex flex-1 gap-0.5" role="tablist">
      {#each tabs as t (t.id)}
        <button
          role="tab"
          aria-selected={tab === t.id}
          class="tab {tab === t.id ? 'on' : ''}"
          onclick={() => (tab = t.id)}
          title={t.label}
        >
          <span>{t.icon}</span><span class="text-[11px]">{t.label}</span>
          {#if t.id === 'tasks' && running}<span class="badge">{running}</span>{/if}
          {#if t.id === 'pins' && pins.length}<span class="badge muted">{pins.length}</span>{/if}
        </button>
      {/each}
    </div>
    <button class="px-2 text-gray-400 hover:text-white" onclick={onClose} title="Hide the workspace (Ctrl+.)" aria-label="Hide workspace">✕</button>
  </header>

  <div class="flex-1 min-h-0 overflow-auto p-3">
    <!-- Live stays mounted-but-idle when hidden so its history isn't refetched on every switch. -->
    <div hidden={tab !== 'live'}><LiveMetrics active={tab === 'live'} runningTasks={running} {notify} /></div>

    {#if tab === 'tasks'}
      <div class="space-y-3">
        <div class="space-y-2">
          <textarea
            bind:value={taskInput}
            onkeydown={taskKey}
            rows="3"
            placeholder="Ask for something to run beside the chat… (Ctrl+Enter)&#10;Start with / to run a command, e.g. /monitor"
            class="w-full text-sm bg-white/5 border border-white/10 rounded-lg px-3 py-2 placeholder-gray-500 focus:outline-none focus:border-cosmic-cyan resize-none"
          ></textarea>
          <div class="flex gap-2">
            <button class="btn primary flex-1" disabled={!taskInput.trim()} onclick={submitTask}>⚡ Run side task</button>
            <button class="btn" disabled={briefing} onclick={situationBrief} title="An AI brief of alerts, load, GPUs, models and schedules">📋 Brief</button>
          </div>
          <p class="text-[11px] text-gray-500">Side tasks have no tools (they can't run commands or open files), so they never need approval and don't wait for the chat.</p>
        </div>

        {#if tasks.length}
          <div class="flex justify-between items-center">
            <span class="text-xs text-gray-400">{running} running · {tasks.length - running} finished</span>
            <button class="text-xs text-gray-400 hover:text-white" onclick={() => (tasks = tasks.filter((t) => t.status === 'running'))}>Clear finished</button>
          </div>
        {/if}

        {#each tasks as t (t.id)}
          <article class="card {t.status}">
            <button class="w-full flex items-center gap-2 text-left" onclick={() => (expanded[t.id] = !expanded[t.id])}>
              <span class="status-dot {t.status}"></span>
              <span class="flex-1 truncate text-sm font-medium">{t.kind === 'command' ? '⌘ ' : ''}{t.title}</span>
              <span class="text-[11px] text-gray-500 tabular-nums">{elapsed(t)}</span>
            </button>
            {#if expanded[t.id]}
              {#if t.context}
                <details class="mt-1 text-[11px] text-gray-500"><summary class="cursor-pointer">Context given ({t.context.length} chars)</summary><pre class="whitespace-pre-wrap max-h-32 overflow-auto">{t.context}</pre></details>
              {/if}
              {#if taskText(t)}
                <!-- Model and command output is untrusted: always sanitized by renderMarkdown. -->
                <div class="md-content text-sm mt-2 max-h-80 overflow-auto">{@html renderMarkdown(taskText(t))}</div>
              {:else if t.status === 'running'}
                <p class="text-xs text-cosmic-cyan mt-2 animate-pulse">{t.kind === 'command' ? 'Running… (approve any dialog that appears)' : 'Thinking…'}</p>
              {/if}
              {#if t.notes.length}
                <ul class="mt-1 text-[11px] text-gray-400 space-y-0.5">{#each t.notes as n}<li>{n}</li>{/each}</ul>
              {/if}
              <div class="flex flex-wrap gap-1 mt-2">
                {#if t.status === 'running' && t.kind === 'ai'}
                  <button class="chip" onclick={() => cancelTask(t)}>⏹ Stop</button>
                {/if}
                {#if taskText(t)}
                  <button class="chip" onclick={() => copy(taskText(t))}>📋 Copy</button>
                  <button class="chip" onclick={() => pin(taskText(t), t.title)}>📌 Pin</button>
                  <button class="chip" onclick={() => onInsert(taskText(t))} title="Put the result in the chat box to use it in the conversation">💬 To chat</button>
                {/if}
                {#if t.status !== 'running'}
                  <button class="chip" onclick={() => runTask(t.instruction, { title: t.title, context: t.context })}>↻ Again</button>
                  <button class="chip" onclick={() => (tasks = tasks.filter((x) => x.id !== t.id))}>✕</button>
                {/if}
              </div>
            {/if}
          </article>
        {:else}
          <p class="text-xs text-gray-500 text-center py-4">No side tasks yet. Try 📋 Brief, a prompt from 📚, or ⚡ on any chat reply.</p>
        {/each}
      </div>
    {:else if tab === 'prompts'}
      <div class="space-y-3">
        <textarea
          bind:value={promptInput}
          rows="2"
          placeholder="Fill-in for {'{{input}}'}: a topic, notes, a command…"
          class="w-full text-sm bg-white/5 border border-white/10 rounded-lg px-3 py-2 placeholder-gray-500 focus:outline-none focus:border-cosmic-cyan resize-none"
        ></textarea>
        <ul class="space-y-1.5">
          {#each prompts as p (p.id)}
            <li class="card">
              <div class="flex items-center gap-2">
                <span class="text-lg">{p.icon}</span>
                <span class="flex-1 min-w-0">
                  <span class="block text-sm font-medium truncate">{p.title}</span>
                  <span class="block text-[11px] text-gray-500 truncate" title={p.text}>{p.text}</span>
                </span>
              </div>
              <div class="flex gap-1 mt-1.5">
                <button class="chip {p.target === 'task' ? 'hot' : ''}" onclick={() => usePrompt(p, 'task')} title="Run beside the chat (no tools)">⚡ Task</button>
                <button class="chip {p.target === 'chat' ? 'hot' : ''}" onclick={() => usePrompt(p, 'chat')} title="Send to the main chat (tools available)">💬 Chat</button>
                <span class="flex-1"></span>
                <button class="chip" onclick={() => (editing = { ...p })} title="Edit">✎</button>
                <button class="chip" onclick={() => deletePrompt(p.id)} title="Delete">🗑</button>
              </div>
            </li>
          {/each}
        </ul>
        {#if editing}
          <div class="card space-y-2">
            <div class="flex gap-2">
              <input bind:value={editing.icon} maxlength="4" class="field w-12 text-center" aria-label="Icon" />
              <input bind:value={editing.title} maxlength="60" placeholder="Title" class="field flex-1" aria-label="Title" />
            </div>
            <textarea bind:value={editing.text} rows="4" placeholder={'Prompt text. Use {{input}} where the fill-in goes.'} class="field w-full resize-y" aria-label="Prompt text"></textarea>
            <div class="flex items-center gap-2 text-xs">
              <label class="flex items-center gap-1"><input type="radio" bind:group={editing.target} value="task" /> Side task</label>
              <label class="flex items-center gap-1"><input type="radio" bind:group={editing.target} value="chat" /> Chat</label>
              <span class="flex-1"></span>
              <button class="btn" onclick={() => (editing = null)}>Cancel</button>
              <button class="btn primary" onclick={savePrompt}>Save</button>
            </div>
          </div>
        {:else}
          <div class="flex justify-between">
            <button class="btn" onclick={newPrompt}>＋ New prompt</button>
            <button class="text-xs text-gray-500 hover:text-white" onclick={resetPrompts}>Reset to defaults</button>
          </div>
        {/if}
      </div>
    {:else if tab === 'pins'}
      <div class="space-y-2">
        {#each pins as p (p.id)}
          <article class="card">
            <div class="flex items-center gap-2 text-[11px] text-gray-500 mb-1">
              <span class="flex-1 truncate">📌 {p.source}</span><span>{ago(p.pinned)}</span>
            </div>
            <!-- Pinned text came from the model or a command: sanitized. -->
            <div class="md-content text-sm overflow-hidden {pinOpen[p.id] ? '' : 'clamp'}">{@html renderMarkdown(p.text)}</div>
            <div class="flex flex-wrap gap-1 mt-2">
              <button class="chip" onclick={() => (pinOpen[p.id] = !pinOpen[p.id])}>{pinOpen[p.id] ? 'Less' : 'More'}</button>
              <button class="chip" onclick={() => copy(p.text)}>📋 Copy</button>
              <button class="chip" onclick={() => onInsert(p.text)}>💬 To chat</button>
              <button class="chip" onclick={() => pinToMemory(p)} title="Save to long-term memory (kb-core)">🧠 Remember</button>
              <button class="chip" onclick={() => unpin(p.id)}>✕</button>
            </div>
          </article>
        {:else}
          <p class="text-xs text-gray-500 text-center py-4">Pin replies and task results with 📌 to keep them handy. Pins stay on this computer.</p>
        {/each}
      </div>
    {:else if tab === 'activity'}
      <ActivityPanel active={tab === 'activity'} refreshKey={opsTick} {notify} {onOpenRules} />
    {/if}
  </div>
</aside>

<style>
  .dock {
    width: 23rem;
    flex-shrink: 0;
  }
  @keyframes dock-in {
    from { opacity: 0; transform: translateX(16px); }
    to { opacity: 1; transform: translateX(0); }
  }
  .animate-dock { animation: dock-in 0.25s ease-out; }
  .tab {
    display: flex;
    flex-direction: column;
    align-items: center;
    flex: 1;
    position: relative;
    padding: 0.3rem 0.2rem;
    border-radius: 10px;
    color: rgb(156 163 175);
    transition: background-color 0.15s ease, color 0.15s ease;
  }
  .tab:hover { background: rgba(255, 255, 255, 0.06); color: white; }
  .tab.on { background: rgba(0, 212, 255, 0.15); color: #00ffff; }
  .badge {
    position: absolute;
    top: 0;
    right: 0.3rem;
    min-width: 1rem;
    padding: 0 0.25rem;
    border-radius: 9999px;
    background: #00ffff;
    color: #0a0a0f;
    font-size: 0.6rem;
    font-weight: 700;
    line-height: 1rem;
  }
  .badge.muted { background: rgba(255, 255, 255, 0.25); color: white; }
  .card {
    padding: 0.6rem 0.7rem;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }
  .card.running { border-color: rgba(0, 212, 255, 0.4); }
  .card.error { border-color: rgba(248, 113, 113, 0.4); }
  .status-dot { width: 0.5rem; height: 0.5rem; border-radius: 9999px; flex-shrink: 0; background: #4ade80; }
  .status-dot.running { background: #00d4ff; animation: pulse 1.2s ease-in-out infinite; }
  .status-dot.error { background: #f87171; }
  .status-dot.cancelled { background: #9ca3af; }
  @keyframes pulse { 50% { opacity: 0.3; } }
  .chip {
    padding: 0.15rem 0.5rem;
    border-radius: 8px;
    font-size: 0.72rem;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }
  .chip:hover { background: rgba(255, 255, 255, 0.14); }
  .chip.hot { border-color: rgba(0, 212, 255, 0.5); }
  .btn {
    padding: 0.4rem 0.75rem;
    border-radius: 10px;
    font-size: 0.8rem;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.1);
  }
  .btn:hover:not(:disabled) { background: rgba(255, 255, 255, 0.14); }
  .btn.primary { background: rgba(0, 212, 255, 0.25); border-color: rgba(0, 212, 255, 0.5); }
  .btn.primary:hover:not(:disabled) { background: rgba(0, 212, 255, 0.4); }
  .btn:disabled { opacity: 0.45; cursor: not-allowed; }
  .field {
    font-size: 0.8rem;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 0.35rem 0.5rem;
  }
  .clamp { max-height: 6.5rem; mask-image: linear-gradient(to bottom, #000 60%, transparent); }
  .md-content :global(pre) { background: rgba(0, 0, 0, 0.35); padding: 0.5rem; border-radius: 0.4rem; overflow-x: auto; margin: 0.4rem 0; }
  .md-content :global(code) { font-family: ui-monospace, monospace; font-size: 0.85em; }
  .md-content :global(p) { margin: 0.2rem 0; }
  .md-content :global(ul), .md-content :global(ol) { padding-left: 1.1rem; list-style: disc; }
  .md-content :global(.md-link) { text-decoration: underline; color: #7dd3fc; cursor: help; }
  @media (prefers-reduced-motion: reduce) {
    .animate-dock, .status-dot.running { animation: none; }
  }
</style>
