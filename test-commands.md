# OMNIX Test Commands

## Test 1: System Monitoring
Command: `/monitor`
Expected: Display CPU, Memory, Uptime, Processes, and Top 5 processes

## Test 2: Execute Command
Command: `/execute echo "Hello OMNIX!"`
Expected: Execute echo command and return output

## Test 3: File Operations - List
Command: `/file list .`
Expected: List files in current directory

## Test 4: Search Query
Command: `/search test`
Expected: Search functionality response

## Test 5: General AI Query
Command: `What can you help me with?`
Expected: AI response with capabilities

## Test 6: System Status Check
Command: `Tell me about my system`
Expected: AI processes and provides system information

## Avatar Emotion Tests:
- Idle state: Default when no activity
- Thinking: When processing queries with "?"
- Working: When executing /execute commands
- Success: After successful command completion
- Error: When command fails
- Listening: When voice input is activated
- Processing: During AI response generation

## UI Feature Tests:
- Quick action buttons (System Status, List Files, Help)
- Command suggestions (type "/" to see)
- Notification system (appears on command execution)
- Navigation between views (Home, Commands, History, etc.)
- CPU/Memory progress bars in sidebar
- Message history with timestamps