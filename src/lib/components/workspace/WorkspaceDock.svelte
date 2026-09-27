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
   *
   * The notepad sits under every tab. Text size, accent and width come from
   * the page's display preferences (`$lib/display`).
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
  import {
    ACCENTS,
    AVATAR_MAX,
    AVATAR_MIN,
    TEXT_SIZES,
    accentColor,
    accentVars,
    clampAvatar,
    clampDock,
    DEFAULT_DISPLAY,
    type DisplayPrefs,
    type TextSize
  } from '$lib/display';
  import LiveMetrics from './LiveMetrics.svelte';
  import ActivityPanel from './ActivityPanel.svelte';
  import Notepad from './Notepad.svelte';

  export type Tab = 'live' | 'tasks' | 'prompts' | 'pins' | 'activity';

  let {
    opsTick = 0,
    visible = true,
    display = $bindable(),
    moodColor = '#22d3ee',
    notify,
    onChat,
    onInsert,
    onOpenRules,
    onTaskDone,
    onClose
  }: {
    /** Bumped by the page on every ops event. */
    opsTick?: number;
    /** False while the dock is hidden: live polling pauses. */
    visible?: boolean;
    display: DisplayPrefs;
    /** The avatar's current colour, for the `mood` accent. */
    moodColor?: string;
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

  // ------------------------------------------------------------ notepad
  let notepad = $state<ReturnType<typeof Notepad> | undefined>();
  /** Append text to the open note. */
  export function note(text: string, source?: string) {
    const clean = stripThinking(text).trim();
    if (clean) notepad?.append(clean, source);
  }

  /** Switch to a tab (from the command palette). */
  export function show(t: Tab) {
    tab = t;
  }
  export function openNote(id: string) {
    notepad?.open(id);
  }
  export function newNote() {
    notepad?.create();
  }

  // ------------------------------------------------------------ display
  let showDisplay = $state(false);
  /** Replace (not mutate) so the change reaches the page through `bind:display`. */
  function setDisplay(patch: Partial<DisplayPrefs>) {
    display = { ...display, ...patch };
  }
  let accent = $derived(accentColor(display.accent, moodColor));
  let dockStyle = $derived(
    `width: ${display.dockWidth}px; font-size: ${TEXT_SIZES[display.text].dock}px; ${accentVars(accent)}`
  );
  const sizes = Object.entries(TEXT_SIZES) as [TextSize, (typeof TEXT_SIZES)[TextSize]][];

  // Drag the left edge to resize the dock.
  let widthDrag: { x: number; w: number } | null = null;
  function edgeDown(e: PointerEvent) {
    widthDrag = { x: e.clientX, w: display.dockWidth };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function edgeMove(e: PointerEvent) {
    if (!widthDrag) return;
    setDisplay({ dockWidth: clampDock(widthDrag.w + (widthDrag.x - e.clientX), window.innerWidth) });
  }
  function edgeKey(e: KeyboardEvent) {
    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
      e.preventDefault();
      setDisplay({ dockWidth: clampDock(display.dockWidth + (e.key === 'ArrowLeft' ? 24 : -24), window.innerWidth) });
    }
  }

  const tabs: { id: Tab; label: string; icon: string }[] = [
    { id: 'live', label: 'Live', icon: '📈' },
    { id: 'tasks', label: 'Tasks', icon: '⚡' },
    { id: 'prompts', label: 'Prompts', icon: '📚' },
    { id: 'pins', label: 'Pins', icon: '📌' },
    { id: 'activity', label: 'Ops', icon: '🛰️' }
  ];
</script>


<aside class="dock glass-panel flex flex-col min-h-0 animate-dock" style={dockStyle} aria-label="Workspace">
  <button
    type="button"
    class="edge"
    aria-label="Resize the workspace (←/→)"
    onpointerdown={edgeDown}
    onpointermove={edgeMove}
    onpointerup={() => (widthDrag = null)}
    onpointercancel={() => (widthDrag = null)}
    onkeydown={edgeKey}
    title="Drag to resize"
  ></button>

  <header class="flex items-center gap-1 px-2 pt-2 pb-1 border-b border-white/10 relative">
    <div class="flex flex-1 gap-0.5" role="tablist">
      {#each tabs as t (t.id)}
        <button
          role="tab"
          aria-selected={tab === t.id}
          class="tab {tab === t.id ? 'on' : ''}"
          onclick={() => (tab = t.id)}
          title={t.label}
        >
          <span>{t.icon}</span><span class="tab-label">{t.label}</span>
          {#if t.id === 'tasks' && running}<span class="badge">{running}</span>{/if}
          {#if t.id === 'pins' && pins.length}<span class="badge muted">{pins.length}</span>{/if}
        </button>
      {/each}
    </div>
    <button class="hdr-btn {showDisplay ? 'on' : ''}" onclick={() => (showDisplay = !showDisplay)} title="Display: text size, accent, avatar size" aria-label="Display settings" aria-expanded={showDisplay}>Aa</button>
    <button class="hdr-btn" onclick={onClose} title="Hide the workspace (Ctrl+.)" aria-label="Hide workspace">✕</button>

    {#if showDisplay}
      <div class="popover" role="dialog" aria-label="Display settings">
        <div class="pop-row">
          <span class="pop-label">Text size</span>
          <div class="seg" role="group" aria-label="Text size">
            {#each sizes as [id, s] (id)}
              <button class:on={display.text === id} onclick={() => setDisplay({ text: id })} style="font-size: {0.7 + Object.keys(TEXT_SIZES).indexOf(id) * 0.08}rem">{s.label}</button>
            {/each}
          </div>
        </div>
        <div class="pop-row">
          <span class="pop-label">Accent</span>
          <div class="flex gap-1.5">
            {#each ACCENTS as a (a.id)}
              <button
                class="swatch {display.accent === a.id ? 'on' : ''} {a.id === 'mood' ? 'mood' : ''}"
                style={a.id === 'mood' ? `--sw: ${moodColor}` : `--sw: ${a.color}`}
                onclick={() => setDisplay({ accent: a.id })}
                title={a.label}
                aria-label="Accent: {a.label}"
                aria-pressed={display.accent === a.id}
              ></button>
            {/each}
          </div>
        </div>
        <div class="pop-row">
          <label class="pop-label" for="avatar-size">Avatar</label>
          <div class="flex items-center gap-1.5 flex-1">
            <button class="mini-btn" onclick={() => setDisplay({ avatar: clampAvatar(display.avatar - 0.1) })} aria-label="Smaller avatar">−</button>
            <input id="avatar-size" type="range" min={AVATAR_MIN} max={AVATAR_MAX} step="0.05" value={display.avatar} oninput={(e) => setDisplay({ avatar: clampAvatar(Number(e.currentTarget.value)) })} class="flex-1 slider" />
            <button class="mini-btn" onclick={() => setDisplay({ avatar: clampAvatar(display.avatar + 0.1) })} aria-label="Larger avatar">＋</button>
            <span class="tabular-nums dim w-10 text-right">{Math.round(display.avatar * 100)}%</span>
          </div>
        </div>
        <div class="flex justify-between items-center pt-1">
          <span class="faint tiny">Width {display.dockWidth}px: drag the left edge</span>
          <button class="faint tiny hover:text-white" onclick={() => setDisplay({ ...DEFAULT_DISPLAY })}>Reset</button>
        </div>
      </div>
    {/if}
  </header>

  <div class="flex-1 min-h-0 overflow-auto p-3">
    <!-- Live stays mounted-but-idle when hidden so its history isn't refetched on every switch. -->
    <div hidden={tab !== 'live'}><LiveMetrics active={visible && tab === 'live'} runningTasks={running} {notify} /></div>

    {#if tab === 'tasks'}
      <div class="space-y-3">
        <div class="space-y-2">
          <textarea
            bind:value={taskInput}
            onkeydown={taskKey}
            rows="3"
            placeholder="Ask for something to run beside the chat… (Ctrl+Enter)&#10;Start with / to run a command, e.g. /monitor"
            class="field w-full resize-none"
          ></textarea>
          <div class="flex gap-2">
            <button class="btn primary flex-1" disabled={!taskInput.trim()} onclick={submitTask}>⚡ Run side task</button>
            <button class="btn" disabled={briefing} onclick={situationBrief} title="An AI brief of alerts, load, GPUs, models and schedules">📋 Brief</button>
          </div>
          <p class="faint tiny">Side tasks have no tools (they can't run commands or open files), so they never need approval and don't wait for the chat.</p>
        </div>

        {#if tasks.length}
          <div class="flex justify-between items-center">
            <span class="small dim">{running} running · {tasks.length - running} finished</span>
            <button class="small dim hover:text-white" onclick={() => (tasks = tasks.filter((t) => t.status === 'running'))}>Clear finished</button>
          </div>
        {/if}

        {#each tasks as t (t.id)}
          <article class="card {t.status}">
            <button class="w-full flex items-center gap-2 text-left" onclick={() => (expanded[t.id] = !expanded[t.id])}>
              <span class="status-dot {t.status}"></span>
              <span class="flex-1 truncate font-medium">{t.kind === 'command' ? '⌘ ' : ''}{t.title}</span>
              <span class="tiny faint tabular-nums">{elapsed(t)}</span>
            </button>
            {#if expanded[t.id]}
              {#if t.context}
                <details class="mt-1 tiny faint"><summary class="cursor-pointer">Context given ({t.context.length} chars)</summary><pre class="whitespace-pre-wrap max-h-32 overflow-auto">{t.context}</pre></details>
              {/if}
              {#if taskText(t)}
                <!-- Model and command output is untrusted: always sanitized by renderMarkdown. -->
                <div class="md-rich mt-2 max-h-80 overflow-auto">{@html renderMarkdown(taskText(t))}</div>
              {:else if t.status === 'running'}
                <p class="small accent mt-2 animate-pulse">{t.kind === 'command' ? 'Running… (approve any dialog that appears)' : 'Thinking…'}</p>
              {/if}
              {#if t.notes.length}
                <ul class="mt-1 tiny dim space-y-0.5">{#each t.notes as n}<li>{n}</li>{/each}</ul>
              {/if}
              <div class="flex flex-wrap gap-1 mt-2">
                {#if t.status === 'running' && t.kind === 'ai'}
                  <button class="chip" onclick={() => cancelTask(t)}>⏹ Stop</button>
                {/if}
                {#if taskText(t)}
                  <button class="chip" onclick={() => copy(taskText(t))}>📋 Copy</button>
                  <button class="chip" onclick={() => pin(taskText(t), t.title)}>📌 Pin</button>
                  <button class="chip" onclick={() => note(taskText(t), t.title)} title="Append to the open note">📝 Note</button>
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
          <p class="small faint text-center py-4">No side tasks yet. Try 📋 Brief, a prompt from 📚, or ⚡ on any chat reply.</p>
        {/each}
      </div>
    {:else if tab === 'prompts'}
      <div class="space-y-3">
        <textarea
          bind:value={promptInput}
          rows="2"
          placeholder="Fill-in for {'{{input}}'}: a topic, notes, a command…"
          class="field w-full resize-none"
        ></textarea>
        <ul class="space-y-1.5">
          {#each prompts as p (p.id)}
            <li class="card">
              <div class="flex items-center gap-2">
                <span class="text-lg">{p.icon}</span>
                <span class="flex-1 min-w-0">
                  <span class="block font-medium truncate">{p.title}</span>
                  <span class="block tiny faint truncate" title={p.text}>{p.text}</span>
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
            <div class="flex items-center gap-2 small">
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
            <button class="small faint hover:text-white" onclick={resetPrompts}>Reset to defaults</button>
          </div>
        {/if}
      </div>
    {:else if tab === 'pins'}
      <div class="space-y-2">
        {#each pins as p (p.id)}
          <article class="card">
            <div class="flex items-center gap-2 tiny faint mb-1">
              <span class="flex-1 truncate">📌 {p.source}</span><span>{ago(p.pinned)}</span>
            </div>
            <!-- Pinned text came from the model or a command: sanitized. -->
            <div class="md-rich overflow-hidden {pinOpen[p.id] ? '' : 'clamp'}">{@html renderMarkdown(p.text)}</div>
            <div class="flex flex-wrap gap-1 mt-2">
              <button class="chip" onclick={() => (pinOpen[p.id] = !pinOpen[p.id])}>{pinOpen[p.id] ? 'Less' : 'More'}</button>
              <button class="chip" onclick={() => copy(p.text)}>📋 Copy</button>
              <button class="chip" onclick={() => note(p.text, p.source)} title="Append to the open note">📝 Note</button>
              <button class="chip" onclick={() => onInsert(p.text)}>💬 To chat</button>
              <button class="chip" onclick={() => pinToMemory(p)} title="Save to long-term memory (kb-core)">🧠 Remember</button>
              <button class="chip" onclick={() => unpin(p.id)}>✕</button>
            </div>
          </article>
        {:else}
          <p class="small faint text-center py-4">Pin replies and task results with 📌 to keep them handy. Pins stay on this computer.</p>
        {/each}
      </div>
    {:else if tab === 'activity'}
      <ActivityPanel active={visible && tab === 'activity'} refreshKey={opsTick} {notify} {onOpenRules} />
    {/if}
  </div>

  <Notepad
    bind:this={notepad}
    {notify}
    {onInsert}
    onTask={(instruction, opts) => runTask(instruction, opts)}
    onPin={(text, source) => pin(text, source)}
  />
</aside>

<style>
  /* Text colours for the dark glass: body ≈ 15:1, dim ≈ 10:1, faint ≈ 7:1. */
  .dock {
    --text: #eef1f6;
    --text-dim: #c3cad6;
    --text-faint: #9aa3b4;
    --tile: rgba(255, 255, 255, 0.05);
    --tile-border: rgba(255, 255, 255, 0.08);
    --c-cpu: #38bdf8;
    --c-mem: #a78bfa;
    --c-gpu: #34d399;
    --c-net: #fbbf24;
    position: relative;
    flex-shrink: 0;
    max-width: 50vw;
    color: var(--text);
    line-height: 1.45;
  }
  .edge {
    position: absolute;
    left: -4px;
    top: 16px;
    bottom: 16px;
    width: 8px;
    cursor: ew-resize;
    touch-action: none;
    z-index: 5;
    border-radius: 9999px;
    transition: background-color 0.15s ease;
  }
  .edge:hover,
  .edge:focus-visible { background: var(--accent-line); outline: none; }
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
    color: var(--text-faint);
    transition: background-color 0.15s ease, color 0.15s ease;
  }
  .tab-label { font-size: 0.8em; font-weight: 500; }
  .tab:hover { background: rgba(255, 255, 255, 0.06); color: var(--text); }
  .tab.on { background: var(--accent-soft); color: var(--accent); box-shadow: inset 0 -2px 0 var(--accent); }
  .hdr-btn { padding: 0.15rem 0.45rem; border-radius: 8px; color: var(--text-faint); font-size: 0.86em; font-weight: 600; }
  .hdr-btn:hover { color: var(--text); background: rgba(255, 255, 255, 0.07); }
  .hdr-btn.on { color: var(--accent); background: var(--accent-soft); }
  .badge {
    position: absolute;
    top: 0;
    right: 0.3rem;
    min-width: 1rem;
    padding: 0 0.25rem;
    border-radius: 9999px;
    background: var(--accent);
    color: #0a0a0f;
    font-size: 0.62rem;
    font-weight: 700;
    line-height: 1rem;
  }
  .badge.muted { background: rgba(255, 255, 255, 0.25); color: white; }
  .popover {
    position: absolute;
    top: calc(100% + 6px);
    right: 0.5rem;
    left: 0.5rem;
    z-index: 20;
    padding: 0.75rem;
    border-radius: 14px;
    background: rgba(18, 22, 34, 0.96);
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 18px 50px -12px rgba(0, 0, 0, 0.7);
    -webkit-backdrop-filter: blur(18px);
    backdrop-filter: blur(18px);
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    font-size: 0.9em;
  }
  .pop-row { display: flex; align-items: center; gap: 0.6rem; }
  .pop-label { width: 4.5rem; flex-shrink: 0; color: var(--text-dim); font-weight: 500; }
  .seg { display: inline-flex; border-radius: 9px; background: rgba(255, 255, 255, 0.06); padding: 2px; }
  .seg button { min-width: 2rem; padding: 0.1rem 0.45rem; border-radius: 7px; color: var(--text-dim); font-weight: 600; }
  .seg button.on { background: var(--accent-soft); color: var(--accent); }
  .swatch {
    width: 1.35rem;
    height: 1.35rem;
    border-radius: 9999px;
    background: var(--sw);
    border: 2px solid transparent;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.15);
    transition: transform 0.12s ease;
  }
  .swatch.mood { background: conic-gradient(from 0deg, #22d3ee, #a78bfa, #fb7185, #fbbf24, #34d399, #22d3ee); }
  .swatch:hover { transform: scale(1.12); }
  .swatch.on { border-color: #fff; box-shadow: 0 0 0 2px var(--sw), 0 0 12px var(--sw); }
  .slider { accent-color: var(--accent); }
  .mini-btn { width: 1.4rem; height: 1.4rem; border-radius: 7px; background: rgba(255, 255, 255, 0.07); color: var(--text); line-height: 1; }
  .mini-btn:hover { background: rgba(255, 255, 255, 0.15); }
  .card {
    padding: 0.65rem 0.75rem;
    border-radius: 12px;
    background: var(--tile);
    border: 1px solid var(--tile-border);
  }
  .card.running { border-color: var(--accent-line); }
  .card.error { border-color: rgba(248, 113, 113, 0.45); }
  .status-dot { width: 0.5rem; height: 0.5rem; border-radius: 9999px; flex-shrink: 0; background: #4ade80; }
  .status-dot.running { background: var(--accent); animation: pulse 1.2s ease-in-out infinite; }
  .status-dot.error { background: #f87171; }
  .status-dot.cancelled { background: #9ca3af; }
  @keyframes pulse { 50% { opacity: 0.3; } }
  .chip {
    padding: 0.15rem 0.55rem;
    border-radius: 8px;
    font-size: 0.82em;
    color: var(--text-dim);
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }
  .chip:hover { background: rgba(255, 255, 255, 0.14); color: var(--text); }
  .chip.hot { border-color: var(--accent-line); color: var(--text); }
  .btn {
    padding: 0.4rem 0.75rem;
    border-radius: 10px;
    font-size: 0.9em;
    font-weight: 500;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.1);
  }
  .btn:hover:not(:disabled) { background: rgba(255, 255, 255, 0.14); }
  .btn.primary { background: var(--accent-soft); border-color: var(--accent-line); color: #fff; }
  .btn.primary:hover:not(:disabled) { background: var(--accent-line); }
  .btn:disabled { opacity: 0.45; cursor: not-allowed; }
  .field {
    font-size: 0.93em;
    color: var(--text);
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 9px;
    padding: 0.4rem 0.6rem;
  }
  .field::placeholder { color: var(--text-faint); opacity: 0.85; }
  .field:focus { outline: none; border-color: var(--accent-line); }
  .small { font-size: 0.86em; }
  .tiny { font-size: 0.8em; }
  .dim { color: var(--text-dim); }
  .faint { color: var(--text-faint); }
  .accent { color: var(--accent); }
  .clamp { max-height: 6.5rem; mask-image: linear-gradient(to bottom, #000 60%, transparent); }
  @media (prefers-reduced-motion: reduce) {
    .animate-dock, .status-dot.running { animation: none; }
  }
</style>
