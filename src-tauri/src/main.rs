// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::process::Command;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize)]
struct SystemStatus {
    cpu: f32,
    memory: f32,
    status: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CommandResponse {
    success: bool,
    output: String,
    error: Option<String>,
}

// System status monitoring
#[tauri::command]
async fn get_system_status() -> Result<SystemStatus, String> {
    // Get CPU usage (simplified - in production use sysinfo crate)
    let cpu = get_cpu_usage();
    let memory = get_memory_usage();
    
    Ok(SystemStatus {
        cpu,
        memory,
        status: "online".to_string(),
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

// File operations handler
async fn handle_file_operation(operation: &str) -> Result<String, String> {
    Ok(format!("File operation: {} (implementation pending)", operation))
}

// Search handler
async fn handle_search(query: &str) -> Result<String, String> {
    Ok(format!("Searching for: {} (implementation pending)", query))
}

// System monitoring
async fn handle_monitoring() -> Result<String, String> {
    let status = get_system_status().await?;
    Ok(format!(
        "System Status:\n• CPU: {:.1}%\n• Memory: {:.1}%\n• Status: {}",
        status.cpu, status.memory, status.status
    ))
}

// LLM integration (placeholder for Ollama/cloud LLM)
async fn process_with_llm(prompt: &str) -> Result<String, String> {
    // This will integrate with Ollama or cloud LLMs
    // For now, return a helpful response
    Ok(format!(
        "🤖 OMNIX AI Response:\n\nI received your request: \"{}\"\n\n\
        I'm ready to help with:\n\
        • System commands (/execute)\n\
        • File operations (/file)\n\
        • Search operations (/search)\n\
        • System monitoring (/monitor)\n\
        • And much more!\n\n\
        Full AI integration coming soon with local LLM support.",
        prompt
    ))
}

// Helper functions for system monitoring
fn get_cpu_usage() -> f32 {
    // Simplified - in production use sysinfo crate
    use std::fs;
    
    #[cfg(target_os = "linux")]
    {
        if let Ok(stat) = fs::read_to_string("/proc/stat") {
            // Parse CPU stats (simplified)
            return 15.5; // Placeholder
        }
    }
    
    // Default fallback
    12.3
}

fn get_memory_usage() -> f32 {
    // Simplified - in production use sysinfo crate
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
            // Parse memory stats (simplified)
            return 45.2; // Placeholder
        }
    }
    
    // Default fallback
    38.7
}

// Read file content
#[tauri::command]
async fn read_file(path: String) -> Result<String, String> {
    use std::fs;
    fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file: {}", e))
}

// Write file content
#[tauri::command]
async fn write_file(path: String, content: String) -> Result<(), String> {
    use std::fs;
    fs::write(&path, content)
        .map_err(|e| format!("Failed to write file: {}", e))
}

// List directory contents
#[tauri::command]
async fn list_directory(path: String) -> Result<Vec<String>, String> {
    use std::fs;
    
    let entries = fs::read_dir(&path)
        .map_err(|e| format!("Failed to read directory: {}", e))?;
    
    let mut files = Vec::new();
    for entry in entries {
        if let Ok(entry) = entry {
            if let Some(name) = entry.file_name().to_str() {
                files.push(name.to_string());
            }
        }
    }
    
    Ok(files)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            get_system_status,
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

// Made with Bob
