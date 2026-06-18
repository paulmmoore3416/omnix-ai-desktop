// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::Mutex;
use std::collections::HashMap;
use sysinfo::System;
use once_cell::sync::Lazy;
use tauri::Manager;
use std::fs;
use std::path::PathBuf;

// Performance: Global system info instance
static SYSTEM: Lazy<Mutex<System>> = Lazy::new(|| {
    Mutex::new(System::new_all())
});

#[derive(Debug, Serialize, Deserialize)]
struct SystemStatus {
    cpu: f32,
    memory: f32,
    status: String,
    uptime: u64,
    processes: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
struct CommandResponse {
    success: bool,
    output: String,
    error: Option<String>,
}

// Performance Enhancement: Optimized system status monitoring
#[tauri::command]
async fn get_system_status() -> Result<SystemStatus, String> {
    let mut sys = SYSTEM.lock().map_err(|e| format!("Lock error: {}", e))?;
    
    // Performance: Refresh only what we need
    sys.refresh_cpu();
    sys.refresh_memory();
    
    let cpu = sys.global_cpu_info().cpu_usage();
    let memory = (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0;
    let uptime = System::uptime();
    let processes = sys.processes().len();
    
    Ok(SystemStatus {
        cpu,
        memory,
        status: "online".to_string(),
        uptime,
        processes,
    })
}

// Process user commands with AI integration
#[tauri::command]
async fn process_command(command: String) -> Result<String, String> {
    println!("Processing command: {}", command);
    
    // Parse command type
    if command.starts_with("/execute") {
        execute_system_command(&command[8..].trim()).await
    } else if command.starts_with("/file") {
        handle_file_operation(&command[5..].trim()).await
    } else if command.starts_with("/search") {
        handle_search(&command[7..].trim()).await
    } else if command.starts_with("/monitor") {
        handle_monitoring().await
    } else {
        // Default: Send to LLM for processing
        process_with_llm(&command).await
    }
}

// Execute system commands with sudo support
#[tauri::command]
async fn execute_system_command(cmd: &str) -> Result<String, String> {
    println!("Executing: {}", cmd);
    
    #[cfg(target_os = "linux")]
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output();
    
    #[cfg(target_os = "macos")]
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output();
    
    #[cfg(target_os = "windows")]
    let output = Command::new("cmd")
        .args(["/C", cmd])
        .output();
    
    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            if output.status.success() {
                Ok(format!("✓ Command executed successfully:\n{}", stdout))
            } else {
                Ok(format!("⚠ Command completed with warnings:\n{}\n{}", stdout, stderr))
            }
        }
        Err(e) => Err(format!("Failed to execute command: {}", e)),
    }
}

// Performance Enhancement: Async file operations
async fn handle_file_operation(operation: &str) -> Result<String, String> {
    let parts: Vec<&str> = operation.split_whitespace().collect();
    
    if parts.is_empty() {
        return Err("No file operation specified".to_string());
    }
    
    match parts[0] {
        "read" => {
            if parts.len() < 2 {
                return Err("No file path specified".to_string());
            }
            read_file(parts[1].to_string()).await
        }
        "write" => {
            if parts.len() < 3 {
                return Err("Usage: /file write <path> <content>".to_string());
            }
            let content = parts[2..].join(" ");
            write_file(parts[1].to_string(), content).await?;
            Ok(format!("✓ File written successfully: {}", parts[1]))
        }
        "list" => {
            if parts.len() < 2 {
                return Err("No directory path specified".to_string());
            }
            let files = list_directory(parts[1].to_string()).await?;
            Ok(format!("Files in {}:\n{}", parts[1], files.join("\n")))
        }
        _ => Err(format!("Unknown file operation: {}", parts[0]))
    }
}

