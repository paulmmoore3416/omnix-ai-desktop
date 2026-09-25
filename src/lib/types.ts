/**
 * TypeScript mirrors of the Rust IPC types (src-tauri/src).
 * Keep in sync with the `Serialize`/`Deserialize` structs; field names are
 * snake_case on the wire.
 */

export type RiskTier = 'read_only' | 'mutating' | 'privileged' | 'denied';

/** Mirrors `settings::Settings`. */
export interface Settings {
  schema_version: number;
  general: {
    theme: string;
    language: string;
    auto_start: boolean;
    notifications: boolean;
    sound_effects: boolean;
    minimize_to_tray: boolean;
    check_updates: boolean;
  };
  ai: {
    provider: 'ollama' | 'openai' | 'anthropic' | 'gemini' | 'xai';
    ollama_host: string;
    ollama_model: string;
    /** Read-only: computed from the OS keychain. */
    has_openai_key: boolean;
    has_anthropic_key: boolean;
    has_gemini_key: boolean;
    has_xai_key: boolean;
    temperature: number;
    max_tokens: number;
    stream_responses: boolean;
    context_window: number;
  };
  memresort: {
    enabled: boolean;
    host: string;
    port: number;
    auto_connect: boolean;
  };
  integrations: Record<string, unknown>;
  voice: {
    enabled: boolean;
    whisper_model: string;
    language: string;
    tts_engine: string;
    tts_voice: string;
    wake_word: string;
    continuous_listening: boolean;
  };
  memory: {
    max_memory_size: number;
    auto_summarize: boolean;
    retention_days: number;
    enable_semantic_search: boolean;
  };
  security: {
    require_confirmation: boolean;
    enable_sudo: boolean;
    local_only: boolean;
    log_all_commands: boolean;
    encrypt_memory: boolean;
    audit_log: boolean;
    allowed_commands: string[];
    blocked_commands: string[];
    command_timeout_secs: number;
    max_output_bytes: number;
    confirmation_timeout_secs: number;
    autonomous_mode: boolean;
    max_autonomous_steps: number;
  };
  performance: {
    max_concurrent_tasks: number;
    cache_enabled: boolean;
    cache_size: number;
    monitoring_interval: number;
    adaptive_refresh: boolean;
    low_power_mode: boolean;
  };
}

/** Provider ids accepted by `set_secret` / `delete_secret` / `has_secret`. */
export type SecretProvider =
  | 'openai'
  | 'anthropic'
  | 'gemini'
  | 'xai'
  | 'github'
  | 'google_drive'
  | 'jira'
  | 'notion';

/** Mirrors `security::executor::ExecResult`. */
export interface ExecResult {
  request_id: string;
  exit_code: number | null;
  stdout: string;
  stderr: string;
  truncated: boolean;
  duration_ms: number;
  tier: RiskTier;
}

/** Mirrors `security::audit::VerifyReport`. */
export interface VerifyReport {
  valid: boolean;
  entries: number;
  first_bad_line: number | null;
  reason: string | null;
}

/** Mirrors `system::metrics::SystemStatus`. */
export interface SystemStatus {
  cpu: number;
  memory: number;
  status: string;
  uptime: number;
  processes: number;
}

/** Mirrors `system::metrics::SystemInfo`. */
export interface SystemInfo {
  os: string | null;
  kernel: string | null;
  os_version: string | null;
  hostname: string | null;
  uptime: number;
  cpu_model: string | null;
  cpu_cores: number;
  total_memory: number;
  used_memory: number;
  total_swap: number;
  used_swap: number;
}

/** Mirrors `system::metrics::RealTimeStats`. `null` = not measured. */
export interface RealTimeStats {
  cpu: number;
  memory: number;
  swap: number;
  disk: number | null;
  network: { rx: number; tx: number } | null;
  temperature: number | null;
}

/** Mirrors `system::processes::ProcessInfo`. */
export interface ProcessInfo {
  pid: number;
  name: string;
  cpu: number;
  memory: number;
  status: string;
}

/** Shape of `get_knowledge_data`. */
export interface KnowledgeData {
  available: boolean;
  totalMemories: number;
  totalDocuments: number;
  totalKnowledgeBases: number;
  storageUsed: number;
  vectorDimensions: number;
  embeddingModel: string;
  memories: unknown[];
  documents: unknown[];
  knowledgeBases: unknown[];
}

/** Shape of `get_system_control_data`. */
export interface SystemControlData {
  available: boolean;
  processes: ProcessInfo[];
  services: unknown[];
  automations: unknown[];
  scheduledTasks: unknown[];
  alerts: unknown[];
}
