<script lang="ts">
  import { onMount } from 'svelte';
  import { call, errorMessage, unavailable } from '$lib/api';
  import type { KnowledgeData } from '$lib/types';

  interface Memory {
    id: string;
    content: string;
    timestamp: Date;
    tags: string[];
    importance: number;
    category: string;
    source?: string;
    pinned?: boolean;
    reinforced?: number;
    accessCount?: number;
    rejected?: number;
    activation?: number | null;
    collection?: string | null;
  }
  interface Hit {
    id: string;
    content: string;
    score: number;
    similarity: number | null;
    timestamp: Date;
    tags: string[];
    kind: string;
    source: string;
  }
  interface HiddenMemory {
    id: string;
    content: string;
    created_at?: string | null;
    superseded_by: string;
    hidden_reason?: 'merged' | 'superseded' | null;
    reviewed?: boolean;
    judged_by?: 'llm' | 'nli' | 'llm+nli' | 'similarity' | null;
  }
  interface KnowledgeBase {
    name: string;
    description: string;
    memories: number;
    documents: number;
    chunks: number;
    updated_at: string;
  }
  interface Doc {
    id: string;
    name: string;
    type: string;
    size: number;
    indexed: boolean;
    chunks: number;
    sourcePath?: string | null;
    timestamp: Date;
  }

  let activeTab = $state('memories');
  let searchQuery = $state('');
  let isSearching = $state(false);
  let isProcessing = $state(false);
  let available = $state(false);
  let extended = $state(false);
  let lastError = $state('');
  let info = $state('');
  let searched = $state(false);

  let memories = $state<Memory[]>([]);
  let searchResults = $state<Hit[]>([]);
  let documents = $state<Doc[]>([]);
  let data = $state<KnowledgeData | null>(null);

  let hidden = $state<HiddenMemory[] | null>(null);
  let showHidden = $state(false);

  let newMemory = $state({ content: '', tags: '', importance: 5, category: 'general' });
  let targetKb = $state('');
  let searchKb = $state('');
  let newKb = $state({ name: '', description: '' });
  let kbs = $derived(((data?.knowledgeBases ?? []) as KnowledgeBase[]));

  const tabs = [
    { id: 'memories', label: 'Memories', icon: '💭' },
    { id: 'documents', label: 'Documents', icon: '📄' },
    { id: 'knowledge-bases', label: 'Knowledge Bases', icon: '📚' },
    { id: 'search', label: 'Search', icon: '🔍' },
    { id: 'analytics', label: 'Analytics', icon: '📊' }
  ];

  const categories = [
    'general', 'personal', 'preference', 'work', 'project', 'technical',
    'research', 'meeting', 'idea', 'task', 'reference'
  ];

  const sourceLabel: Record<string, string> = {
    user: '✍️ you',
    assistant: '🤖 assistant',
    extract: '🧠 learned',
    import: '📥 imported',
    mcp: '🔌 via MCP'
  };
  /** Who hid a memory (kb-core `judged_by`). */
  const judgeLabel: Record<string, string> = {
    llm: 'by the local model',
    nli: 'by the NLI model',
    'llm+nli': 'both judges agreed',
    similarity: 'near-identical text'
  };

  onMount(loadData);

  async function loadData() {
    try {
      const parsed = await call<KnowledgeData>('get_knowledge_data');
      data = parsed;
      available = parsed.available;
      extended = parsed.extended;
      memories = (parsed.memories as any[]).map((m) => ({ ...m, timestamp: new Date(m.timestamp) }));
      documents = (parsed.documents as any[]).map((d) => ({ ...d, timestamp: new Date(d.timestamp) }));
    } catch (error) {
      lastError = errorMessage(error);
    }
  }

  function flash(message: string) {
    lastError = '';
    info = message;
  }

  async function run(fn: () => Promise<void>) {
    isProcessing = true;
    try {
      await fn();
    } catch (error) {
      info = '';
      lastError = errorMessage(error);
    } finally {
      isProcessing = false;
    }
  }

  function saveMemory() {
    if (!newMemory.content.trim()) return;
    return run(async () => {
      const tags = newMemory.tags.split(',').map((t) => t.trim()).filter(Boolean);
      const r = await call<{ status?: string; related?: Array<{ content: string }> }>('save_memory', {
        content: newMemory.content,
        tags,
        importance: newMemory.importance,
        category: newMemory.category,
        collection: targetKb || null
      });
      if (r.status === 'reinforced') flash('Already known: the existing memory was reinforced instead of duplicated.');
      else if (r.status === 'updated') flash('Merged into an existing memory (your wording was more detailed).');
      else if (r.related?.length)
        flash(`Saved. Linked to ${r.related.length} related ${r.related.length === 1 ? 'memory' : 'memories'}; outdated ones are superseded automatically when a local model is configured.`);
      else flash('Saved.');
      newMemory = { content: '', tags: '', importance: 5, category: 'general' };
      await loadData();
    });
  }

  function deleteMemory(id: string) {
    if (!confirm('Delete this memory?')) return;
    return run(async () => {
      await call('delete_memory', { id });
      await loadData();
    });
  }

  function togglePin(m: Memory) {
    return run(async () => {
      await call('update_memory', { id: m.id, patch: { pinned: !m.pinned } });
      await loadData();
    });
  }

  async function loadHidden() {
    const rows = await call<HiddenMemory[]>('list_hidden_memories', { limit: 200 });
    // Unchecked rulings first; newest first within each group (server order).
    hidden = [...rows.filter((m) => !m.reviewed), ...rows.filter((m) => m.reviewed)];
  }

  function toggleHidden() {
    showHidden = !showHidden;
    if (showHidden) return run(loadHidden);
  }

  function openReview() {
    activeTab = 'memories';
    showHidden = true;
    return run(loadHidden);
  }

  // Agree with the model's ruling: the memory stays hidden and stops counting
  // as "needs review". Restore remains available afterwards.
  function keepRuling(m: HiddenMemory) {
    return run(async () => {
      await call('update_memory', { id: m.id, patch: { reviewed: true } });
      await Promise.all([loadData(), loadHidden()]);
    });
  }

  // Undo a consolidation ruling: the memory returns exactly as it was.
  function restoreMemory(m: HiddenMemory) {
    return run(async () => {
      await call('update_memory', { id: m.id, patch: { superseded_by: null } });
      flash('Restored. The memory is back in recall.');
      await Promise.all([loadData(), loadHidden()]);
    });
  }

  function deleteDocument(d: Doc) {
    if (!confirm(`Remove ${d.name} from the index?${d.sourcePath ? ' (It is in a watched folder and will come back on the next sync unless you delete the file.)' : ''}`)) return;
    return run(async () => {
      await call('delete_document', { id: d.id });
      await loadData();
    });
  }

  async function semanticSearch() {
    if (!searchQuery.trim()) return;
    isSearching = true;
    try {
      const results = await call<any[]>('semantic_search', { query: searchQuery, limit: 20, collection: searchKb || null });
      searchResults = results.map((r) => ({ ...r, timestamp: new Date(r.timestamp) }));
    } catch (error) {
      lastError = errorMessage(error);
      searchResults = [];
    } finally {
      isSearching = false;
      searched = true;
    }
  }

  async function indexDocument() {
    try {
      const path = await call<string | null>('select_file');
      if (!path) return;
      await run(async () => {
        const chunks = await call<number>('index_document', { path, collection: targetKb || null });
        flash(`Indexed ${path} (${chunks} chunks).`);
        await loadData();
      });
    } catch (error) {
      lastError = errorMessage(error);
    }
  }

  function syncFolders() {
    return run(async () => {
      const r = await call<{ folders: Record<string, { indexed: number; removed: number; unchanged: number }> }>('sync_knowledge_folders');
      const t = Object.values(r.folders ?? {}).reduce(
        (a, f) => ({ indexed: a.indexed + f.indexed, removed: a.removed + f.removed, unchanged: a.unchanged + f.unchanged }),
        { indexed: 0, removed: 0, unchanged: 0 }
      );
      flash(`Folders synced: ${t.indexed} updated, ${t.removed} removed, ${t.unchanged} unchanged.`);
      await loadData();
    });
  }

  function createKb() {
    if (!newKb.name.trim()) return;
    return run(async () => {
      await call('create_knowledge_base', { name: newKb.name, description: newKb.description });
      flash(`Knowledge base “${newKb.name}” created.`);
      newKb = { name: '', description: '' };
      await loadData();
    });
  }

  function deleteKb(name: string) {
    // The backend asks for confirmation in a native dialog.
    return run(async () => {
      const r = await call<{ items: number; documents: number }>('delete_knowledge_base', { name });
      flash(`Deleted “${name}” (${r.items} items, ${r.documents} documents).`);
      if (targetKb === name) targetKb = '';
      if (searchKb === name) searchKb = '';
      await loadData();
    });
  }

  function exportKnowledge() {
    return run(async () => {
      const path = await call<string | null>('export_knowledge');
      if (path) flash(`Exported to ${path} (readable only by you).`);
    });
  }

  function importKnowledge() {
    return run(async () => {
      const r = await call<Record<string, number> | null>('import_knowledge');
      if (r) flash(`Imported: ${r.memories_created ?? 0} new memories, ${r.memories_merged ?? 0} merged, ${r.documents ?? 0} documents, ${r.skipped ?? 0} skipped.`);
      await loadData();
    });
  }

  function optimizeVectorDB() {
    return run(async () => {
      const r = await call<{ merged_duplicates: number; embedded_pending: number; seconds: number }>('optimize_vector_db');
      flash(`Optimized in ${r.seconds}s: merged ${r.merged_duplicates} duplicate memories, embedded ${r.embedded_pending} pending items.`);
      await loadData();
    });
  }

  function formatBytes(bytes: number): string {
    if (!bytes) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.min(sizes.length - 1, Math.floor(Math.log(bytes) / Math.log(k)));
    return Math.round((bytes / Math.pow(k, i)) * 10) / 10 + ' ' + sizes[i];
  }

  function ago(ts: string | Date | null | undefined): string {
    if (!ts) return 'never';
    const s = (Date.now() - new Date(ts).getTime()) / 1000;
    if (Number.isNaN(s)) return '';
    if (s < 60) return 'just now';
    if (s < 3600) return `${Math.floor(s / 60)}m ago`;
    if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
    return `${Math.floor(s / 86400)}d ago`;
  }

  const activityLabel: Record<string, string> = {
    memory_saved: '💾 Memory saved',
    memory_reinforced: '🔁 Memory reinforced',
    memory_merged: '🔗 Duplicate merged',
    memory_superseded: '⏭ Outdated memory superseded',
    memory_updated: '✏️ Memory edited',
    memory_deleted: '🗑 Memory deleted',
    document_indexed: '📄 Document indexed',
    document_deleted: '🗑 Document removed',
    extract: '🧠 Facts captured',
    import: '📥 Import',
    maintenance: '⚡ Optimized',
    reembed: '🔄 Re-embedding (model changed)'
  };

  function activityDetail(kind: string, d: Record<string, unknown>): string {
    if (kind === 'document_indexed') return `${d.name} (${d.chunks} chunks, ${d.reused ?? 0} reused)`;
    if (kind === 'document_deleted') return String(d.name ?? '');
    if (kind === 'extract') return `${d.saved} new of ${d.found} found`;
    if (kind === 'maintenance') return `${d.merged} merged, ${d.embedded} embedded`;
    if (kind === 'memory_reinforced') return `similarity ${d.similarity}`;
    return '';
  }

  function getImportanceColor(importance: number): string {
    if (importance >= 8) return 'text-red-400';
    if (importance >= 5) return 'text-yellow-400';
    return 'text-green-400';
  }

  let categoryRows = $derived(
    Object.entries(data?.categories ?? {}).sort((a, b) => b[1] - a[1])
  );
  let categoryMax = $derived(Math.max(1, ...categoryRows.map(([, n]) => n)));
  let watchFolders = $derived(data?.watch?.folders ?? []);
  let extUnavailable = $derived(!available || !extended);