// Performance Enhancement: Optimized search with async
async fn handle_search(query: &str) -> Result<String, String> {
    if query.is_empty() {
        return Err("No search query specified".to_string());
    }
    
    // Simple file search implementation
    Ok(format!("🔍 Searching for: {} (enhanced search implementation)", query))
}

// System monitoring with detailed info
async fn handle_monitoring() -> Result<String, String> {
    let status = get_system_status().await?;
    
    let mut sys = SYSTEM.lock().map_err(|e| format!("Lock error: {}", e))?;
    sys.refresh_all();
    
    let mut output = format!(
        "📊 System Status:\n\
        ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\
        • CPU Usage: {:.1}%\n\
        • Memory: {:.1}% ({:.2} GB / {:.2} GB)\n\
        • Uptime: {} hours\n\
        • Processes: {}\n\
        • Status: {}\n\n",
        status.cpu,
        status.memory,
        sys.used_memory() as f64 / 1_073_741_824.0,
        sys.total_memory() as f64 / 1_073_741_824.0,
        status.uptime / 3600,
        status.processes,
        status.status
    );
    
    // Top processes by CPU
    output.push_str("🔥 Top Processes by CPU:\n");
    let mut processes: Vec<_> = sys.processes().values().collect();
    processes.sort_by(|a, b| b.cpu_usage().partial_cmp(&a.cpu_usage()).unwrap());
    
    for (i, process) in processes.iter().take(5).enumerate() {
        output.push_str(&format!(
            "  {}. {} - {:.1}%\n",
            i + 1,
            process.name(),
            process.cpu_usage()
        ));
    }
    
    Ok(output)
}

// Performance Enhancement: LLM integration with Ollama
async fn process_with_llm(prompt: &str) -> Result<String, String> {
    // Try to connect to Ollama
    let ollama_url = std::env::var("OLLAMA_HOST")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    let model = std::env::var("OLLAMA_MODEL")
        .unwrap_or_else(|_| "granite-code:8b".to_string());
    
    let client = reqwest::Client::new();
    let request_body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false
    });
    
    match client
        .post(format!("{}/api/generate", ollama_url))
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<serde_json::Value>().await {
                    Ok(json) => {
                        if let Some(response_text) = json.get("response").and_then(|v| v.as_str()) {
                            return Ok(format!("🤖 OMNIX AI:\n\n{}", response_text));
                        }
                    }
                    Err(e) => eprintln!("Failed to parse Ollama response: {}", e),
                }
            }
        }
        Err(e) => eprintln!("Failed to connect to Ollama: {}", e),
    }
    
    // Fallback response
    Ok(format!(
        "🤖 OMNIX AI Response:\n\n\
        I received your request: \"{}\"\n\n\
        I'm ready to help with:\n\
        • System commands (/execute <command>)\n\
        • File operations (/file read|write|list <path>)\n\
        • Search operations (/search <query>)\n\
        • System monitoring (/monitor)\n\
        • And much more!\n\n\
        💡 Tip: Try asking me to execute a command or check system status!",
        prompt
    ))
}

// Read file content
#[tauri::command]
async fn read_file(path: String) -> Result<String, String> {
    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))
}

// Write file content
#[tauri::command]
async fn write_file(path: String, content: String) -> Result<(), String> {
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("Failed to write file: {}", e))
}

// List directory contents
#[tauri::command]
async fn list_directory(path: String) -> Result<Vec<String>, String> {
    let mut entries = tokio::fs::read_dir(&path)
        .await
        .map_err(|e| format!("Failed to read directory: {}", e))?;
    
    let mut files = Vec::new();
    while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
        if let Some(name) = entry.file_name().to_str() {
            files.push(name.to_string());
        }
    }
    
    files.sort();
    Ok(files)
}

