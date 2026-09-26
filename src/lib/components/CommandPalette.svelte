<script lang="ts">
  /**
   * Command palette (Ctrl+K). Items come from the page; typing three or more
   * characters also searches long-term memory (kb-core) by meaning.
   *
   * Memory text is untrusted data: it is only ever shown as plain text and
   * handed on as text (to the chat box or the notepad), never executed.
   */
  import { tick } from 'svelte';
  import { errorMessage } from '$lib/api';
  import { preview, rank, type MemoryHit, type PaletteItem } from '$lib/palette';

  let {
    open = $bindable(false),
    items,
    searchMemory,
    onMemory,
    onAsk,
    onTask
  }: {
    open?: boolean;
    items: PaletteItem[];
    searchMemory?: (query: string) => Promise<MemoryHit[]>;
    /** Use a memory: put it in the chat box, or append it to the note. */
    onMemory: (hit: MemoryHit, target: 'chat' | 'note') => void;
    onAsk: (text: string) => void;
    onTask: (text: string) => void;
  } = $props();

  let query = $state('');
  let active = $state(0);
  let input = $state<HTMLInputElement | undefined>();
  let list = $state<HTMLElement | undefined>();
  let hits = $state<MemoryHit[]>([]);
  let searching = $state(false);
  let memoryError = $state('');

  $effect(() => {
    if (!open) return;
    query = '';
    active = 0;
    hits = [];
    memoryError = '';
    tick().then(() => input?.focus());
  });

  // Memory search, debounced; stale answers are dropped.
  let seq = 0;
  $effect(() => {
    const q = query.trim();
    if (!open || !searchMemory || q.length < 3 || q.startsWith('/')) {
      hits = [];
      searching = false;
      return;
    }
    const my = ++seq;
    searching = true;
    const t = setTimeout(async () => {
      try {
        const r = await searchMemory(q);
        if (my === seq) {
          hits = r.filter((h) => h.score >= 0.35).slice(0, 6);
          memoryError = '';
        }
      } catch (e) {
        if (my === seq) memoryError = errorMessage(e);
      } finally {
        if (my === seq) searching = false;
      }
    }, 250);
    return () => clearTimeout(t);
  });

  let memoryItems = $derived<PaletteItem[]>(
    hits.map((h) => ({
      id: `mem-${h.id}`,
      group: 'Memory',
      icon: h.kind === 'document' ? '📄' : '🧠',
      label: preview(h.content),
      hint: `${Math.round(h.score * 100)}% · ${h.kind === 'document' ? (h.source ?? 'document') : 'memory'}${h.collection && h.collection !== 'default' ? ` · ${h.collection}` : ''}`,
      run: () => onMemory(h, 'chat'),
      alt: { label: 'to note', run: () => onMemory(h, 'note') }
    }))
  );

  let askItems = $derived<PaletteItem[]>(
    query.trim()
      ? [
          { id: 'ask', group: 'Ask', icon: '💬', label: `Ask OMNIX: ${query.trim()}`, run: () => onAsk(query.trim()) },
          { id: 'task', group: 'Ask', icon: '⚡', label: `Side task: ${query.trim()}`, hint: 'runs beside the chat', run: () => onTask(query.trim()) }
        ]
      : []
  );

  // Ranked static items, then memory hits (already ranked by kb-core), then
  // "ask". Rows are clustered by group, each group placed where its best
  // match ranked, so headers never repeat.
  let results = $derived.by(() => {
    const byGroup = new Map<string, PaletteItem[]>();
    for (const item of [...rank(items, query, 30), ...memoryItems, ...askItems]) {
      const rows = byGroup.get(item.group);
      if (rows) rows.push(item);
      else byGroup.set(item.group, [item]);
    }
    return [...byGroup.values()].flat();
  });
  let groups = $derived.by(() => {
    const out: { group: string; rows: { item: PaletteItem; index: number }[] }[] = [];
    results.forEach((item, index) => {
      const last = out.at(-1);
      if (last?.group === item.group) last.rows.push({ item, index });
      else out.push({ group: item.group, rows: [{ item, index }] });
    });
    return out;
  });

  $effect(() => {
    if (active >= results.length) active = Math.max(0, results.length - 1);
  });

  function choose(item: PaletteItem | undefined, alt = false) {
    if (!item) return;
    open = false;
    (alt && item.alt ? item.alt.run : item.run)();
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const n = results.length;
      if (!n) return;
      active = (active + (e.key === 'ArrowDown' ? 1 : n - 1)) % n;
      tick().then(() => list?.querySelector('[aria-selected="true"]')?.scrollIntoView?.({ block: 'nearest' }));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      choose(results[active], e.shiftKey);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      open = false;
    }
  }