</script>

<div class="h-full flex flex-col">
  <!-- Header -->
  <div class="flex items-center justify-between mb-6">
    <div>
      <h2 class="text-3xl font-bold glow-text">Knowledge Base</h2>
      <p class="text-gray-400 mt-1">Long-term memory and indexed notes{#if data?.embeddingModel} · {data.embeddingModel}{/if}</p>
    </div>
    <div class="flex gap-2">
      <button
        onclick={exportKnowledge}
        disabled={isProcessing || extUnavailable || unavailable('export_knowledge')}
        title="Save all memories and documents to a .jsonl file"
        class="disabled:opacity-40 disabled:cursor-not-allowed glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm"
      >
        📤 Export
      </button>
      <button
        onclick={importKnowledge}
        disabled={isProcessing || extUnavailable || unavailable('import_knowledge')}
        title="Merge an export file (duplicates are merged, not repeated)"
        class="disabled:opacity-40 disabled:cursor-not-allowed glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm"
      >
        📥 Import
      </button>
      <button
        onclick={optimizeVectorDB}
        disabled={isProcessing || extUnavailable || unavailable('optimize_vector_db')}
        title="Merge duplicate memories, embed pending items, compact the database"
        class="disabled:opacity-40 disabled:cursor-not-allowed glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm"
      >
        ⚡ Optimize
      </button>
    </div>
  </div>

  {#if !available}
    <div class="glass-panel p-4 mb-4 bg-yellow-500/10 border border-yellow-500/30 text-sm text-gray-300">
      <span class="text-xs bg-yellow-500/20 text-yellow-300 px-2 py-1 rounded mr-2">Not configured</span>
      Long-term memory is disabled. Run <code>./scripts/setup-memory.sh</code> to install the local kb-core service, then restart OMNIX (or set its URL in Settings → Memory).
    </div>
  {:else if !extended}
    <div class="glass-panel p-3 mb-4 bg-white/5 text-sm text-gray-300">
      Connected to a kb-core service without analytics/document extensions: saving, search and indexing work; export, import and optimize need the bundled kb-core.
    </div>
  {:else if data?.status === 'degraded'}
    <div class="glass-panel p-3 mb-4 bg-yellow-500/10 border border-yellow-500/30 text-sm" role="status">
      Embedding model unavailable, so search is keyword-only for now. New items are stored and embedded automatically once it's back.
      {#if data.embedError}<span class="block text-xs text-gray-400 mt-1">{data.embedError}</span>{/if}
    </div>
  {/if}
  {#if info}
    <div class="glass-panel p-3 mb-4 bg-green-500/20 text-sm" role="status">{info}</div>
  {/if}
  {#if lastError}
    <div class="glass-panel p-3 mb-4 bg-red-500/20 text-sm" role="alert">{lastError}</div>
  {/if}

  {#if extended && (data?.needsReview ?? 0) > 0}
    <button onclick={openReview} disabled={isProcessing}
      class="glass-panel w-full p-3 mb-4 bg-amber-500/15 hover:bg-amber-500/25 text-sm text-left">
      🗂 {data?.needsReview} memory {data?.needsReview === 1 ? 'change' : 'changes'} by the local model to review:
      merged duplicates and replaced facts are hidden, not deleted. Check them →
    </button>
  {/if}

  <!-- Stats Overview -->
  <div class="grid grid-cols-4 gap-4 mb-6">
    <div class="glass-panel p-4">
      <div class="text-2xl font-bold text-cosmic-cyan">{data?.totalMemories ?? 0}</div>
      <div class="text-sm text-gray-400">Memories</div>
    </div>
    <div class="glass-panel p-4">
      <div class="text-2xl font-bold text-cosmic-purple">{data?.totalDocuments ?? 0}</div>
      <div class="text-sm text-gray-400">Documents{#if data?.totalChunks} · {data.totalChunks} chunks{/if}</div>
    </div>
    <div class="glass-panel p-4">
      <div class="text-2xl font-bold text-green-400">{watchFolders.length}</div>
      <div class="text-sm text-gray-400">Watched folders</div>
    </div>
    <div class="glass-panel p-4">
      <div class="text-2xl font-bold text-yellow-400">{formatBytes(data?.storageUsed ?? 0)}</div>
      <div class="text-sm text-gray-400">Storage used</div>
    </div>
  </div>

  <div class="flex-1 flex gap-4 overflow-hidden">
    <!-- Tabs Sidebar -->
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

    <!-- Content Area -->
    <div class="flex-1 glass-panel p-6 overflow-auto">
      {#if activeTab === 'memories'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan">Memories</h3>

          <div class="glass-panel p-4 bg-white/5 space-y-3">
            <h4 class="font-bold text-sm">Add a memory</h4>
            <textarea
              bind:value={newMemory.content}
              placeholder="What should OMNIX remember? One fact per memory works best."
              rows="3"
              class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 text-sm"
            ></textarea>
            {#if kbs.length > 1}
              <div>
                <label for="kv-kb" class="block text-xs text-gray-400 mb-1">Knowledge base</label>
                <select id="kv-kb" bind:value={targetKb} class="w-full md:w-64 bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm">
                  <option value="">default</option>
                  {#each kbs.filter((k) => k.name !== 'default') as kb}<option value={kb.name}>{kb.name}</option>{/each}
                </select>
              </div>
            {/if}
            <div class="grid grid-cols-3 gap-3">
              <div>
                <label for="kv-field-1" class="block text-xs text-gray-400 mb-1">Tags (comma-separated)</label>
                <input id="kv-field-1" type="text" bind:value={newMemory.tags} placeholder="work, important"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
              </div>
              <div>
                <label for="kv-field-2" class="block text-xs text-gray-400 mb-1">Category</label>
                <select id="kv-field-2" bind:value={newMemory.category} class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm">
                  {#each categories as category}
                    <option value={category}>{category}</option>
                  {/each}
                </select>
              </div>
              <div>
                <label for="kv-field-3" class="block text-xs text-gray-400 mb-1">Importance (1-10)</label>
                <input id="kv-field-3" type="number" bind:value={newMemory.importance} min="1" max="10"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
              </div>
            </div>
            <button
              onclick={saveMemory}
              disabled={isProcessing || !newMemory.content.trim() || !available}
              title={!available ? 'Set up kb-core first (scripts/setup-memory.sh)' : undefined}
              class="w-full px-4 py-2 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg font-medium transition-all disabled:opacity-50"
            >
              {isProcessing ? '⏳ Saving...' : '💾 Save Memory'}
            </button>
          </div>

          <div class="space-y-3">
            {#each memories as memory (memory.id)}
              <div class="glass-panel p-4 bg-white/5 hover:bg-white/10 transition-all">
                <div class="flex items-start justify-between mb-2">
                  <div class="flex items-center gap-2 flex-wrap">
                    <span class="text-xs px-2 py-1 rounded bg-cosmic-blue/20 text-cosmic-cyan">{memory.category}</span>
                    <span class="text-xs {getImportanceColor(memory.importance)}" title="importance {memory.importance}/10">
                      {'★'.repeat(Math.min(memory.importance, 10))}
                    </span>
                    {#if memory.collection && memory.collection !== 'default'}
                      <span class="text-xs px-2 py-1 rounded bg-cosmic-purple/20 text-purple-200">📚 {memory.collection}</span>
                    {/if}
                    {#if memory.source}
                      <span class="text-xs px-2 py-1 rounded bg-white/5 text-gray-300">{sourceLabel[memory.source] ?? memory.source}</span>
                    {/if}
                    {#if memory.reinforced}
                      <span class="text-xs text-gray-400" title="stated again {memory.reinforced}×">🔁 {memory.reinforced}</span>
                    {/if}
                    {#if memory.accessCount}
                      <span class="text-xs text-gray-400" title="recalled {memory.accessCount}×">👁 {memory.accessCount}</span>
                    {/if}
                    {#if memory.rejected}
                      <span class="text-xs text-amber-300" title="You flagged it as wrong {memory.rejected}× when it was recalled: it ranks lower. Edit it, or delete it if it is wrong.">👎 {memory.rejected}</span>
                    {/if}
                  </div>
                  <div class="flex items-center gap-2">
                    {#if extended}
                      <button onclick={() => togglePin(memory)} disabled={isProcessing}
                        title={memory.pinned ? 'Unpin (lets it fade with time)' : 'Pin (never fades)'}
                        class="text-sm {memory.pinned ? '' : 'opacity-40 hover:opacity-100'}">📌</button>
                    {/if}
                    <button onclick={() => deleteMemory(memory.id)} disabled={!available || isProcessing}
                      class="text-red-400 hover:text-red-300 text-sm" title="Delete">🗑️</button>
                  </div>
                </div>
                <p class="text-sm mb-2 whitespace-pre-wrap">{memory.content}</p>
                <div class="flex items-center justify-between text-xs text-gray-400 gap-4">
                  <div class="flex gap-2 flex-wrap">
                    {#each memory.tags as tag}
                      <span class="px-2 py-1 rounded bg-white/5">#{tag}</span>
                    {/each}
                  </div>
                  <div class="flex items-center gap-3 shrink-0">
                    {#if memory.activation != null}
                      <span class="flex items-center gap-1" title="How present this memory is (importance, recency, use)">
                        <span class="w-16 h-1.5 bg-white/10 rounded-full overflow-hidden inline-block">
                          <span class="block h-full bg-cosmic-cyan" style="width: {Math.round(memory.activation * 100)}%"></span>
                        </span>
                      </span>
                    {/if}
                    <span>{memory.timestamp.toLocaleString()}</span>
                  </div>
                </div>
              </div>
            {/each}
            {#if memories.length === 0}
              <div class="text-center py-12 text-gray-400">
                <div class="text-4xl mb-2">💭</div>
                <p>No memories stored yet</p>
                <p class="text-xs mt-2">Add one here, ask the assistant to "remember …", or turn on Settings → Memory → Learn from conversations.</p>
              </div>
            {/if}
          </div>

          {#if extended}
            <div class="glass-panel p-4 bg-white/5 space-y-3">
              <button onclick={toggleHidden} disabled={isProcessing} class="w-full flex items-center justify-between text-sm font-bold">
                <span>🗂 Merged &amp; replaced memories{(data?.needsReview ?? 0) > 0 ? ` · ${data?.needsReview} to review` : ''}</span>
                <span class="text-gray-400">{showHidden ? '▾' : '▸'}</span>
              </button>
              {#if showHidden}
                <p class="text-xs text-gray-400">
                  When the local model decides two memories say the same thing, or that a newer fact replaces an
                  older one, the older memory is hidden from recall, never deleted. If it got that wrong, restore it;
                  if it was right, keep it hidden.
                </p>
                {#each hidden ?? [] as m (m.id)}
                  <div class="p-3 rounded-lg bg-white/5 flex items-start justify-between gap-3">
                    <div class="min-w-0">
                      <span class="text-xs px-2 py-1 rounded bg-white/5 text-gray-300">
                        {m.hidden_reason === 'merged' ? '🔀 merged into a duplicate' : '⏭ replaced by a newer fact'}
                      </span>
                      {#if m.judged_by}
                        <span class="text-xs text-gray-500 ml-1">{judgeLabel[m.judged_by] ?? m.judged_by}</span>
                      {/if}
                      <p class="text-sm mt-2 whitespace-pre-wrap text-gray-300">{m.content}</p>
                      {#if m.created_at}<p class="text-xs text-gray-500 mt-1">{new Date(m.created_at).toLocaleString()}</p>{/if}
                    </div>
                    <div class="shrink-0 flex flex-col items-end gap-2">
                      <button onclick={() => restoreMemory(m)} disabled={isProcessing}
                        class="px-3 py-1 text-xs rounded-lg bg-cosmic-blue/30 hover:bg-cosmic-blue/60">↩ Restore</button>
                      {#if m.reviewed}
                        <span class="text-xs text-gray-500" title="You checked this ruling and kept it">✓ checked</span>
                      {:else}
                        <button onclick={() => keepRuling(m)} disabled={isProcessing}
                          class="px-3 py-1 text-xs rounded-lg bg-white/10 hover:bg-white/20"
                          title="The model was right: keep it hidden">✓ Keep</button>
                      {/if}
                    </div>
                  </div>
                {/each}
                {#if hidden && hidden.length === 0}
                  <p class="text-xs text-gray-400">Nothing merged or replaced yet.</p>
                {/if}
              {/if}
            </div>
          {/if}
        </div>

      {:else if activeTab === 'documents'}
        <div class="space-y-6">
          <div class="flex items-center justify-between">
            <h3 class="text-xl font-bold text-cosmic-cyan">Documents</h3>
            <div class="flex gap-2">
              {#if watchFolders.length}
                <button onclick={syncFolders} disabled={isProcessing || extUnavailable}
                  class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm disabled:opacity-50">🔄 Sync folders</button>
              {/if}
              <button onclick={indexDocument} disabled={isProcessing || !available}
                title={!available ? 'Set up kb-core first (scripts/setup-memory.sh)' : undefined}
                class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm disabled:opacity-50">
                {isProcessing ? '⏳ Working...' : '📄 Index Document'}
              </button>
            </div>
          </div>

          {#if watchFolders.length}
            <div class="glass-panel p-4 bg-white/5 text-sm space-y-1">
              <div class="font-bold">Watched folders <span class="text-xs text-gray-400 font-normal">re-scanned every {Math.round((data?.watch?.interval_s ?? 120) / 60)} min · last {ago(data?.watch?.last_run)}</span></div>
              {#each watchFolders as f}
                <div class="flex justify-between text-xs text-gray-300">
                  <code>{f}</code>
                  {#if data?.watch?.errors?.[f]}
                    <span class="text-red-400">{data.watch.errors[f]}</span>
                  {:else if data?.watch?.last_result?.[f]}
                    <span class="text-gray-400">{data.watch.last_result[f].files} files</span>
                  {/if}
                </div>
              {/each}
            </div>
          {:else if extended}
            <p class="text-xs text-gray-400">Tip: keep a notes folder indexed live with <code>./scripts/setup-memory.sh ~/notes</code>; edits are picked up automatically.</p>
          {/if}

          <div class="space-y-2">
            {#each documents as doc (doc.id)}
              <div class="glass-panel p-3 bg-white/5 hover:bg-white/10 transition-all flex items-center justify-between gap-3">
                <div class="flex items-center gap-3 min-w-0">
                  <span class="text-2xl">{doc.type === 'md' ? '📝' : doc.type === 'txt' ? '📄' : '📋'}</span>
                  <div class="min-w-0">
                    <div class="font-bold text-sm truncate" title={doc.sourcePath ?? doc.name}>{doc.name}</div>
                    <div class="text-xs text-gray-400">{formatBytes(doc.size)} • {doc.chunks} chunks • {ago(doc.timestamp)}{doc.sourcePath ? ' • watched' : ''}</div>
                  </div>
                </div>
                <div class="flex items-center gap-2 shrink-0">
                  {#if doc.indexed}
                    <span class="text-xs px-2 py-1 rounded bg-green-500/20 text-green-400">✓ Indexed</span>
                  {:else}
                    <span class="text-xs px-2 py-1 rounded bg-yellow-500/20 text-yellow-400" title="Keyword-searchable now; embeddings are being computed">⏳ Embedding</span>
                  {/if}
                  <button onclick={() => deleteDocument(doc)} disabled={isProcessing} class="text-red-400 hover:text-red-300 text-sm" title="Remove from index">🗑️</button>
                </div>
              </div>
            {/each}
            {#if documents.length === 0}
              <div class="text-center py-12 text-gray-400">
                <div class="text-4xl mb-2">📄</div>
                <p>No documents indexed yet</p>
                <p class="text-xs mt-2">Index a file here, or watch a folder of notes (see above).</p>
              </div>
            {/if}
          </div>
        </div>

      {:else if activeTab === 'knowledge-bases'}
        <div class="space-y-5">
          <div>
            <h3 class="text-xl font-bold text-cosmic-cyan">Knowledge Bases</h3>
            <p class="text-sm text-gray-400 mt-1">Separate stores for different parts of your life or work. Search one or all; OMNIX's automatic recall uses all except your conversation archive.</p>
          </div>
          {#if !extended}
            <p class="text-sm text-gray-400">Knowledge bases need the bundled kb-core service.</p>
          {:else}
            <div class="glass-panel p-4 bg-white/5 grid grid-cols-1 md:grid-cols-[1fr_2fr_auto] gap-2 items-end">
              <div>
                <label for="kb-new-name" class="block text-xs text-gray-400 mb-1">Name</label>
                <input id="kb-new-name" bind:value={newKb.name} placeholder="e.g. homelab" class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
              </div>
              <div>
                <label for="kb-new-desc" class="block text-xs text-gray-400 mb-1">Description</label>
                <input id="kb-new-desc" bind:value={newKb.description} placeholder="What goes in it" class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
              </div>
              <button onclick={createKb} disabled={isProcessing || !newKb.name.trim()} class="px-5 py-2 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg text-sm disabled:opacity-50">➕ Create</button>
            </div>
            <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
              {#each kbs as kb (kb.name)}
                <div class="glass-panel p-4 bg-white/5 space-y-2">
                  <div class="flex justify-between items-start gap-2">
                    <div class="min-w-0">
                      <div class="font-bold">{kb.name}</div>
                      <div class="text-xs text-gray-400">{kb.description || (kb.name === 'conversations' ? 'Archive of your chats with OMNIX' : '—')}</div>
                    </div>
                    {#if kb.name !== 'default'}
                      <button onclick={() => deleteKb(kb.name)} disabled={isProcessing} class="text-xs text-red-400 hover:text-red-300">Delete</button>
                    {/if}
                  </div>
                  <div class="text-xs text-gray-300">{kb.memories} memories · {kb.documents} documents · {kb.chunks} chunks · updated {ago(kb.updated_at)}</div>
                  <div class="flex gap-2 text-xs">
                    <button onclick={() => { targetKb = kb.name === 'default' ? '' : kb.name; activeTab = 'memories'; }} class="px-2 py-1 rounded bg-white/5 hover:bg-white/10">Add memories here</button>
                    <button onclick={() => { searchKb = kb.name; activeTab = 'search'; }} class="px-2 py-1 rounded bg-white/5 hover:bg-white/10">Search it</button>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>

      {:else if activeTab === 'search'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan">Search</h3>
          <div class="glass-panel p-4 bg-white/5">
            <div class="flex gap-3">
              <input
                type="text"
                bind:value={searchQuery}
                onkeydown={(e) => e.key === 'Enter' && available && semanticSearch()}
                placeholder="Search memories and notes by meaning or keyword…"
                class="flex-1 bg-white/5 border border-white/10 rounded-lg px-4 py-3 text-sm"
              />
              {#if kbs.length > 1}
                <select id="kv-search-kb" bind:value={searchKb} class="bg-white/5 border border-white/10 rounded-lg px-3 py-3 text-sm" aria-label="Knowledge base to search">
                  <option value="">All knowledge bases</option>
                  {#each kbs as kb}<option value={kb.name}>{kb.name}</option>{/each}
                </select>
              {/if}
              <button
                onclick={semanticSearch}
                disabled={isSearching || !searchQuery.trim() || !available}
                class="px-6 py-3 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg font-medium transition-all disabled:opacity-50"
              >
                {isSearching ? '⏳ Searching...' : '🔍 Search'}
              </button>
            </div>
            <p class="mt-3 text-xs text-gray-400">
              Hybrid search: meaning (embeddings) plus exact keywords, with frequently used and important memories ranked higher.
              {#if data?.embeddingModel}Model: {data.embeddingModel}{#if data.vectorDimensions} · {data.vectorDimensions} dimensions{/if}{/if}
            </p>
          </div>

          {#if searchResults.length > 0}
            <div class="space-y-3">
              <h4 class="font-bold text-sm">Results ({searchResults.length})</h4>
              {#each searchResults as result (result.id)}
                <div class="glass-panel p-4 bg-white/5 hover:bg-white/10 transition-all">
                  <div class="flex items-start justify-between gap-4">
                    <div class="flex-1 min-w-0">
                      <div class="text-xs text-gray-400 mb-1">
                        {result.kind === 'document' ? `📝 ${result.source}` : '💭 memory'}
                      </div>
                      <p class="text-sm mb-2 whitespace-pre-wrap break-words">{result.content}</p>
                      <div class="flex gap-2 flex-wrap">
                        {#each result.tags as tag}
                          <span class="text-xs px-2 py-1 rounded bg-white/5">#{tag}</span>
                        {/each}
                      </div>
                    </div>
                    <div class="text-right shrink-0">
                      <div class="text-sm font-bold text-cosmic-cyan">{Math.round(result.score * 100)}%</div>
                      <div class="text-xs text-gray-400">relevance</div>
                      {#if result.similarity != null}
                        <div class="text-xs text-gray-500 mt-1" title="raw cosine similarity">cos {result.similarity.toFixed(2)}</div>
                      {/if}
                    </div>
                  </div>
                </div>
              {/each}
            </div>
          {:else if searched && !isSearching}
            <div class="text-center py-12 text-gray-400">
              <div class="text-4xl mb-2">🔍</div>
              <p>No results found</p>
            </div>
          {/if}
        </div>

      {:else if activeTab === 'analytics'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan">Analytics</h3>
          {#if !extended}
            <p class="text-sm text-gray-400">Analytics need the bundled kb-core service (<code>./scripts/setup-memory.sh</code>).</p>
          {:else if data}
            <div class="grid grid-cols-2 gap-4">
              <div class="glass-panel p-6 bg-white/5">
                <h4 class="font-bold mb-4">Memories by category</h4>
                <div class="space-y-3">
                  {#each categoryRows as [category, count]}
                    <div>
                      <div class="flex justify-between text-sm mb-1">
                        <span>{category}</span>
                        <span class="text-cosmic-cyan">{count}</span>
                      </div>
                      <div class="w-full h-2 bg-white/10 rounded-full overflow-hidden">
                        <div class="h-full bg-cosmic-cyan" style="width: {(count / categoryMax) * 100}%"></div>
                      </div>
                    </div>
                  {/each}
                  {#if categoryRows.length === 0}<p class="text-sm text-gray-400">No memories yet.</p>{/if}
                </div>
                {#if Object.keys(data.sources ?? {}).length}
                  <div class="flex gap-3 flex-wrap mt-4 text-xs text-gray-400">
                    {#each Object.entries(data.sources ?? {}) as [src, n]}
                      <span>{sourceLabel[src] ?? src}: {n}</span>
                    {/each}
                  </div>
                {/if}
              </div>

              <div class="glass-panel p-6 bg-white/5">
                <h4 class="font-bold mb-4">Engine</h4>
                <div class="space-y-2 text-sm">
                  <div class="flex justify-between"><span>Status</span>
                    <span class="{data.status === 'ok' ? 'text-green-400' : 'text-yellow-400'} flex items-center gap-1">
                      <span class="w-2 h-2 rounded-full {data.status === 'ok' ? 'bg-green-400' : 'bg-yellow-400'}"></span>{data.status === 'ok' ? 'Healthy' : 'Keyword-only (embeddings down)'}
                    </span></div>
                  <div class="flex justify-between"><span>Embedding model</span><span class="text-cosmic-cyan">{data.embeddingModel} · {data.vectorDimensions}d</span></div>
                  <div class="flex justify-between"><span>Learning model</span><span class="text-cosmic-cyan">{data.llmModel ?? 'off'}</span></div>
                  <div class="flex justify-between"><span>Waiting for embedding</span><span>{data.pendingEmbeddings ?? 0}</span></div>
                  <div class="flex justify-between"><span>Merged or superseded memories</span><span>{data.superseded ?? 0}</span></div>
                  <div class="flex justify-between"><span>Awaiting your review</span><span>{data.needsReview ?? 0}</span></div>
                  <div class="flex justify-between"><span>Memory links</span><span>{data.links ?? 0}</span></div>
                  <div class="flex justify-between"><span>Searches (24h)</span><span>{data.searches24h ?? 0}{#if data.avgSearchMs != null} · avg {data.avgSearchMs} ms{/if}</span></div>
                  <div class="flex justify-between"><span>Last optimization</span><span class="text-gray-400">{ago(data.lastMaintenance)}</span></div>
                  <div class="flex justify-between"><span>Storage</span><span>{formatBytes(data.storageUsed)}</span></div>
                </div>
              </div>

              <div class="glass-panel p-6 bg-white/5 col-span-2">
                <h4 class="font-bold mb-4">Recent activity</h4>
                <div class="space-y-2">
                  {#each data.recentActivity ?? [] as ev}
                    <div class="flex items-center gap-3 text-sm">
                      <span>{activityLabel[ev.kind] ?? ev.kind}</span>
                      <span class="text-xs text-gray-400 truncate">{activityDetail(ev.kind, ev.detail)}</span>
                      <span class="ml-auto text-gray-400 text-xs shrink-0">{ago(ev.ts)}</span>
                    </div>
                  {/each}
                  {#if !(data.recentActivity ?? []).length}<p class="text-sm text-gray-400">Nothing yet.</p>{/if}
                </div>
              </div>
            </div>
          {/if}
        </div>
      {/if}
    </div>
  </div>
</div>