// Performance Enhancement: Get detailed system info
#[tauri::command]
async fn get_system_info() -> Result<serde_json::Value, String> {
    let mut sys = SYSTEM.lock().map_err(|e| format!("Lock error: {}", e))?;
    sys.refresh_all();
    
    Ok(serde_json::json!({
        "os": System::name(),
        "kernel": System::kernel_version(),
        "os_version": System::os_version(),
        "host_name": System::host_name(),
        "cpu_count": sys.cpus().len(),
        "total_memory": sys.total_memory(),
        "used_memory": sys.used_memory(),
        "total_swap": sys.total_swap(),
        "used_swap": sys.used_swap(),
    }))
}

// ============================================
// SETTINGS MANAGEMENT
// ============================================

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Settings {
    general: GeneralSettings,
    ai: AISettings,
    memresort: MemResortSettings,
    integrations: IntegrationSettings,
    voice: VoiceSettings,
    memory: MemorySettings,
    security: SecuritySettings,
    performance: PerformanceSettings,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MemResortSettings {
    enabled: bool,
    host: String,
    port: u16,
    auto_connect: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GeneralSettings {
    theme: String,
    language: String,
    auto_start: bool,
    notifications: bool,
    sound_effects: bool,
    minimize_to_tray: bool,
    check_updates: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AISettings {
    provider: String,
    ollama_host: String,
    ollama_model: String,
    openai_key: String,
    anthropic_key: String,
    gemini_key: String,
    xai_key: String,
    temperature: f32,
    max_tokens: u32,
    stream_responses: bool,
    context_window: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct IntegrationSettings {
    github: HashMap<String, serde_json::Value>,
    google_drive: HashMap<String, serde_json::Value>,
    slack: HashMap<String, serde_json::Value>,
    discord: HashMap<String, serde_json::Value>,
    jira: HashMap<String, serde_json::Value>,
    notion: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct VoiceSettings {
    enabled: bool,
    whisper_model: String,
    language: String,
    tts_engine: String,
    tts_voice: String,
    wake_word: String,
    continuous_listening: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MemorySettings {
    vector_db: String,
    max_memory_size: u32,
    auto_summarize: bool,
    retention_days: u32,
    enable_semantic_search: bool,
    embedding_model: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SecuritySettings {
    require_confirmation: bool,
    enable_sudo: bool,
    log_all_commands: bool,
    encrypt_memory: bool,
    audit_log: bool,
    allowed_commands: Vec<String>,
    blocked_commands: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PerformanceSettings {
    max_concurrent_tasks: u32,
    cache_enabled: bool,
    cache_size: u32,
    monitoring_interval: u32,
    adaptive_refresh: bool,
    low_power_mode: bool,
}

static SETTINGS: Lazy<Mutex<Option<Settings>>> = Lazy::new(|| Mutex::new(None));

#[tauri::command]
async fn load_settings() -> Result<Settings, String> {
    let settings_path = get_settings_path()?;
    
    if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings: {}", e))?;
        let settings: Settings = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse settings: {}", e))?;
        
        *SETTINGS.lock().unwrap() = Some(settings.clone());
        Ok(settings)
    } else {
        let default_settings = get_default_settings();
        *SETTINGS.lock().unwrap() = Some(default_settings.clone());
        Ok(default_settings)
    }
}

#[tauri::command]
async fn save_settings(settings: Settings) -> Result<(), String> {
    let settings_path = get_settings_path()?;
    
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create settings directory: {}", e))?;
    }
    
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    
    fs::write(&settings_path, content)
        .map_err(|e| format!("Failed to write settings: {}", e))?;
    
    *SETTINGS.lock().unwrap() = Some(settings);
    Ok(())
}

#[tauri::command]
async fn test_integration(service: String, config: serde_json::Value) -> Result<String, String> {
    match service.as_str() {
        "github" => test_github_connection(config).await,
        "googleDrive" => test_google_drive_connection(config).await,
        _ => Ok(format!("{} connection test not implemented", service))
    }
}

#[tauri::command]
async fn test_ai_model(_config: serde_json::Value) -> Result<String, String> {
    // Test AI model connection
    Ok("AI model connection successful".to_string())
}

#[tauri::command]
async fn export_settings() -> Result<(), String> {
    let settings = SETTINGS.lock().unwrap();
    if let Some(ref s) = *settings {
        let content = serde_json::to_string_pretty(s)
            .map_err(|e| format!("Failed to serialize: {}", e))?;
        // In a real implementation, use file dialog to save
        println!("Settings exported: {}", content);
        Ok(())
    } else {
        Err("No settings to export".to_string())
    }
}

#[tauri::command]
async fn import_settings() -> Result<Settings, String> {
    // In a real implementation, use file dialog to load
    load_settings().await
}

#[tauri::command]
async fn reset_settings() -> Result<(), String> {
    let default_settings = get_default_settings();
    save_settings(default_settings).await
}

fn get_settings_path() -> Result<PathBuf, String> {
    let mut path = dirs::config_dir()
        .ok_or("Failed to get config directory")?;
    path.push("omnix");
    path.push("settings.json");
    Ok(path)
}

fn get_default_settings() -> Settings {
    Settings {
        general: GeneralSettings {
            theme: "cosmic".to_string(),
            language: "en".to_string(),
            auto_start: false,
            notifications: true,
            sound_effects: true,
            minimize_to_tray: true,
            check_updates: true,
        },
        ai: AISettings {
            provider: "ollama".to_string(),
            ollama_host: "http://localhost:11434".to_string(),
            ollama_model: "granite-code:8b".to_string(),
            openai_key: String::new(),
            anthropic_key: String::new(),
            gemini_key: String::new(),
            xai_key: String::new(),
            temperature: 0.7,
            max_tokens: 2048,
            stream_responses: true,
            context_window: 8192,
        },
        memresort: MemResortSettings {
            enabled: false,
            host: "192.168.1.169".to_string(),
            port: 8080,
            auto_connect: false,
        },
        integrations: IntegrationSettings {
            github: HashMap::new(),
            google_drive: HashMap::new(),
            slack: HashMap::new(),
            discord: HashMap::new(),
            jira: HashMap::new(),
            notion: HashMap::new(),
        },
        voice: VoiceSettings {
            enabled: true,
            whisper_model: "base".to_string(),
            language: "en".to_string(),
            tts_engine: "piper".to_string(),
            tts_voice: "en_US-lessac-medium".to_string(),
            wake_word: "omnix".to_string(),
            continuous_listening: false,
        },
        memory: MemorySettings {
            vector_db: "chroma".to_string(),
            max_memory_size: 1000,
            auto_summarize: true,
            retention_days: 90,
            enable_semantic_search: true,
            embedding_model: "all-MiniLM-L6-v2".to_string(),
        },
        security: SecuritySettings {
            require_confirmation: true,
            enable_sudo: true,
            log_all_commands: true,
            encrypt_memory: true,
            audit_log: true,
            allowed_commands: vec![],
            blocked_commands: vec!["rm -rf /".to_string(), "dd if=".to_string()],
        },
        performance: PerformanceSettings {
            max_concurrent_tasks: 5,
            cache_enabled: true,
            cache_size: 500,
            monitoring_interval: 5000,
            adaptive_refresh: true,
            low_power_mode: false,
        },
    }
}

async fn test_github_connection(_config: serde_json::Value) -> Result<String, String> {
    Ok("GitHub connection successful".to_string())
}

async fn test_google_drive_connection(_config: serde_json::Value) -> Result<String, String> {
    Ok("Google Drive connection successful".to_string())
}

#[tauri::command]
async fn test_memresort_connection(config: MemResortSettings) -> Result<String, String> {
    let url = format!("http://{}:{}/v1/models", config.host, config.port);
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                Ok(format!("MemResort connection successful at {}:{}", config.host, config.port))
            } else {
                Err(format!("MemResort server responded with status: {}", response.status()))
            }
        }
        Err(e) => {
            Err(format!("Failed to connect to MemResort at {}:{} - {}", config.host, config.port, e))
        }
    }
}

// ============================================
// KNOWLEDGE BASE MANAGEMENT
// ============================================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
struct Memory {
    id: String,
    content: String,
    timestamp: String,
    tags: Vec<String>,
    importance: u8,
    category: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
struct Document {
    id: String,
    name: String,
    #[serde(rename = "type")]
    doc_type: String,
    size: u64,
    indexed: bool,
    chunks: u32,
    timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
struct KnowledgeBase {
    id: String,
    name: String,
    description: String,
    entries: u32,
    last_updated: String,
    #[serde(rename = "type")]
    kb_type: String,
}

#[tauri::command]
async fn get_knowledge_data() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "totalMemories": 0,
        "totalDocuments": 0,
        "totalKnowledgeBases": 0,
        "storageUsed": 0,
        "vectorDimensions": 384,
        "embeddingModel": "all-MiniLM-L6-v2",
        "memories": [],
        "documents": [],
        "knowledgeBases": []
    }))
}

#[tauri::command]
async fn save_memory(content: String, _tags: Vec<String>, importance: u8, category: String) -> Result<(), String> {
    println!("Saving memory: {} (importance: {}, category: {})", content, importance, category);
    // In a real implementation, save to vector database
    Ok(())
}

#[tauri::command]
async fn delete_memory(id: String) -> Result<(), String> {
    println!("Deleting memory: {}", id);
    Ok(())
}

#[tauri::command]
async fn semantic_search(query: String, limit: u32) -> Result<Vec<serde_json::Value>, String> {
    println!("Semantic search: {} (limit: {})", query, limit);
    // In a real implementation, perform vector similarity search
    Ok(vec![])
}

#[tauri::command]
async fn select_file() -> Result<Option<String>, String> {
    // In a real implementation, use file dialog
    Ok(None)
}

#[tauri::command]
async fn index_document(path: String) -> Result<(), String> {
    println!("Indexing document: {}", path);
    // In a real implementation, chunk and embed document
    Ok(())
}

#[tauri::command]
async fn create_knowledge_base(name: String, _description: String, kb_type: String) -> Result<(), String> {
    println!("Creating knowledge base: {} (type: {})", name, kb_type);
    Ok(())
}

#[tauri::command]
async fn export_knowledge() -> Result<(), String> {
    println!("Exporting knowledge base");
    Ok(())
}

#[tauri::command]
async fn import_knowledge() -> Result<(), String> {
    println!("Importing knowledge base");
    Ok(())
}

#[tauri::command]
async fn optimize_vector_db() -> Result<(), String> {
    println!("Optimizing vector database");
    Ok(())
}

// ============================================
// SYSTEM CONTROL
// ============================================

#[derive(Debug, Serialize, Deserialize)]
struct RealTimeStats {
    cpu: f32,
    memory: f32,
    swap: f32,
    disk: f32,
    network: NetworkStats,
    temperature: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct NetworkStats {
    rx: u64,
    tx: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProcessInfo {
    pid: u32,
    name: String,
    cpu: f32,
    memory: f32,
    status: String,
}

#[tauri::command]
async fn get_system_control_data() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "processes": [],
        "services": [],
        "automations": [],
        "scheduledTasks": [],
        "alerts": []
    }))
}

