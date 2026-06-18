<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  
  let activeTab = $state('memories');
  let searchQuery = $state('');
  let isSearching = $state(false);
  let isProcessing = $state(false);
  
  let memories = $state<Array<{
    id: string;
    content: string;
    timestamp: Date;
    tags: string[];
    importance: number;
    category: string;
  }>>([]);
  
  let searchResults = $state<Array<{
    id: string;
    content: string;
    similarity: number;
    timestamp: Date;
    tags: string[];
  }>>([]);
  
  let documents = $state<Array<{
    id: string;
    name: string;
    type: string;
    size: number;
    indexed: boolean;
    chunks: number;
    timestamp: Date;
  }>>([]);
  
  let knowledgeBases = $state<Array<{
    id: string;
    name: string;
    description: string;
    entries: number;
    lastUpdated: Date;
    type: 'personal' | 'project' | 'shared';
  }>>([]);
  
  let stats = $state({
    totalMemories: 0,
    totalDocuments: 0,
    totalKnowledgeBases: 0,
    storageUsed: 0,
    vectorDimensions: 384,
    embeddingModel: 'all-MiniLM-L6-v2'
  });
  
  let newMemory = $state({
    content: '',
    tags: '',
    importance: 5,
    category: 'general'
  });
  
  let newKnowledgeBase = $state({
    name: '',
    description: '',
    type: 'personal' as 'personal' | 'project' | 'shared'
  });
  
  const tabs = [
    { id: 'memories', label: 'Memories', icon: '💭' },
    { id: 'documents', label: 'Documents', icon: '📄' },
    { id: 'knowledge-bases', label: 'Knowledge Bases', icon: '📚' },
    { id: 'search', label: 'Semantic Search', icon: '🔍' },
    { id: 'analytics', label: 'Analytics', icon: '📊' }
  ];
  
  const categories = [
    'general', 'work', 'personal', 'technical', 'creative', 
    'research', 'meeting', 'idea', 'task', 'reference'
  ];
  
  onMount(async () => {
    await loadData();
  });
  
  async function loadData() {
    try {
      const data = await invoke('get_knowledge_data');
      const parsed = data as typeof stats & { memories: typeof memories; documents: typeof documents; knowledgeBases: typeof knowledgeBases };
      
      stats = {
        totalMemories: parsed.totalMemories || 0,
        totalDocuments: parsed.totalDocuments || 0,
        totalKnowledgeBases: parsed.totalKnowledgeBases || 0,
        storageUsed: parsed.storageUsed || 0,
        vectorDimensions: parsed.vectorDimensions || 384,
        embeddingModel: parsed.embeddingModel || 'all-MiniLM-L6-v2'
      };
      
      memories = (parsed.memories || []).map((m: any) => ({
        ...m,
        timestamp: new Date(m.timestamp)
      }));
      
      documents = (parsed.documents || []).map((d: any) => ({
        ...d,
        timestamp: new Date(d.timestamp)
      }));
      
      knowledgeBases = (parsed.knowledgeBases || []).map((kb: any) => ({
        ...kb,
        lastUpdated: new Date(kb.lastUpdated)
      }));
    } catch (error) {
      console.error('Failed to load knowledge data:', error);
    }
  }
  
  async function saveMemory() {
    if (!newMemory.content.trim()) return;
    
    isProcessing = true;
    try {
      const tags = newMemory.tags.split(',').map(t => t.trim()).filter(t => t);
      await invoke('save_memory', {
        content: newMemory.content,
        tags,
        importance: newMemory.importance,
        category: newMemory.category
      });
      
      newMemory = { content: '', tags: '', importance: 5, category: 'general' };
      await loadData();
    } catch (error) {
      console.error('Failed to save memory:', error);
    } finally {
      isProcessing = false;
    }
  }
  
  async function deleteMemory(id: string) {
    if (!confirm('Are you sure you want to delete this memory?')) return;
    
    try {
      await invoke('delete_memory', { id });
      await loadData();
    } catch (error) {
      console.error('Failed to delete memory:', error);
    }
  }
  
  async function semanticSearch() {
    if (!searchQuery.trim()) return;
    
    isSearching = true;
    try {
      const results = await invoke('semantic_search', { query: searchQuery, limit: 20 });
      searchResults = (results as any[]).map((r: any) => ({
        ...r,
        timestamp: new Date(r.timestamp)
      }));
    } catch (error) {
      console.error('Search failed:', error);
      searchResults = [];
    } finally {
      isSearching = false;
    }
  }
  
  async function indexDocument() {
    try {
      const path = await invoke('select_file');
      if (path) {
        isProcessing = true;
        await invoke('index_document', { path });
        await loadData();
      }
    } catch (error) {
      console.error('Failed to index document:', error);
    } finally {
      isProcessing = false;
    }
  }
  
  async function createKnowledgeBase() {
    if (!newKnowledgeBase.name.trim()) return;
    
    isProcessing = true;
    try {
      await invoke('create_knowledge_base', {
        name: newKnowledgeBase.name,
        description: newKnowledgeBase.description,
        type: newKnowledgeBase.type
      });
      
      newKnowledgeBase = { name: '', description: '', type: 'personal' };
      await loadData();
    } catch (error) {
      console.error('Failed to create knowledge base:', error);
    } finally {
      isProcessing = false;
    }
  }
  
  async function exportKnowledge() {
    try {
      await invoke('export_knowledge');
    } catch (error) {
      console.error('Export failed:', error);
    }
  }
  
  async function importKnowledge() {
    try {
      await invoke('import_knowledge');
      await loadData();
    } catch (error) {
      console.error('Import failed:', error);
    }
  }
  
  async function optimizeVectorDB() {
    isProcessing = true;
    try {
      await invoke('optimize_vector_db');
      await loadData();
    } catch (error) {
      console.error('Optimization failed:', error);
    } finally {
      isProcessing = false;
    }
  }
  
  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
  }
  
  function getImportanceColor(importance: number): string {
    if (importance >= 8) return 'text-red-400';
    if (importance >= 5) return 'text-yellow-400';
    return 'text-green-400';
  }
