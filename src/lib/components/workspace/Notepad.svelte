<script lang="ts">
  /**
   * Resizable Markdown notepad with a clipboard history, docked at the
   * bottom of the workspace. Drag the top edge to resize; the height, mode
   * and notes persist on this computer.
   *
   * Notes are the user's own text, but the preview still goes through
   * `renderMarkdown` (DOMPurify): notes often hold pasted model output, and
   * clips come from replies and commands.
   *
   * A note can be kept in memory (🔗): it is indexed into kb-core's `notes`
   * knowledge base and re-synced a few seconds after each edit, so chat
   * recall and search find it. kb-core only re-embeds the edited chunks.
   */
  import { onDestroy } from 'svelte';
  import { call, errorMessage } from '$lib/api';
  import { ago } from '$lib/format';
  import { renderMarkdown } from '$lib/markdown';
  import { copyText } from '$lib/workspace';
  import {
    MAX_NOTES,
    MAX_NOTE_CHARS,
    appendText,
    applyFormat,
    clampHeight,
    counts,
    loadClips,
    loadLayout,
    loadNotes,
    newNote,
    noteTitle,
    onClips,
    saveClips,
    saveLayout,
    saveNotes,
    taskGlyphs,
    toggleTask,
    type Clip,
    type Edit
  } from '$lib/notepad';

  let {
    notify,
    onInsert,
    onTask,
    onPin
  }: {
    notify: (msg: string, type?: 'info' | 'success' | 'error') => void;
    /** Put text in the chat box. */
    onInsert: (text: string) => void;
    /** Run a side task over the note. */
    onTask: (instruction: string, opts: { title: string; context: string }) => void;
    onPin: (text: string, source: string) => void;
  } = $props();

  const initial = loadNotes();
  let notes = $state(initial.notes);
  let activeId = $state(initial.active);
  let note = $derived(notes.find((n) => n.id === activeId) ?? notes[0]);
  let layout = $state(loadLayout());
  let showClips = $state(false);
  let clips = $state<Clip[]>(loadClips());
  let area = $state<HTMLTextAreaElement | undefined>();
  let box = $state<HTMLElement | undefined>();
  let saved = $state(true);

  const unsubscribe = onClips((c) => (clips = c));
  onDestroy(unsubscribe);

  // ---------------------------------------------------------- persistence
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  function persist() {
    saved = false;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      saveNotes({ notes, active: activeId });
      saved = true;
    }, 400);
  }
  onDestroy(() => {
    clearTimeout(saveTimer);
    if (!saved) saveNotes({ notes, active: activeId });
  });
  $effect(() => saveLayout(layout));

  function setText(text: string) {
    note.text = text.slice(0, MAX_NOTE_CHARS);
    note.updated = Date.now();
    persist();
    if (note.synced) queueSync(note.id);
  }

  // ---------------------------------------------------------- memory sync
  type SyncState = { state: 'syncing' | 'ok' | 'error'; at?: number; msg?: string };
  let sync = $state<Record<string, SyncState>>({});
  const syncTimers = new Map<string, ReturnType<typeof setTimeout>>();
  /** Delay after the last keystroke before re-indexing. */
  const SYNC_DELAY_MS = 4000;

  function queueSync(id: string) {
    clearTimeout(syncTimers.get(id));
    syncTimers.set(id, setTimeout(() => pushSync(id), SYNC_DELAY_MS));
  }
  onDestroy(() => syncTimers.forEach((t) => clearTimeout(t)));

  async function pushSync(id: string): Promise<boolean> {
    syncTimers.delete(id);
    const n = notes.find((x) => x.id === id);
    if (!n || !n.text.trim()) return false;
    sync[id] = { state: 'syncing' };
    try {
      await call('sync_note', { id, content: n.text });
      sync[id] = { state: 'ok', at: Date.now() };
      return true;
    } catch (e) {
      sync[id] = { state: 'error', msg: errorMessage(e) };
      return false;
    }
  }

  async function toggleSync() {
    const n = note;
    if (!n.synced) {
      if (!n.text.trim()) {
        notify('Write something first, then keep it in memory', 'info');
        return;
      }
      if (await pushSync(n.id)) {
        n.synced = true;
        persist();
        notify('Kept in memory: OMNIX can now recall this note (📚 notes knowledge base)', 'success');
      } else {
        notify(`Keep in memory: ${sync[n.id]?.msg ?? 'failed'}`, 'error');
      }
    } else {
      clearTimeout(syncTimers.get(n.id));
      try {
        await call('unsync_note', { id: n.id });
        n.synced = false;
        delete sync[n.id];
        persist();
        notify('Removed from memory; the note stays here', 'info');
      } catch (e) {
        notify(`Remove from memory: ${errorMessage(e)}`, 'error');
      }
    }
  }

  let syncInfo = $derived.by(() => {
    if (!note.synced) return '';
    const s = sync[note.id];
    if (!s) return '🔗 In memory';
    if (s.state === 'syncing') return '🔗 Syncing…';
    if (s.state === 'error') return `🔗 Sync failed: ${s.msg}`;
    return `🔗 Synced ${ago(s.at)}`;
  });

  /** Append text to the open note (from a reply, task or pin). */
  export function append(text: string, source?: string) {
    setText(appendText(note.text, text, source));
    layout.open = true;
    showClips = false;
    notify(`Added to “${noteTitle(note.text)}”`, 'success');
  }

  /** Open a note by id (from the command palette). */
  export function open(id: string) {
    if (!notes.some((n) => n.id === id)) return;
    activeId = id;
    layout.open = true;
    showClips = false;
    persist();
  }

  /** Start a new note and focus it. */
  export function create() {
    layout.open = true;
    addNote();
  }

  function addNote() {
    if (notes.length >= MAX_NOTES) {
      notify(`Up to ${MAX_NOTES} notes: delete one first`, 'info');
      return;
    }
    const n = newNote();
    notes.unshift(n);
    activeId = n.id;
    showClips = false;
    layout.mode = 'edit';
    persist();
    queueMicrotask(() => area?.focus());
  }

  function deleteNote() {
    if (note.text.trim() && !confirmDelete) {
      confirmDelete = true;
      setTimeout(() => (confirmDelete = false), 3000);
      return;
    }
    confirmDelete = false;
    if (note.synced) {
      const id = note.id;
      clearTimeout(syncTimers.get(id));
      call('unsync_note', { id }).catch((e) => notify(`Remove from memory: ${errorMessage(e)}`, 'error'));
    }
    notes = notes.filter((n) => n.id !== note.id);
    if (!notes.length) notes.push(newNote());
    activeId = notes[0].id;
    persist();
  }
  let confirmDelete = $state(false);

  // ---------------------------------------------------------- editing
  function restore(e: Edit) {
    setText(e.text);
    queueMicrotask(() => {
      if (!area) return;
      area.focus();
      area.setSelectionRange(e.start, e.end);
    });
  }

  function format(kind: Parameters<typeof applyFormat>[3]) {
    if (!area) return;
    restore(applyFormat(note.text, area.selectionStart, area.selectionEnd, kind));
  }

  function keydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && (e.key === 'b' || e.key === 'i')) {
      e.preventDefault();
      format(e.key === 'b' ? 'bold' : 'italic');
    } else if (mod && e.key === 'Enter' && area) {
      // Ctrl+Enter ticks the task on the current line.
      e.preventDefault();
      const pos = area.selectionStart;
      const line = note.text.slice(0, pos).split('\n').length - 1;
      restore({ text: toggleTask(note.text, line), start: pos, end: pos });
    } else if (e.key === 'Tab' && !e.shiftKey && area) {
      e.preventDefault();
      const { selectionStart: s, selectionEnd: en } = area;
      restore({ text: note.text.slice(0, s) + '  ' + note.text.slice(en), start: s + 2, end: s + 2 });
    }
  }

  // ---------------------------------------------------------- resize
  let drag: { y: number; h: number } | null = null;
  function available() {
    const dock = box?.closest('aside');
    return dock ? dock.clientHeight - 150 : 600;
  }
  function gripDown(e: PointerEvent) {
    drag = { y: e.clientY, h: layout.height };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function gripMove(e: PointerEvent) {
    if (!drag) return;
    layout.height = clampHeight(drag.h + (drag.y - e.clientY), available());
  }
  function gripUp() {
    drag = null;
  }
  function gripKey(e: KeyboardEvent) {
    if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      e.preventDefault();
      layout.height = clampHeight(layout.height + (e.key === 'ArrowUp' ? 24 : -24), available());
    }
  }

  // ---------------------------------------------------------- actions
  async function copy(text: string) {
    if (await copyText(text)) notify('Copied', 'success');
    else notify('Copy is not allowed here: it is in 📋 Clips instead', 'info');
  }

  async function paste() {
    try {
      const text = await navigator.clipboard.readText();
      if (text) append(text);
    } catch {
      notify('Reading the clipboard is not allowed here: press Ctrl+V in the note', 'info');
    }
  }

  function tidy() {
    const text = note.text.trim();
    if (!text) return;
    onTask(
      'Tidy these notes into clean, well-structured Markdown with headings, bullets and task lists where they fit. Keep every fact; do not add new ones.',
      { title: `🧹 Tidy “${noteTitle(text, 24)}”`, context: text }
    );
  }

  function useClip(c: Clip) {
    if (area && !showClips) {
      const { selectionStart: s, selectionEnd: en } = area;
      restore({ text: note.text.slice(0, s) + c.text + note.text.slice(en), start: s + c.text.length, end: s + c.text.length });
    } else {
      append(c.text);
    }
  }

  let stats = $derived(counts(note.text));
  let preview = $derived(layout.mode === 'edit' ? '' : renderMarkdown(taskGlyphs(note.text)));