</script>

{#if open}
  <div class="backdrop" role="presentation" onclick={() => (open = false)}></div>
  <div class="palette" role="dialog" aria-modal="true" aria-label="Command palette">
    <div class="flex items-center gap-2 px-4 py-3 border-b border-white/10">
      <span class="text-lg" aria-hidden="true">🔎</span>
      <input
        bind:this={input}
        bind:value={query}
        onkeydown={keydown}
        oninput={() => (active = 0)}
        placeholder="Go to, run, open a note, or search memory…"
        class="flex-1 bg-transparent text-base text-white placeholder-gray-400 focus:outline-none"
        role="combobox"
        aria-expanded="true"
        aria-controls="palette-list"
        aria-activedescendant={results[active] ? `pal-${active}` : undefined}
        aria-label="Search commands and memory"
      />
      {#if searching}<span class="text-xs text-gray-400 animate-pulse">memory…</span>{/if}
      <kbd>Esc</kbd>
    </div>
    <div id="palette-list" class="max-h-[60vh] overflow-auto py-1" role="listbox" bind:this={list}>
      {#each groups as g (g.group)}
        <div class="group-label">{g.group}</div>
        {#each g.rows as { item, index } (item.id)}
          <!-- Keys are handled by the combobox input (aria-activedescendant). -->
          <div
            id="pal-{index}"
            role="option"
            tabindex="-1"
            aria-selected={index === active}
            class="row {index === active ? 'on' : ''}"
            onclick={(e) => choose(item, e.shiftKey)}
            onkeydown={() => {}}
            onmousemove={() => (active = index)}
          >
            <span class="icon" aria-hidden="true">{item.icon}</span>
            <span class="flex-1 min-w-0 truncate">{item.label}</span>
            {#if item.hint}<span class="hint">{item.hint}</span>{/if}
            {#if index === active && item.alt}<span class="hint">⇧↵ {item.alt.label}</span>{/if}
          </div>
        {/each}
      {:else}
        <p class="px-4 py-6 text-sm text-gray-400 text-center">Nothing matches.</p>
      {/each}
      {#if memoryError}<p class="px-4 py-2 text-xs text-amber-300">Memory search: {memoryError}</p>{/if}
    </div>
    <div class="flex gap-4 px-4 py-2 border-t border-white/10 text-[11px] text-gray-400">
      <span><kbd>↑↓</kbd> move</span><span><kbd>↵</kbd> run</span><span><kbd>⇧↵</kbd> alternate</span>
      <span class="ml-auto">3+ letters also search memory</span>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: rgba(3, 5, 12, 0.55);
    -webkit-backdrop-filter: blur(3px);
    backdrop-filter: blur(3px);
  }
  .palette {
    position: fixed;
    z-index: 61;
    top: 12vh;
    left: 50%;
    width: min(640px, calc(100vw - 2rem));
    transform: translateX(-50%);
    border-radius: 18px;
    background: rgba(16, 20, 32, 0.96);
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 30px 90px -20px rgba(0, 0, 0, 0.8), 0 0 0 1px rgba(34, 211, 238, 0.08);
    animation: pop 0.14s ease-out;
    color: #eef1f6;
  }
  @keyframes pop {
    from { opacity: 0; transform: translateX(-50%) translateY(-6px) scale(0.98); }
    to { opacity: 1; transform: translateX(-50%); }
  }
  .group-label {
    padding: 0.5rem 1rem 0.2rem;
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #9aa3b4;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin: 0 0.4rem;
    padding: 0.45rem 0.6rem;
    border-radius: 10px;
    font-size: 0.9rem;
    cursor: pointer;
  }
  .row.on { background: rgba(34, 211, 238, 0.14); box-shadow: inset 2px 0 0 #22d3ee; }
  .icon { width: 1.4rem; text-align: center; }
  .hint { font-size: 0.72rem; color: #9aa3b4; white-space: nowrap; }
  kbd {
    font-family: ui-monospace, monospace;
    font-size: 0.68rem;
    padding: 0.05rem 0.35rem;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #c3cad6;
  }
  @media (prefers-reduced-motion: reduce) {
    .palette { animation: none; }
  }
</style>
