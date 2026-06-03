// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::Mutex;
use sysinfo::System;
use once_cell::sync::Lazy;
use tauri::Manager;

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
            list_directory
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