</script>

<section
  bind:this={box}
  class="notepad"
  class:open={layout.open}
  style={layout.open ? `height: ${layout.height}px` : ''}
  aria-label="Notepad"
>
  {#if layout.open}
    <button
      type="button"
      class="grip"
      aria-label="Resize the notepad (↑/↓)"
      onpointerdown={gripDown}
      onpointermove={gripMove}
      onpointerup={gripUp}
      onpointercancel={gripUp}
      onkeydown={gripKey}
      ondblclick={() => (layout.height = 260)}
      title="Drag to resize · double-click to reset"
    ></button>
  {/if}

  <header class="flex items-center gap-1.5 px-2.5 py-1.5">
    <button class="title-btn" onclick={() => (layout.open = !layout.open)} aria-expanded={layout.open} title={layout.open ? 'Collapse' : 'Expand'}>
      <span class="caret" class:down={layout.open}>▸</span> 📝 <span class="font-semibold">Notepad</span>
    </button>
    {#if layout.open}
      {#if !showClips}
        <select class="picker" bind:value={activeId} onchange={persist} aria-label="Open note">
          {#each notes as n (n.id)}<option value={n.id}>{noteTitle(n.text, 28)}</option>{/each}
        </select>
        <button class="ic" onclick={addNote} title="New note">＋</button>
        <button class="ic {confirmDelete ? 'danger' : ''}" onclick={deleteNote} title={confirmDelete ? 'Click again to delete' : 'Delete this note'}>{confirmDelete ? 'Delete?' : '🗑'}</button>
      {:else}
        <span class="flex-1"></span>
      {/if}
      <button class="ic {showClips ? 'on' : ''}" onclick={() => (showClips = !showClips)} title="Clips: what you copied in OMNIX">📋{#if clips.length}<sup>{clips.length}</sup>{/if}</button>
    {:else}
      <span class="flex-1 truncate faint">{noteTitle(note.text)}</span>
    {/if}
  </header>

  {#if layout.open}
    {#if showClips}
      <div class="flex-1 min-h-0 overflow-auto px-2.5 pb-2 space-y-1.5">
        {#each clips as c (c.id)}
          <div class="clip">
            <button class="flex-1 min-w-0 text-left" onclick={() => useClip(c)} title="Add to the note">
              <span class="block truncate">{c.text}</span>
              <span class="faint tiny">{ago(c.at)} · {c.text.length} chars</span>
            </button>
            <button class="ic" onclick={() => copy(c.text)} title="Copy again">⧉</button>
            <button class="ic" onclick={() => onInsert(c.text)} title="To chat">💬</button>
            <button class="ic" onclick={() => saveClips(clips.filter((x) => x.id !== c.id))} title="Remove">✕</button>
          </div>
        {:else}
          <p class="faint tiny text-center py-4">Anything you copy with OMNIX's 📋 buttons lands here, newest first.</p>
        {/each}
        <div class="flex justify-between">
          <button class="faint tiny hover:text-white" onclick={paste} title="Add what is on the system clipboard to the note">📥 Paste from clipboard</button>
          {#if clips.length}<button class="faint tiny hover:text-white" onclick={() => saveClips([])}>Clear clips</button>{/if}
        </div>
      </div>
    {:else}
      <div class="flex items-center gap-px px-2 pb-1 flex-wrap">
        <div class="seg" role="group" aria-label="View">
          <button class:on={layout.mode === 'edit'} onclick={() => (layout.mode = 'edit')} title="Write">✎</button>
          <button class:on={layout.mode === 'split'} onclick={() => (layout.mode = 'split')} title="Write + preview">◫</button>
          <button class:on={layout.mode === 'preview'} onclick={() => (layout.mode = 'preview')} title="Preview Markdown">👁</button>
        </div>
        {#if layout.mode !== 'preview'}
          <span class="sep"></span>
          <button class="fmt font-bold" onclick={() => format('bold')} title="Bold (Ctrl+B)">B</button>
          <button class="fmt italic" onclick={() => format('italic')} title="Italic (Ctrl+I)">I</button>
          <button class="fmt" onclick={() => format('heading')} title="Heading">H</button>
          <button class="fmt" onclick={() => format('bullet')} title="Bullet list">•</button>
          <button class="fmt" onclick={() => format('task')} title="Task (Ctrl+Enter ticks it)">☐</button>
          <button class="fmt mono" onclick={() => format('code')} title="Code">{'</>'}</button>
        {/if}
        <span class="flex-1"></span>
        <button class="fmt" disabled={!note.text.trim()} onclick={() => copy(note.text)} title="Copy the note">⧉</button>
        <button class="fmt" disabled={!note.text.trim()} onclick={() => onInsert(note.text)} title="Put the note in the chat box">💬</button>
        <button class="fmt" disabled={!note.text.trim()} onclick={tidy} title="Tidy into clean Markdown (side task)">🧹</button>
        <button class="fmt" disabled={!note.text.trim()} onclick={() => onPin(note.text, `Note · ${noteTitle(note.text)}`)} title="Pin">📌</button>
        <button
          class="fmt {note.synced ? 'on' : ''}"
          disabled={!note.synced && !note.text.trim()}
          onclick={toggleSync}
          aria-pressed={!!note.synced}
          title={note.synced
            ? 'Kept in memory: re-indexed as you edit. Click to remove it from memory'
            : 'Keep in memory: index this note in kb-core (notes knowledge base) so chat can recall it, and keep it synced as you edit'}
        >🔗</button>
      </div>

      <div class="body flex-1 min-h-0 flex {layout.mode === 'split' ? 'flex-col' : ''}">
        {#if layout.mode !== 'preview'}
          <textarea
            bind:this={area}
            value={note.text}
            oninput={(e) => setText(e.currentTarget.value)}
            onkeydown={keydown}
            spellcheck="true"
            placeholder={'Jot anything. Markdown works: # heading, **bold**, - [ ] task, `code`, tables…'}
            class="editor flex-1 min-h-0"
            aria-label="Note text"
          ></textarea>
        {/if}
        {#if layout.mode !== 'edit'}
          <!-- Sanitized: notes often hold pasted model output. -->
          <div class="md-rich preview flex-1 min-h-0 overflow-auto">
            {#if note.text.trim()}{@html preview}{:else}<p class="faint">Nothing to preview yet.</p>{/if}
          </div>
        {/if}
      </div>
      <footer class="flex justify-between px-2.5 py-1 faint tiny tabular-nums">
        <span>{stats.words} words · {stats.chars.toLocaleString()} chars</span>
        <span class="truncate ml-2 {sync[note.id]?.state === 'error' ? 'text-red-300' : ''}" title={syncInfo}>{syncInfo ? `${syncInfo} · ` : ''}{saved ? `Saved ${ago(note.updated)}` : 'Saving…'}</span>
      </footer>
    {/if}
  {/if}
</section>

<style>
  .notepad {
    position: relative;
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    min-height: 0;
    border-top: 1px solid var(--tile-border, rgba(255, 255, 255, 0.1));
    background: linear-gradient(to bottom, rgba(255, 255, 255, 0.035), rgba(255, 255, 255, 0.015));
  }
  .grip {
    position: absolute;
    top: -5px;
    left: 0;
    right: 0;
    height: 10px;
    cursor: ns-resize;
    touch-action: none;
    z-index: 2;
  }
  .grip::after {
    content: '';
    position: absolute;
    left: 50%;
    top: 3px;
    width: 42px;
    height: 4px;
    margin-left: -21px;
    border-radius: 9999px;
    background: rgba(255, 255, 255, 0.22);
    transition: background-color 0.15s ease, width 0.15s ease;
  }
  .grip:hover::after,
  .grip:focus-visible::after {
    background: var(--accent, #22d3ee);
    width: 64px;
    margin-left: -32px;
  }
  .grip:focus-visible { outline: none; }
  .title-btn { display: flex; align-items: center; gap: 0.35rem; color: var(--text, #eef1f6); font-size: 0.93em; white-space: nowrap; }
  .caret { display: inline-block; transition: transform 0.15s ease; color: var(--text-faint, #9aa3b4); }
  .caret.down { transform: rotate(90deg); }
  .picker {
    flex: 1;
    min-width: 0;
    font-size: 0.86em;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 0.15rem 0.35rem;
    color: var(--text, #eef1f6);
  }
  .picker option { background: #141824; }
  .ic {
    padding: 0.1rem 0.4rem;
    border-radius: 7px;
    font-size: 0.86em;
    color: var(--text-dim, #c3cad6);
  }
  .ic:hover { background: rgba(255, 255, 255, 0.1); color: white; }
  .ic.on { background: var(--accent-soft, rgba(34, 211, 238, 0.18)); color: var(--accent, #22d3ee); }
  .ic sup { font-size: 0.7em; margin-left: 1px; }
  .seg { display: inline-flex; border-radius: 8px; background: rgba(255, 255, 255, 0.05); padding: 1px; }
  .seg button { padding: 0.05rem 0.45rem; border-radius: 7px; font-size: 0.86em; color: var(--text-dim, #c3cad6); }
  .seg button.on { background: var(--accent-soft, rgba(34, 211, 238, 0.18)); color: var(--accent, #22d3ee); }
  .sep { width: 1px; height: 1rem; background: rgba(255, 255, 255, 0.12); margin: 0 0.15rem; }
  .fmt {
    min-width: 1.35rem;
    padding: 0.05rem 0.2rem;
    border-radius: 6px;
    font-size: 0.86em;
    color: var(--text-dim, #c3cad6);
  }
  .fmt:hover:not(:disabled) { background: rgba(255, 255, 255, 0.1); color: white; }
  .fmt:disabled { opacity: 0.35; }
  .fmt.on { background: var(--accent-soft, rgba(34, 211, 238, 0.18)); color: var(--accent, #22d3ee); }
  .ic.danger { color: #fca5a5; background: rgba(248, 113, 113, 0.15); }
  .mono { font-family: ui-monospace, monospace; font-size: 0.78em; }
  .body { padding: 0 0.6rem; gap: 0.4rem; }
  .editor {
    width: 100%;
    resize: none;
    font-family: ui-monospace, 'JetBrains Mono', monospace;
    font-size: 0.9em;
    line-height: 1.55;
    color: var(--text, #eef1f6);
    background: rgba(0, 0, 0, 0.22);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    padding: 0.5rem 0.6rem;
    caret-color: var(--accent, #22d3ee);
  }
  .editor:focus { outline: none; border-color: var(--accent-line, rgba(34, 211, 238, 0.55)); }
  .editor::placeholder { color: var(--text-faint, #9aa3b4); opacity: 0.8; }
  .preview {
    font-size: 0.93em;
    line-height: 1.55;
    color: var(--text, #eef1f6);
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 10px;
    padding: 0.5rem 0.7rem;
  }
  .clip {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.35rem 0.5rem;
    border-radius: 10px;
    font-size: 0.86em;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.07);
  }
  .clip:hover { border-color: var(--accent-line, rgba(34, 211, 238, 0.45)); }
  .faint { color: var(--text-faint, #9aa3b4); }
  .tiny { font-size: 0.78em; }
  @media (prefers-reduced-motion: reduce) {
    .caret, .grip::after { transition: none; }
  }
</style>
