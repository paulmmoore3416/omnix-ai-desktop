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
    cloud_model: string;
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
  mcp: { servers: McpServerConfig[] };
  observability: { loki_url: string };
  voice: {
    enabled: boolean;
    stt_url: string;
    piper_path: string;
    whisper_model: string;
    language: string;
    tts_engine: string;
    tts_voice: string;
    wake_word: string;
    continuous_listening: boolean;
  };
  memory: {
    backend_url: string;
    auto_recall: boolean;
    recall_limit: number;
    recall_min_score: number;
    auto_capture: boolean;
    archive_conversations: boolean;
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
  phone: {
    enabled: boolean;
    account_sid: string;
    from_number: string;
    to_number: string;
    max_per_hour: number;
  };
}

/** Mirrors `phone::PhoneChannel`. */
export type PhoneChannel = 'sms' | 'call';

/** Mirrors `settings::McpServerConfig`. */
export interface McpServerConfig {
  name: string;
  enabled: boolean;
  transport:
    | { type: 'stdio'; command: string; args: string[]; env: Record<string, string>; secret_env: string[] }
    | { type: 'http'; url: string; bearer_token: boolean };
  read_only_tools: string[];
}

/** Mirrors `mcp::ServerStatus`. */
export interface McpServerStatus {
  name: string;
  tools: string[];
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
  | 'notion'
  | 'kb_core'
  | 'twilio'
  | `mcp.${string}.${string}`;

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
  /** The service offers kb-core's analytics/document extensions. */
  extended: boolean;
  totalMemories: number;
  totalDocuments: number;
  totalChunks?: number;
  totalKnowledgeBases: number;
  storageUsed: number;
  vectorDimensions: number;
  embeddingModel: string;
  llmModel?: string | null;
  status?: string;
  embedError?: string | null;
  pendingEmbeddings?: number;
  superseded?: number;
  /** Merged/superseded memories whose model ruling the user hasn't checked. */
  needsReview?: number;
  links?: number;
  categories?: Record<string, number>;
  sources?: Record<string, number>;
  searches24h?: number;
  avgSearchMs?: number | null;
  lastMaintenance?: string | null;
  watch?: {
    folders?: string[];
    interval_s?: number;
    last_run?: string | null;
    last_result?: Record<string, { files: number; indexed: number; unchanged: number; removed: number; skipped: number }>;
    errors?: Record<string, string>;
  };
  recentActivity?: Array<{ ts: string; kind: string; detail: Record<string, unknown> }>;
  memories: unknown[];
  documents: unknown[];
  knowledgeBases: unknown[];
}

/** Shape of `get_system_control_data`. */
/** Mirrors `system::gpu::GpuInfo`. */
export interface GpuInfo {
  index: number;
  vendor: string;
  name: string;
  driver: string | null;
  pci: string | null;
  utilization: number | null;
  memory_used: number | null;
  memory_total: number | null;
  temperature: number | null;
  power_w: number | null;
  power_limit_w: number | null;
  fan_percent: number | null;
  clock_mhz: number | null;
  clock_max_mhz: number | null;
  processes: { pid: number; name: string; memory: number }[];
  note: string | null;
}

/** Mirrors `system::metrics::DetailedStats`. */
export interface DetailedStats {
  cpu: number;
  per_core: number[];
  cpu_freq_mhz: number | null;
  load: [number, number, number];
  memory_total: number;
  memory_used: number;
  memory_available: number;
  swap_total: number;
  swap_used: number;
  disks: { name: string; mount: string; fs: string; total: number; used: number; removable: boolean }[];
  network: { name: string; rx: number; tx: number; total_rx: number; total_tx: number }[];
  sensors: { label: string; temperature: number; critical: number | null }[];
  uptime: number;
  processes: number;
}

/** Mirrors `system::history::Sample`. */
export interface HistorySample {
  ts: number;
  cpu: number;
  memory: number;
  swap: number;
  disk: number | null;
  net_rx: number;
  net_tx: number;
  temp: number | null;
  gpus: { util: number | null; mem: number | null; temp: number | null }[];
}

export interface ModelStat {
  model: string;
  turns: number;
  generations: number;
  prompt_tokens: number;
  output_tokens: number;
  tokens_estimated: boolean;
  tokens_per_sec: number | null;
  prompt_tokens_per_sec: number | null;
  avg_ttft_ms: number | null;
  cold_starts: number;
  load_ms: number;
}

/** Mirrors `ai::metrics::AgentMetrics::snapshot`. */
export interface AgentMetrics {
  uptime_s: number;
  turns: number;
  errors: number;
  cancelled: number;
  models: ModelStat[];
  tools: { tool: string; calls: number; ok: number; failed: number; avg_ms: number | null; max_ms: number }[];
  recall: { runs: number; hits: number; empty: number };
  capture: { runs: number; saved: number };
  recent: {
    ts: string;
    model: string;
    duration_ms: number;
    ttft_ms: number | null;
    output_tokens: number;
    tools: number;
    recalled: number;
    outcome: string;
  }[];
}