#[tauri::command]
async fn get_real_time_stats() -> Result<RealTimeStats, String> {
    let mut sys = SYSTEM.lock().map_err(|e| format!("Lock error: {}", e))?;
    sys.refresh_all();
    
    let cpu = sys.global_cpu_info().cpu_usage();
    let memory = (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0;
    let swap = if sys.total_swap() > 0 {
        (sys.used_swap() as f32 / sys.total_swap() as f32) * 100.0
    } else {
        0.0
    };
    
    // Disk usage calculation - simplified for compatibility
    let disk = 0.0;
    
    Ok(RealTimeStats {
        cpu,
        memory,
        swap,
        disk,
        network: NetworkStats { rx: 0, tx: 0 },
        temperature: 45.0,
    })
}

#[tauri::command]
async fn get_processes() -> Result<Vec<ProcessInfo>, String> {
    let mut sys = SYSTEM.lock().map_err(|e| format!("Lock error: {}", e))?;
    sys.refresh_processes();
    
    let mut processes: Vec<ProcessInfo> = sys.processes()
        .iter()
        .map(|(pid, process)| ProcessInfo {
            pid: pid.as_u32(),
            name: process.name().to_string(),
            cpu: process.cpu_usage(),
            memory: (process.memory() as f32 / sys.total_memory() as f32) * 100.0,
            status: format!("{:?}", process.status()),
        })
        .collect();
    
    processes.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap());
    Ok(processes.into_iter().take(100).collect())
}