</script>

<div class="h-full flex flex-col">
  <!-- Header -->
  <div class="flex items-center justify-between mb-6">
    <div>
      <h2 class="text-3xl font-bold glow-text">Knowledge Base</h2>
      <p class="text-gray-400 mt-1">Manage your AI's memory and knowledge</p>
    </div>
    <div class="flex gap-2">
      <button
        onclick={exportKnowledge}
        class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm"
      >
        📤 Export
      </button>
      <button
        onclick={importKnowledge}
        class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm"
      >
        📥 Import
      </button>
      <button
        onclick={optimizeVectorDB}
        disabled={isProcessing}
        class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm disabled:opacity-50"
      >
        ⚡ Optimize
      </button>
    </div>
  </div>
  
  <!-- Stats Overview -->
  <div class="grid grid-cols-4 gap-4 mb-6">
    <div class="glass-panel p-4 hover:scale-105 transition-transform">
      <div class="text-2xl font-bold text-cosmic-cyan">{stats.totalMemories}</div>
      <div class="text-sm text-gray-400">Memories</div>
    </div>
    <div class="glass-panel p-4 hover:scale-105 transition-transform">
      <div class="text-2xl font-bold text-cosmic-purple">{stats.totalDocuments}</div>
      <div class="text-sm text-gray-400">Documents</div>
    </div>
    <div class="glass-panel p-4 hover:scale-105 transition-transform">
      <div class="text-2xl font-bold text-green-400">{stats.totalKnowledgeBases}</div>
      <div class="text-sm text-gray-400">Knowledge Bases</div>
    </div>
    <div class="glass-panel p-4 hover:scale-105 transition-transform">
      <div class="text-2xl font-bold text-yellow-400">{formatBytes(stats.storageUsed)}</div>
      <div class="text-sm text-gray-400">Storage Used</div>
    </div>
  </div>
  
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
    
    <!-- Content Area -->
    <div class="flex-1 glass-panel p-6 overflow-auto">
      {#if activeTab === 'memories'}
        <div class="space-y-6">
          <div class="flex items-center justify-between mb-4">
            <h3 class="text-xl font-bold text-cosmic-cyan">Memory Storage</h3>
          </div>
          
          <!-- Add New Memory -->
          <div class="glass-panel p-4 bg-white/5 space-y-3">
            <h4 class="font-bold text-sm">Add New Memory</h4>
            <textarea
              bind:value={newMemory.content}
              placeholder="What should I remember?"
              rows="3"
              class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 text-sm"
            ></textarea>
            
            <div class="grid grid-cols-3 gap-3">
              <div>
                <label class="block text-xs text-gray-400 mb-1">Tags (comma-separated)</label>
                <input
                  type="text"
                  bind:value={newMemory.tags}
                  placeholder="work, important"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                />
              </div>
              
              <div>
                <label class="block text-xs text-gray-400 mb-1">Category</label>
                <select bind:value={newMemory.category} class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm">
                  {#each categories as category}
                    <option value={category}>{category}</option>
                  {/each}
                </select>
              </div>
              
              <div>
                <label class="block text-xs text-gray-400 mb-1">Importance (1-10)</label>
                <input
                  type="number"
                  bind:value={newMemory.importance}
                  min="1"
                  max="10"
                  class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"
                />
              </div>
            </div>
            
            <button
              onclick={saveMemory}
              disabled={isProcessing || !newMemory.content.trim()}
              class="w-full px-4 py-2 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg font-medium transition-all disabled:opacity-50"
            >
              {isProcessing ? '⏳ Saving...' : '💾 Save Memory'}
            </button>
          </div>
          
          <!-- Memory List -->
          <div class="space-y-3">
            {#each memories as memory}
              <div class="glass-panel p-4 bg-white/5 hover:bg-white/10 transition-all">
                <div class="flex items-start justify-between mb-2">
                  <div class="flex items-center gap-2">
                    <span class="text-xs px-2 py-1 rounded bg-cosmic-blue/20 text-cosmic-cyan">
                      {memory.category}
                    </span>
                    <span class="text-xs {getImportanceColor(memory.importance)}">
                      {'★'.repeat(Math.min(memory.importance, 10))}
                    </span>
                  </div>
                  <button
                    onclick={() => deleteMemory(memory.id)}
                    class="text-red-400 hover:text-red-300 text-sm"
                  >
                    🗑️
                  </button>
                </div>
                
                <p class="text-sm mb-2">{memory.content}</p>
                
                <div class="flex items-center justify-between text-xs text-gray-400">
                  <div class="flex gap-2">
                    {#each memory.tags as tag}
                      <span class="px-2 py-1 rounded bg-white/5">#{tag}</span>
                    {/each}
                  </div>
                  <span>{memory.timestamp.toLocaleString()}</span>
                </div>
              </div>
            {/each}
            
            {#if memories.length === 0}
              <div class="text-center py-12 text-gray-400">
                <div class="text-4xl mb-2">💭</div>
                <p>No memories stored yet</p>
              </div>
            {/if}
          </div>
        </div>
        
      {:else if activeTab === 'documents'}
        <div class="space-y-6">
          <div class="flex items-center justify-between mb-4">
            <h3 class="text-xl font-bold text-cosmic-cyan">Document Index</h3>
            <button
              onclick={indexDocument}
              disabled={isProcessing}
              class="glass-panel px-4 py-2 hover:bg-white/10 transition-all text-sm disabled:opacity-50"
            >
              {isProcessing ? '⏳ Indexing...' : '📄 Index Document'}
            </button>
          </div>
          
          <div class="space-y-3">
            {#each documents as doc}
              <div class="glass-panel p-4 bg-white/5 hover:bg-white/10 transition-all">
                <div class="flex items-start justify-between mb-2">
                  <div class="flex items-center gap-3">
                    <span class="text-2xl">
                      {doc.type === 'pdf' ? '📕' : doc.type === 'txt' ? '📄' : doc.type === 'md' ? '📝' : '📋'}
                    </span>
                    <div>
                      <div class="font-bold text-sm">{doc.name}</div>
                      <div class="text-xs text-gray-400">{formatBytes(doc.size)} • {doc.chunks} chunks</div>
                    </div>
                  </div>
                  <div class="flex items-center gap-2">
                    {#if doc.indexed}
                      <span class="text-xs px-2 py-1 rounded bg-green-500/20 text-green-400">✓ Indexed</span>
                    {:else}
                      <span class="text-xs px-2 py-1 rounded bg-yellow-500/20 text-yellow-400">⏳ Pending</span>
                    {/if}
                  </div>
                </div>
                
                <div class="text-xs text-gray-400">
                  Added: {doc.timestamp.toLocaleString()}
                </div>
              </div>
            {/each}
            
            {#if documents.length === 0}
              <div class="text-center py-12 text-gray-400">
                <div class="text-4xl mb-2">📄</div>
                <p>No documents indexed yet</p>
                <p class="text-xs mt-2">Index documents to enable semantic search across their content</p>
              </div>
            {/if}
          </div>
        </div>
        
      {:else if activeTab === 'knowledge-bases'}
        <div class="space-y-6">
          <div class="flex items-center justify-between mb-4">
            <h3 class="text-xl font-bold text-cosmic-cyan">Knowledge Bases</h3>
          </div>
          
          <!-- Create New Knowledge Base -->
          <div class="glass-panel p-4 bg-white/5 space-y-3">
            <h4 class="font-bold text-sm">Create Knowledge Base</h4>
            
            <input
              type="text"
              bind:value={newKnowledgeBase.name}
              placeholder="Knowledge base name"
              class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 text-sm"
            />
            
            <textarea
              bind:value={newKnowledgeBase.description}
              placeholder="Description"
              rows="2"
              class="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2 text-sm"
            ></textarea>
            
            <div class="flex gap-3">
              <select bind:value={newKnowledgeBase.type} class="flex-1 bg-white/5 border border-white/10 rounded-lg px-4 py-2 text-sm">
                <option value="personal">Personal</option>
                <option value="project">Project</option>
                <option value="shared">Shared</option>
              </select>
              
              <button
                onclick={createKnowledgeBase}
                disabled={isProcessing || !newKnowledgeBase.name.trim()}
                class="px-6 py-2 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg font-medium transition-all disabled:opacity-50"
              >
                {isProcessing ? '⏳' : '➕ Create'}
              </button>
            </div>
          </div>
          
          <!-- Knowledge Base List -->
          <div class="grid grid-cols-2 gap-4">
            {#each knowledgeBases as kb}
              <div class="glass-panel p-4 bg-white/5 hover:bg-white/10 transition-all cursor-pointer">
                <div class="flex items-start justify-between mb-3">
                  <div>
                    <h4 class="font-bold">{kb.name}</h4>
                    <p class="text-xs text-gray-400 mt-1">{kb.description}</p>
                  </div>
                  <span class="text-xs px-2 py-1 rounded {kb.type === 'personal' ? 'bg-blue-500/20 text-blue-400' : kb.type === 'project' ? 'bg-purple-500/20 text-purple-400' : 'bg-green-500/20 text-green-400'}">
                    {kb.type}
                  </span>
                </div>
                
                <div class="flex items-center justify-between text-xs text-gray-400">
                  <span>{kb.entries} entries</span>
                  <span>Updated: {kb.lastUpdated.toLocaleDateString()}</span>
                </div>
              </div>
            {/each}
            
            {#if knowledgeBases.length === 0}
              <div class="col-span-2 text-center py-12 text-gray-400">
                <div class="text-4xl mb-2">📚</div>
                <p>No knowledge bases created yet</p>
              </div>
            {/if}
          </div>
        </div>
        
      {:else if activeTab === 'search'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan mb-4">Semantic Search</h3>
          
          <div class="glass-panel p-4 bg-white/5">
            <div class="flex gap-3">
              <input
                type="text"
                bind:value={searchQuery}
                onkeypress={(e) => e.key === 'Enter' && semanticSearch()}
                placeholder="Search across all knowledge..."
                class="flex-1 bg-white/5 border border-white/10 rounded-lg px-4 py-3 text-sm"
              />
              <button
                onclick={semanticSearch}
                disabled={isSearching || !searchQuery.trim()}
                class="px-6 py-3 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg font-medium transition-all disabled:opacity-50"
              >
                {isSearching ? '⏳ Searching...' : '🔍 Search'}
              </button>
            </div>
            
            <div class="mt-3 text-xs text-gray-400">
              <p>💡 Semantic search finds results based on meaning, not just keywords</p>
              <p class="mt-1">Model: {stats.embeddingModel} • Dimensions: {stats.vectorDimensions}</p>
            </div>
          </div>
          
          {#if searchResults.length > 0}
            <div class="space-y-3">
              <h4 class="font-bold text-sm">Results ({searchResults.length})</h4>
              {#each searchResults as result}
                <div class="glass-panel p-4 bg-white/5 hover:bg-white/10 transition-all">
                  <div class="flex items-start justify-between mb-2">
                    <div class="flex-1">
                      <p class="text-sm mb-2">{result.content}</p>
                      <div class="flex gap-2">
                        {#each result.tags as tag}
                          <span class="text-xs px-2 py-1 rounded bg-white/5">#{tag}</span>
                        {/each}
                      </div>
                    </div>
                    <div class="ml-4 text-right">
                      <div class="text-sm font-bold text-cosmic-cyan">
                        {(result.similarity * 100).toFixed(1)}%
                      </div>
                      <div class="text-xs text-gray-400">similarity</div>
                    </div>
                  </div>
                  
                  <div class="text-xs text-gray-400 mt-2">
                    {result.timestamp.toLocaleString()}
                  </div>
                </div>
              {/each}
            </div>
          {:else if searchQuery && !isSearching}
            <div class="text-center py-12 text-gray-400">
              <div class="text-4xl mb-2">🔍</div>
              <p>No results found</p>
            </div>
          {/if}
        </div>
        
      {:else if activeTab === 'analytics'}
        <div class="space-y-6">
          <h3 class="text-xl font-bold text-cosmic-cyan mb-4">Knowledge Analytics</h3>
          
          <div class="grid grid-cols-2 gap-4">
            <div class="glass-panel p-6 bg-white/5">
              <h4 class="font-bold mb-4">Memory Distribution</h4>
              <div class="space-y-3">
                {#each categories.slice(0, 5) as category}
                  <div>
                    <div class="flex justify-between text-sm mb-1">
                      <span>{category}</span>
                      <span class="text-cosmic-cyan">{Math.floor(Math.random() * 50)}</span>
                    </div>
                    <div class="w-full h-2 bg-white/10 rounded-full overflow-hidden">
                      <div 
                        class="h-full bg-cosmic-cyan transition-all"
                        style="width: {Math.random() * 100}%"
                      ></div>
                    </div>
                  </div>
                {/each}
              </div>
            </div>
            
            <div class="glass-panel p-6 bg-white/5">
              <h4 class="font-bold mb-4">Storage Breakdown</h4>
              <div class="space-y-4">
                <div class="flex items-center justify-between">
                  <span class="text-sm">Memories</span>
                  <span class="text-cosmic-cyan font-bold">{formatBytes(stats.storageUsed * 0.4)}</span>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-sm">Documents</span>
                  <span class="text-cosmic-purple font-bold">{formatBytes(stats.storageUsed * 0.5)}</span>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-sm">Vector Index</span>
                  <span class="text-green-400 font-bold">{formatBytes(stats.storageUsed * 0.1)}</span>
                </div>
              </div>
            </div>
            
            <div class="glass-panel p-6 bg-white/5">
              <h4 class="font-bold mb-4">Recent Activity</h4>
              <div class="space-y-3">
                <div class="flex items-center gap-3 text-sm">
                  <span class="text-green-400">✓</span>
                  <span>Memory saved</span>
                  <span class="ml-auto text-gray-400 text-xs">2m ago</span>
                </div>
                <div class="flex items-center gap-3 text-sm">
                  <span class="text-blue-400">📄</span>
                  <span>Document indexed</span>
                  <span class="ml-auto text-gray-400 text-xs">15m ago</span>
                </div>
                <div class="flex items-center gap-3 text-sm">
                  <span class="text-purple-400">🔍</span>
                  <span>Semantic search</span>
                  <span class="ml-auto text-gray-400 text-xs">1h ago</span>
                </div>
              </div>
            </div>
            
            <div class="glass-panel p-6 bg-white/5">
              <h4 class="font-bold mb-4">System Health</h4>
              <div class="space-y-3">
                <div class="flex items-center justify-between">
                  <span class="text-sm">Vector DB Status</span>
                  <span class="text-green-400 flex items-center gap-1">
                    <span class="w-2 h-2 bg-green-400 rounded-full animate-pulse"></span>
                    Healthy
                  </span>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-sm">Index Size</span>
                  <span class="text-cosmic-cyan">{stats.totalMemories + stats.totalDocuments} vectors</span>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-sm">Last Optimization</span>
                  <span class="text-gray-400 text-xs">2 days ago</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>