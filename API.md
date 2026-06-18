# 📚 OMNIX API Documentation

Complete API reference for OMNIX commands, functions, and integrations.

---

## Table of Contents

- [Command API](#command-api)
- [Voice API](#voice-api)
- [File Operations](#file-operations)
- [System Control](#system-control)
- [Memory & Context](#memory--context)
- [AI Integration](#ai-integration)
- [Plugin System](#plugin-system)
- [Configuration](#configuration)

---

## Command API

### Execute Command

Execute system commands with optional sudo privileges.

**Syntax:**
```
/execute <command> [--sudo] [--confirm]
```

**Examples:**
```bash
# Basic command
/execute ls -la

# With sudo
/execute apt update --sudo

# With confirmation
/execute rm -rf /tmp/cache --confirm

# Chained commands
/execute cd ~/projects && git pull && npm install
```

**Parameters:**
- `command` (required): The command to execute
- `--sudo` (optional): Request elevated privileges
- `--confirm` (optional): Require user confirmation

**Response:**
```json
{
  "success": true,
  "output": "command output here",
  "exitCode": 0,
  "executionTime": 1234
}
```

---

## File Operations

### Read File

Read file contents with optional line range.

**Syntax:**
```
/file read <path> [--lines start:end]
```

**Examples:**
```bash
# Read entire file
/file read ~/Documents/notes.txt

# Read specific lines
/file read app.js --lines 1:50

# Read multiple files
/file read package.json tsconfig.json
```

### Write File

Write content to a file.

**Syntax:**
```
/file write <path> <content> [--append]
```

**Examples:**
```bash
# Create/overwrite file
/file write test.txt "Hello World"

# Append to file
/file write log.txt "New entry" --append

# Write JSON
/file write config.json '{"key": "value"}'
```

### Search Files

Search for patterns in files.

**Syntax:**
```
/file search <pattern> [--path <dir>] [--type <ext>] [--recursive]
```

**Examples:**
```bash
# Search in current directory
/file search "TODO"

# Search in specific path
/file search "import React" --path src/

# Search specific file types
/file search "function" --type js,ts --recursive

# Regex search
/file search "^class \w+" --regex
```

### File Management

**Copy:**
```bash
/file copy <source> <destination>
```

**Move:**
```bash
/file move <source> <destination>
```

**Delete:**
```bash
/file delete <path> [--recursive] [--confirm]
```

**Create Directory:**
```bash
/file mkdir <path> [--parents]
```

---

## System Control

### Process Management

**List Processes:**
```bash
/process list [--sort cpu|memory] [--limit 10]
```

**Kill Process:**
```bash
/process kill <pid|name> [--force]
```

**Start Application:**
```bash
/process start <application> [--args "arguments"]
```

### System Monitoring

**CPU Usage:**
```bash
/monitor cpu [--interval 1000]
```

**Memory Usage:**
```bash
/monitor memory [--detailed]
```

**Disk Usage:**
```bash
/monitor disk [--path /]
```

**Network Activity:**
```bash
/monitor network [--interface eth0]
```

### System Information

**Get System Info:**
```bash
/system info [--detailed]
```

**Response:**
```json
{
  "os": "Linux",
  "version": "Ubuntu 22.04",
  "kernel": "5.15.0",
  "cpu": "Intel Core i7-9700K",
  "memory": {
    "total": "16 GB",
    "used": "8 GB",
    "free": "8 GB"
  },
  "disk": {
    "total": "500 GB",
    "used": "250 GB",
    "free": "250 GB"
  }
}
```

---

## Voice API

### Voice Input

Activate voice recognition.

**Keyboard Shortcut:** `Ctrl+Space`

**API Call:**
```javascript
await invoke('start_voice_input', {
  language: 'en',
  continuous: false
});
```

### Voice Output

Convert text to speech.

**Syntax:**
```
/speak <text> [--voice <name>] [--speed <rate>]
```

**Examples:**
```bash
# Basic TTS
/speak "Hello, how can I help you?"

# Custom voice
/speak "Welcome to OMNIX" --voice en_US-lessac-medium

# Adjust speed
/speak "This is faster" --speed 1.5
```

**API Call:**
```javascript
await invoke('text_to_speech', {
  text: 'Hello World',
  voice: 'en_US-lessac-medium',
  speed: 1.0
});
```

---

## Memory & Context

### Save to Memory

Store information for long-term recall.

**Syntax:**
```
/memory save <key> <value> [--context <name>]
```

**Examples:**
```bash
# Save preference
/memory save coding_style "Use TypeScript with strict mode"

# Save with context
/memory save project_structure "Monorepo with Nx" --context work

# Save complex data
/memory save api_endpoints '{"users": "/api/users", "posts": "/api/posts"}'
```

### Retrieve from Memory

Recall stored information.

**Syntax:**
```
/memory get <key> [--context <name>]
```

**Examples:**
```bash
# Get specific memory
/memory get coding_style

# Get from context
/memory get project_structure --context work

# Search memories
/memory search "TypeScript"
```

### List Memories

**Syntax:**
```
/memory list [--context <name>] [--limit 10]
```

### Clear Memory

**Syntax:**
```
/memory clear [--context <name>] [--confirm]
```

---

## AI Integration

### Chat with AI

Send a message to the AI model.

**Syntax:**
```
/chat <message> [--model <name>] [--temperature <value>]
```

**Examples:**
```bash
# Basic chat
/chat "Explain quantum computing"

# Specific model
/chat "Write a Python function" --model gpt-4

# Adjust creativity
/chat "Generate ideas" --temperature 0.9
```

### Code Generation

**Syntax:**
```
/code generate <description> [--language <lang>]
```

**Examples:**
```bash
# Generate function
/code generate "Binary search algorithm" --language python

# Generate component
/code generate "React button component with TypeScript"

# Generate tests
/code generate "Unit tests for user authentication"
```

### Code Analysis

**Syntax:**
```
/code analyze <file> [--suggestions]
```

**Examples:**
```bash
# Analyze file
/code analyze src/app.ts

# Get improvement suggestions
/code analyze src/utils.js --suggestions

# Security audit
/code analyze . --security --recursive
```

---

## Plugin System

### Install Plugin

**Syntax:**
```
/plugin install <name|url>
```

**Examples:**
```bash
# Install from marketplace
/plugin install github-integration

# Install from URL
/plugin install https://github.com/user/omnix-plugin

# Install local plugin
/plugin install ./my-plugin
```

### List Plugins

**Syntax:**
```
/plugin list [--enabled|--disabled]
```

### Enable/Disable Plugin

**Syntax:**
```
/plugin enable <name>
/plugin disable <name>
```

### Plugin Configuration

**Syntax:**
```
/plugin config <name> <key> <value>
```

---

## Configuration

### Get Configuration

**Syntax:**
```
/config get <key>
```

**Examples:**
```bash
# Get specific config
/config get OLLAMA_MODEL

# Get all configs
/config get --all

# Get by category
/config get --category ai
```

### Set Configuration

**Syntax:**
```
/config set <key> <value>
```

**Examples:**
```bash
# Set model
/config set OLLAMA_MODEL llama2

# Enable feature
/config set ENABLE_AUTONOMOUS_MODE true

# Set API key
/config set OPENAI_API_KEY sk-...
```

### Reset Configuration

**Syntax:**
```
/config reset [<key>] [--confirm]
```

---

## JavaScript/TypeScript API

### Tauri Commands

```typescript
import { invoke } from '@tauri-apps/api/tauri';

// Execute command
const result = await invoke('execute_command', {
  command: 'ls -la',
  sudo: false
});

// Read file
const content = await invoke('read_file', {
  path: '/path/to/file.txt'
});

// Write file
await invoke('write_file', {
  path: '/path/to/file.txt',
  content: 'Hello World'
});

// Search files
const results = await invoke('search_files', {
  pattern: 'TODO',
  path: './src',
  recursive: true
});

// Get system info
const info = await invoke('get_system_info');

// Chat with AI
const response = await invoke('chat', {
  message: 'Hello',
  model: 'gpt-4'
});
```

### Event Listeners

```typescript
import { listen } from '@tauri-apps/api/event';

// Listen for command output
await listen('command-output', (event) => {
  console.log('Output:', event.payload);
});

// Listen for system events
await listen('system-event', (event) => {
  console.log('Event:', event.payload);
});

// Listen for AI responses
await listen('ai-response', (event) => {
  console.log('AI:', event.payload);
});
```

---

## REST API (Future)

OMNIX will support a REST API for remote control in future versions.

**Planned Endpoints:**

```
POST   /api/v1/execute
POST   /api/v1/chat
GET    /api/v1/files
POST   /api/v1/files
GET    /api/v1/system/info
GET    /api/v1/memory
POST   /api/v1/memory
```

---

## Error Handling

All API calls return standardized error responses:

```json
{
  "success": false,
  "error": {
    "code": "PERMISSION_DENIED",
    "message": "Insufficient permissions to execute command",
    "details": "Sudo access required"
  }
}
```

**Error Codes:**
- `PERMISSION_DENIED` - Insufficient permissions
- `FILE_NOT_FOUND` - File or directory not found
- `INVALID_COMMAND` - Command syntax error
- `EXECUTION_FAILED` - Command execution failed
- `TIMEOUT` - Operation timed out
- `NETWORK_ERROR` - Network connection failed
- `API_ERROR` - External API error

---

## Rate Limiting

To prevent abuse and ensure system stability:

- **Command Execution**: 100 commands/minute
- **File Operations**: 1000 operations/minute
- **AI Requests**: 60 requests/minute (cloud APIs)
- **Voice Input**: 10 activations/minute

---

## Best Practices

### Security

1. **Always validate input** before executing commands
2. **Use confirmation prompts** for destructive operations
3. **Log all privileged operations** for audit trail
4. **Rotate API keys** regularly
5. **Never hardcode credentials** in commands

### Performance

1. **Use batch operations** when possible
2. **Limit recursive searches** to specific directories
3. **Cache frequently accessed data**
4. **Use streaming** for large file operations
5. **Monitor resource usage** during intensive tasks

### Error Handling

1. **Always check return values**
2. **Implement retry logic** for network operations
3. **Provide meaningful error messages**
4. **Log errors** for debugging
5. **Gracefully degrade** when services are unavailable

---

## Examples

### Complete Workflow Example

```typescript
// 1. Check system status
const systemInfo = await invoke('get_system_info');
console.log('System:', systemInfo);

// 2. Search for files
const files = await invoke('search_files', {
  pattern: '*.ts',
  path: './src',
  recursive: true
});

// 3. Analyze each file
for (const file of files) {
  const analysis = await invoke('analyze_code', {
    path: file.path
  });
  
  if (analysis.issues.length > 0) {
    console.log(`Issues in ${file.path}:`, analysis.issues);
  }
}

// 4. Generate report
const report = await invoke('generate_report', {
  data: files,
  format: 'markdown'
});

// 5. Save report
await invoke('write_file', {
  path: './reports/analysis.md',
  content: report
});

console.log('Analysis complete!');
```

---

## Support

For API questions and support:

- 📖 [GitHub Wiki](https://github.com/Paulmmoore3416/omnix/wiki)
- 💬 [Discussions](https://github.com/Paulmmoore3416/omnix/discussions)
- 🐛 [Issues](https://github.com/Paulmmoore3416/omnix/issues)

---

**Last Updated:** June 2026  
**API Version:** 1.0.0