import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(async () => 'hello world') }));

import { invoke } from '@tauri-apps/api/core';
import { describeMicError, downsample, encodeWav, stripMarkdown, transcribe } from './voice';

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

  it('encodes 16-bit mono PCM WAV with a valid header', () => {
    const wav = encodeWav(new Float32Array([0, 1, -1, 0.5]), 16000);
    const v = new DataView(wav.buffer);
    const tag = (o: number) => String.fromCharCode(...wav.slice(o, o + 4));
    expect(tag(0)).toBe('RIFF');
    expect(tag(8)).toBe('WAVE');
    expect(tag(36)).toBe('data');
    expect(v.getUint32(4, true)).toBe(36 + 8);
    expect(v.getUint16(22, true)).toBe(1); // mono
    expect(v.getUint32(24, true)).toBe(16000);
    expect(v.getUint32(40, true)).toBe(8);
    expect(v.getInt16(44 + 2, true)).toBe(0x7fff);
    expect(v.getInt16(44 + 4, true)).toBe(-0x8000);
  });

  it('downsamples 48 kHz to 16 kHz by averaging', () => {
    const out = downsample(new Float32Array([0, 0.3, 0.6, 1, 1, 1]), 48000, 16000);
    expect(out.length).toBe(2);
    expect(out[0]).toBeCloseTo(0.3);
    expect(out[1]).toBeCloseTo(1);
  });

  it('explains microphone failures', () => {
    expect(describeMicError({ name: 'NotAllowedError' })).toMatch(/denied/);
    expect(describeMicError({ name: 'NotFoundError' })).toMatch(/no microphone/);
    expect(describeMicError({ name: 'NotReadableError' })).toMatch(/busy/);
  });
});
