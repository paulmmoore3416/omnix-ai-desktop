import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(async () => 'hello world') }));

import { invoke } from '@tauri-apps/api/core';
import { stripMarkdown, transcribe } from './voice';

describe('voice helpers', () => {
  it('strips markdown for speech', () => {
    expect(stripMarkdown('# Title\n**bold** and `x` [link](http://a)')).toBe('Title bold and x link');
    expect(stripMarkdown('run:\n```\nrm -rf x\n```\ndone')).toBe('run: (code omitted) done');
  });

  it('sends raw bytes with the mime header', async () => {
    const bytes = new Uint8Array([1, 2, 3]);
    await expect(transcribe(bytes, 'audio/webm')).resolves.toBe('hello world');
    expect(invoke).toHaveBeenCalledWith('voice_transcribe', bytes, { headers: { 'x-audio-mime': 'audio/webm' } });
  });
});