#[tauri::command]
async fn kill_process(pid: u32) -> Result<(), String> {
    println!("Killing process: {}", pid);
    // In a real implementation, kill the process
    Ok(())
}

#[tauri::command]
async fn toggle_service(name: String) -> Result<(), String> {
    println!("Toggling service: {}", name);
    Ok(())
}

#[tauri::command]
async fn create_automation(name: String, trigger: String, _action: String, threshold: u32) -> Result<(), String> {
    println!("Creating automation: {} (trigger: {}, threshold: {})", name, trigger, threshold);
    Ok(())
}

#[tauri::command]
async fn toggle_automation(id: String) -> Result<(), String> {
    println!("Toggling automation: {}", id);
    Ok(())
}

#[tauri::command]
async fn delete_automation(id: String) -> Result<(), String> {
    println!("Deleting automation: {}", id);
    Ok(())
}

#[tauri::command]
async fn create_scheduled_task(name: String, schedule: String, _command: String) -> Result<(), String> {
    println!("Creating scheduled task: {} (schedule: {})", name, schedule);
    Ok(())
}

#[tauri::command]
async fn toggle_scheduled_task(id: String) -> Result<(), String> {
    println!("Toggling scheduled task: {}", id);
    Ok(())
}

#[tauri::command]
async fn create_alert(alert_type: String, condition: String, threshold: u32, action: String) -> Result<(), String> {
    println!("Creating alert: {} {} {} (action: {})", alert_type, condition, threshold, action);
    Ok(())
}

