import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/svelte';

const state = vi.hoisted(() => ({ configured: false }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'get_knowledge_data') {
      if (!state.configured) {
        return {
          available: false,
          extended: false,
          totalMemories: 0,
          totalDocuments: 0,
          totalKnowledgeBases: 0,
          storageUsed: 0,
          vectorDimensions: 0,
          embeddingModel: '',
          memories: [],
          documents: [],
          knowledgeBases: []
        };
      }
      return {
        available: true,
        extended: true,
        status: 'ok',
        totalMemories: 1,
        totalDocuments: 1,
        totalChunks: 4,
        totalKnowledgeBases: 0,
        storageUsed: 2048,
        vectorDimensions: 768,
        embeddingModel: 'nomic-embed-text',
        llmModel: 'qwen3:8b',
        categories: { preference: 1 },
        sources: { extract: 1 },
        watch: { folders: ['/home/u/notes'], interval_s: 120, last_run: null, last_result: {}, errors: {} },
        recentActivity: [{ ts: '2026-09-25T20:00:00Z', kind: 'memory_saved', detail: {} }],
        memories: [
          {
            id: 'm_1',
            content: 'Paul prefers morning meetings',
            tags: ['prefs'],
            importance: 7,
            category: 'preference',
            timestamp: '2026-09-25T20:00:00Z',
            source: 'extract',
            pinned: false,
            reinforced: 2,
            accessCount: 3,
            activation: 0.8
          }
        ],
        documents: [
          { id: 'd_1', name: 'notes/pve.md', type: 'md', size: 1000, chunks: 4, indexed: true, sourcePath: '/home/u/notes/pve.md', timestamp: '2026-09-25T20:00:00Z' }
        ],
        knowledgeBases: []
      };
    }
    throw { kind: 'not_implemented', message: `${cmd} is not implemented yet` };
  })
}));

import KnowledgeView from './KnowledgeView.svelte';

afterEach(() => {
  cleanup();
  state.configured = false;
});

describe('KnowledgeView', () => {
  it('shows the not-configured banner and disables controls without a service', async () => {
    render(KnowledgeView);
    expect(await screen.findByText(/Long-term memory is disabled/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Export/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /Import/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /Optimize/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /Save Memory/ })).toBeDisabled();
  });

  it('shows real memory data and enables management with kb-core', async () => {
    state.configured = true;
    render(KnowledgeView);
    expect(await screen.findByText('Paul prefers morning meetings')).toBeInTheDocument();
    expect(screen.getByText('🧠 learned')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Export/ })).toBeEnabled();
    expect(screen.getByRole('button', { name: /Optimize/ })).toBeEnabled();
    expect(screen.queryByText(/Long-term memory is disabled/)).not.toBeInTheDocument();
  });
});