export interface InstalledModel {
  name: string;
  size: number;
  family: string | null;
  parameter_size: string | null;
  quantization: string | null;
  modified_at: string | null;
  loaded: boolean;
}

export interface LoadedModel {
  name: string;
  size: number;
  size_vram: number;
  gpu_percent: number;
  context_length: number | null;
  expires_at: string | null;
  parameter_size: string | null;
  quantization: string | null;
}

/** Shape of `get_performance`. */
export interface PerformanceData {
  host: DetailedStats;
  gpus: GpuInfo[];
  history: HistorySample[];
  agent: AgentMetrics;
  models: {
    configured: string;
    installed: InstalledModel[] | null;
    loaded: LoadedModel[] | null;
    error: string | null;
  };
  memory_store: {
    embed_model: string | null;
    llm_model: string | null;
    avg_search_ms: number | null;
    searches_24h: number | null;
    vectors: number | null;
    pending: number | null;
    storage_bytes: number | null;
  } | null;
}

export interface Service {
  unit: string;
  scope: 'system' | 'user';
  description: string;
  load: string;
  active: string;
  sub: string;
  enabled: string | null;
}

export interface Container {
  id: string;
  name: string;
  image: string;
  state: string;
  status: string;
  ports: string;
  project: string | null;
  cpu: number | null;
  memory: string | null;
  memory_percent: number | null;
}

export interface DockerStatus {
  available: boolean;
  reason: string | null;
  containers: Container[];
}

export interface CleanupItem {
  id: string;
  label: string;
  description: string;
  bytes: number;
  kind: 'files' | 'command';
  command: string | null;
  partial: boolean;
}

export interface Recommendation {
  id: string;
  severity: 'info' | 'warning' | 'critical';
  area: string;
  title: string;
  detail: string;
  action: { kind: string; label: string; params: Record<string, unknown> } | null;
}

export type Metric =
  | 'cpu' | 'memory' | 'swap' | 'disk' | 'temperature' | 'gpu_util' | 'gpu_memory' | 'gpu_temp'
  | 'process_missing' | 'service_down' | 'container_down' | 'ollama_down' | 'kb_core_down';

export interface Condition {
  metric: Metric;
  op: 'above' | 'below';
  threshold: number;
  sustain_secs: number;
  target?: string | null;
}

export type OpsAction =
  | { kind: 'notify'; title: string; message: string }
  | { kind: 'command'; command: string; cwd?: string | null; approval?: string | null }
  | { kind: 'ai_report'; prompt: string; save_to_memory: boolean; text_me?: boolean }
  | { kind: 'text'; message: string }
  | { kind: 'call'; message: string };

export type Trigger =
  | { kind: 'alert'; alert_id: string }
  | { kind: 'condition'; condition: Condition }
  | { kind: 'process_start'; name: string }
  | { kind: 'process_stop'; name: string }
  | { kind: 'file_change'; path: string }
  | { kind: 'idle'; minutes: number; cpu_below: number };

export interface RunState {
  last_run: string | null;
  run_count: number;
  last_ok: boolean | null;
  last_result: string | null;
}

export interface Alert {
  id: string;
  name: string;
  condition: Condition;
  enabled: boolean;
  notify: boolean;
  phone?: PhoneChannel | null;
  cooldown_secs: number;
  state: { firing: boolean; since: string | null; last_fired: string | null; fire_count: number; last_value: number | null };
}

export interface Automation {
  id: string;
  name: string;
  trigger: Trigger;
  action: OpsAction;
  enabled: boolean;
  cooldown_secs: number;
  run: RunState;
}

export interface ScheduledTask {
  id: string;
  name: string;
  schedule: string;
  action: OpsAction;
  enabled: boolean;
  next_run: string | null;
  run: RunState;
}

export interface Activity {
  ts: string;
  kind: string;
  name: string;
  ok: boolean;
  summary: string;
}

/** Shape of `get_system_control_data`. */
export interface SystemControlData {
  available: boolean;
  alerts: { alert: Alert; description: string; value: number | null; unit: string }[];
  automations: { automation: Automation; trigger: string; action: string; approved: boolean | null }[];
  tasks: { task: ScheduledTask; when: string; action: string; approved: boolean | null }[];
  activity: Activity[];
}

/** `ops://event` payload. */
export interface OpsEvent {
  kind: string;
  name: string;
  ok: boolean;
  level: 'info' | 'warning' | 'critical';
  summary: string;
  detail: string | null;
}

/** Mirrors `ai::agent::UiEvent` (streamed over a Tauri Channel). */
export type UiEvent =
  | { type: 'token'; text: string }
  | { type: 'tool_call'; id: string; name: string; arguments: unknown }
  | { type: 'tool_result'; id: string; name: string; ok: boolean; summary: string }
  | { type: 'recalled'; memories: RecalledMemory[] }
  | { type: 'notice'; message: string }
  | { type: 'error'; message: string }
  | { type: 'done' };

/** Mirrors `ai::agent::RecalledMemory`: one auto-recalled memory under a reply. */
export interface RecalledMemory {
  id: string;
  /** Plain text (untrusted): render as text, never as HTML. */
  preview: string;
  origin: string | null;
}
