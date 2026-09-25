/**
 * Typed wrapper around Tauri `invoke` plus helpers for the structured
 * `AppError` the Rust backend returns (`{ kind, message }`).
 */
import { invoke } from '@tauri-apps/api/core';

/** Mirrors `error::AppError::kind()`. */
export type AppErrorKind =
  | 'not_implemented'
  | 'policy_denied'
  | 'not_approved'
  | 'invalid_input'
  | 'local_only'
  | 'unavailable'
  | 'execution'
  | 'secret'
  | 'io'
  | 'http'
  | 'json'
  | 'internal';

/** Structured error from the backend. */
export interface AppError {
  kind: AppErrorKind;
  message: string;
}

/**
 * Commands that are wired but return `not_implemented`. Controls bound to
 * them are rendered disabled with a "Not yet available" badge.
 */
export const NOT_IMPLEMENTED = new Set<string>([
  'create_knowledge_base',
  'export_knowledge',
  'import_knowledge',
  'optimize_vector_db',
  'test_integration',
  'toggle_service',
  'create_automation',
  'toggle_automation',
  'delete_automation',
  'create_scheduled_task',
  'toggle_scheduled_task',
  'create_alert',
  'toggle_alert',
  'run_system_cleanup',
  'optimize_system'
]);

/** True if `cmd` is known to be unimplemented in the backend. */
export function unavailable(cmd: string): boolean {
  return NOT_IMPLEMENTED.has(cmd);
}

/** Type guard for the backend's structured error. */
export function isAppError(e: unknown): e is AppError {
  return (
    typeof e === 'object' &&
    e !== null &&
    typeof (e as AppError).kind === 'string' &&
    typeof (e as AppError).message === 'string'
  );
}

/** True if `e` is a `not_implemented` error. */
export function isNotImplemented(e: unknown): boolean {
  return isAppError(e) && e.kind === 'not_implemented';
}

/** Human-readable message for any thrown value. */
export function errorMessage(e: unknown): string {
  if (isAppError(e)) return e.message;
  if (e instanceof Error) return e.message;
  return String(e);
}

/** Typed `invoke`. Rejects with an {@link AppError} (or a Tauri string error). */
export function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}