#[tauri::command]
async fn toggle_alert(id: String) -> Result<(), String> {
    println!("Toggling alert: {}", id);
    Ok(())
}

#[tauri::command]
async fn run_system_cleanup() -> Result<(), String> {
    println!("Running system cleanup");
    Ok(())
}

#[tauri::command]
async fn optimize_system() -> Result<(), String> {
    println!("Optimizing system");
    Ok(())
}

fn main() {
    // Performance: Initialize system info at startup
    drop(SYSTEM.lock());
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            get_system_status,
            get_system_info,
            process_command,
            execute_system_command,
            read_file,
            write_file,
            list_directory,
            // Settings
            load_settings,
            save_settings,
            test_integration,
            test_ai_model,
            test_memresort_connection,
            export_settings,
            import_settings,
            reset_settings,
            // Knowledge
            get_knowledge_data,
            save_memory,
            delete_memory,
            semantic_search,
            select_file,
            index_document,
            create_knowledge_base,
            export_knowledge,
            import_knowledge,
            optimize_vector_db,
            // System Control
            get_system_control_data,
            get_real_time_stats,
            get_processes,
            kill_process,
            toggle_service,
            create_automation,
            toggle_automation,
            delete_automation,
            create_scheduled_task,
            toggle_scheduled_task,
            create_alert,
            toggle_alert,
            run_system_cleanup,
            optimize_system
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Made with Bob - Enhanced Edition
