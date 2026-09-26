import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { call, errorMessage, isAppError, isNotImplemented, unavailable } from './api';

describe('AppError helpers', () => {
  it('recognises the structured backend error', () => {
    const e = { kind: 'not_implemented', message: 'create_alert is not implemented yet' };
    expect(isAppError(e)).toBe(true);
    expect(isNotImplemented(e)).toBe(true);
    expect(errorMessage(e)).toBe('create_alert is not implemented yet');
  });

  it('handles non-AppError values', () => {
    expect(isAppError('boom')).toBe(false);
    expect(isAppError(null)).toBe(false);
    expect(isNotImplemented({ kind: 'policy_denied', message: 'x' })).toBe(false);
    expect(errorMessage(new Error('bad'))).toBe('bad');
    expect(errorMessage('plain string')).toBe('plain string');
  });

  it('knows which commands are unavailable', () => {
    expect(unavailable('test_integration')).toBe(true);
    for (const built of ['create_automation', 'create_alert', 'create_scheduled_task', 'run_system_cleanup', 'optimize_system', 'toggle_service', 'create_knowledge_base']) {
      expect(unavailable(built)).toBe(false);
    }
    expect(unavailable('request_execution')).toBe(false);
    expect(unavailable('kill_process')).toBe(false);
  });

  it('call() forwards to invoke with the command and args', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(42);
    await expect(call<number>('get_x', { a: 1 })).resolves.toBe(42);
    expect(invoke).toHaveBeenCalledWith('get_x', { a: 1 });
  });
});
