import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/svelte';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'get_knowledge_data') {
      return {
        available: false,
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
    throw { kind: 'not_implemented', message: `${cmd} is not implemented yet` };
  })
}));

import KnowledgeView from './KnowledgeView.svelte';

afterEach(cleanup);

describe('KnowledgeView honest stubs', () => {
  it('shows the not-available banner and disables unimplemented controls', async () => {
    render(KnowledgeView);
    expect(await screen.findByText(/Persistent memory and document indexing are not implemented yet/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Export/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /Import/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /Optimize/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /Save Memory/ })).toBeDisabled();
  });
});
