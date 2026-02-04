# A3S Box Hooks System Design

This document describes the hooks mechanism for a3s-box-code, allowing developers to intercept and customize agent behavior through the SDK.

## Overview

Hooks are automated triggers that execute at specific lifecycle points during agent sessions. They enable developers to:

- Validate and transform inputs before execution
- Block dangerous operations
- Perform side effects (logging, notifications)
- Run post-action automation (formatting, testing)
- Integrate with external systems

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           SDK (TypeScript/Python)                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  client.registerHook("PreToolUse", async (event) => {           │   │
│  │    if (event.tool === "Bash" && event.args.includes("rm -rf"))  │   │
│  │      return { action: "block", reason: "Dangerous command" };   │   │
│  │    return { action: "continue" };                                │   │
│  │  });                                                             │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┬───────────┘
                                                              │ gRPC
┌─────────────────────────────────────────────────────────────┴───────────┐
│                         a3s-box-code (Guest Agent)                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │ Hook Engine │──│ Event Queue │──│  Executors  │──│   Results   │    │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘    │
│         │                                                               │
│         ▼                                                               │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                      Agent Loop                                  │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐        │   │
│  │  │ Message  │─▶│  Tool    │─▶│ Execute  │─▶│ Response │        │   │
│  │  │ Received │  │ Planning │  │  Tools   │  │ Generate │        │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘        │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
```

## Hook Events

### Lifecycle Events

| Event | Trigger Point | Use Cases |
|-------|---------------|-----------|
| `SessionStart` | When session is created | Environment setup, validation |
| `SessionEnd` | When session is destroyed | Cleanup, final reporting |
| `GenerateStart` | Before LLM generation begins | Input validation, context injection |
| `GenerateEnd` | After LLM generation completes | Response transformation, logging |

### Tool Events

| Event | Trigger Point | Use Cases |
|-------|---------------|-----------|
| `PreToolUse` | Before tool execution | Validation, input modification, blocking |
| `PostToolUse` | After tool execution | Formatting, testing, notifications |
| `ToolError` | When tool execution fails | Error handling, retry logic |

### Permission Events

| Event | Trigger Point | Use Cases |
|-------|---------------|-----------|
| `PermissionRequest` | When tool requires permission | Custom permission logic |
| `PermissionGranted` | After permission is granted | Audit logging |
| `PermissionDenied` | After permission is denied | Alert, escalation |

### Context Events

| Event | Trigger Point | Use Cases |
|-------|---------------|-----------|
| `PreCompact` | Before context compaction | Archive conversation |
| `PostCompact` | After context compaction | Verify important context retained |
| `ContextWarning` | When context usage exceeds threshold | Proactive management |

## Event Payloads

### Base Event Structure

```typescript
interface HookEvent {
  eventType: HookEventType;
  sessionId: string;
  timestamp: number;
  metadata: Record<string, unknown>;
}
```

### Tool Events

```typescript
interface PreToolUseEvent extends HookEvent {
  eventType: "PreToolUse";
  tool: string;           // "Bash", "Write", "Edit", etc.
  args: ToolArgs;         // Tool-specific arguments
  context: {
    workingDirectory: string;
    recentTools: string[];
    tokenUsage: TokenUsage;
  };
}

interface PostToolUseEvent extends HookEvent {
  eventType: "PostToolUse";
  tool: string;
  args: ToolArgs;
  result: {
    success: boolean;
    output: string;
    exitCode?: number;
    duration: number;
  };
}
```

### Generation Events

```typescript
interface GenerateStartEvent extends HookEvent {
  eventType: "GenerateStart";
  prompt: string;
  systemPrompt?: string;
  model: ModelConfig;
  tools: string[];        // Available tools
}

interface GenerateEndEvent extends HookEvent {
  eventType: "GenerateEnd";
  prompt: string;
  response: {
    text: string;
    toolCalls: ToolCall[];
    usage: TokenUsage;
    duration: number;
  };
}
```

## Hook Response Actions

Hooks return a `HookResponse` to control the flow:

```typescript
type HookResponse =
  | { action: "continue" }                           // Proceed normally
  | { action: "continue"; modified: ModifiedData }   // Proceed with modifications
  | { action: "block"; reason: string }              // Stop execution
  | { action: "retry"; delay?: number }              // Retry after delay
  | { action: "skip" };                              // Skip this hook only
```

### Action Behaviors

| Action | Behavior | Exit Code Equivalent |
|--------|----------|---------------------|
| `continue` | Proceed with execution | 0 |
| `continue` + `modified` | Proceed with modified input | 0 + JSON |
| `block` | Stop execution, report error to agent | 2 |
| `retry` | Retry the operation after delay | N/A (new) |
| `skip` | Skip remaining hooks, continue execution | N/A (new) |

## SDK Interface

### TypeScript SDK

```typescript
import { A3sClient, HookEvent, HookResponse } from "@a3s-lab/box";

const client = new A3sClient();

// Register a hook with async handler
client.registerHook("PreToolUse", async (event: PreToolUseEvent): Promise<HookResponse> => {
  // Block dangerous bash commands
  if (event.tool === "Bash") {
    const command = event.args.command as string;
    if (command.includes("rm -rf /") || command.includes(":(){ :|:& };:")) {
      return {
        action: "block",
        reason: "Dangerous command blocked by security policy"
      };
    }
  }
  return { action: "continue" };
});

// Register hook with matcher pattern
client.registerHook("PreToolUse", {
  matcher: { tool: "Write", pathPattern: "*.env" },
  handler: async (event) => {
    // Validate .env file changes
    const content = event.args.content as string;
    if (content.includes("API_KEY=")) {
      return { action: "block", reason: "Cannot write API keys to .env files" };
    }
    return { action: "continue" };
  }
});

// Modify tool inputs
client.registerHook("PreToolUse", {
  matcher: { tool: "Bash" },
  handler: async (event) => {
    // Add timeout to all bash commands
    const command = event.args.command as string;
    return {
      action: "continue",
      modified: {
        args: { ...event.args, timeout: 30000 }
      }
    };
  }
});

// Post-execution hooks
client.registerHook("PostToolUse", {
  matcher: { tool: "Write", pathPattern: "*.rs" },
  handler: async (event) => {
    if (event.result.success) {
      // Run rustfmt after writing Rust files
      await client.executeTool(event.sessionId, "Bash", {
        command: `rustfmt ${event.args.file_path}`
      });
    }
    return { action: "continue" };
  }
});

// Multiple hooks for same event (run in parallel)
client.registerHook("GenerateEnd", async (event) => {
  // Log to external system
  await fetch("https://metrics.example.com/log", {
    method: "POST",
    body: JSON.stringify({ session: event.sessionId, usage: event.response.usage })
  });
  return { action: "continue" };
});

// Unregister hooks
const hookId = client.registerHook("PreToolUse", handler);
client.unregisterHook(hookId);

// List registered hooks
const hooks = client.listHooks();
```

### Python SDK

```python
from a3s_box import A3sClient, HookEvent, HookResponse

client = A3sClient()

# Register hook with decorator
@client.hook("PreToolUse")
async def validate_bash(event: PreToolUseEvent) -> HookResponse:
    if event.tool == "Bash":
        command = event.args.get("command", "")
        if "rm -rf /" in command:
            return HookResponse.block("Dangerous command blocked")
    return HookResponse.continue_()

# Register hook with matcher
@client.hook("PreToolUse", matcher={"tool": "Write", "path_pattern": "*.py"})
async def format_python(event: PreToolUseEvent) -> HookResponse:
    # Will run only for Write operations on .py files
    return HookResponse.continue_()

# Register hook programmatically
async def log_generation(event: GenerateEndEvent) -> HookResponse:
    print(f"Generated {event.response.usage.total_tokens} tokens")
    return HookResponse.continue_()

client.register_hook("GenerateEnd", log_generation)

# Context manager for temporary hooks
async with client.temporary_hook("PreToolUse", my_handler):
    response = await client.generate(session_id, "Do something")
    # Hook active only within this block
```

## gRPC Service Extension

### Proto Definition

```protobuf
// hooks.proto

service HookService {
  // Register a hook handler (SDK calls this)
  rpc RegisterHook(RegisterHookRequest) returns (RegisterHookResponse);

  // Unregister a hook
  rpc UnregisterHook(UnregisterHookRequest) returns (UnregisterHookResponse);

  // List registered hooks
  rpc ListHooks(ListHooksRequest) returns (ListHooksResponse);

  // Stream for receiving hook events (bidirectional)
  rpc HookStream(stream HookResponse) returns (stream HookEvent);
}

message RegisterHookRequest {
  string event_type = 1;
  HookMatcher matcher = 2;
  HookConfig config = 3;
}

message HookMatcher {
  optional string tool = 1;           // Match specific tool
  optional string path_pattern = 2;   // Glob pattern for file paths
  optional string command_pattern = 3; // Regex for bash commands
}

message HookConfig {
  int32 priority = 1;          // Lower = earlier execution
  int32 timeout_ms = 2;        // Hook execution timeout
  bool async = 3;              // Fire-and-forget (no response needed)
}

message HookEvent {
  string hook_id = 1;
  string event_type = 2;
  string session_id = 3;
  int64 timestamp = 4;
  bytes payload = 5;           // JSON-encoded event data
}

message HookResponse {
  string hook_id = 1;
  HookAction action = 2;
  optional string reason = 3;
  optional bytes modified = 4; // JSON-encoded modifications
}

enum HookAction {
  CONTINUE = 0;
  BLOCK = 1;
  RETRY = 2;
  SKIP = 3;
}
```

## Implementation Details

### Hook Engine (Rust)

```rust
// src/code/src/hooks/mod.rs

pub mod engine;
pub mod events;
pub mod matcher;

pub use engine::HookEngine;
pub use events::*;
pub use matcher::HookMatcher;
```

```rust
// src/code/src/hooks/engine.rs

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

pub struct HookEngine {
    hooks: Arc<RwLock<HashMap<String, Vec<RegisteredHook>>>>,
    event_tx: mpsc::Sender<HookEvent>,
    response_rx: mpsc::Receiver<HookResponse>,
}

pub struct RegisteredHook {
    id: String,
    event_type: HookEventType,
    matcher: Option<HookMatcher>,
    config: HookConfig,
    // Handler is on SDK side, we communicate via gRPC stream
}

impl HookEngine {
    /// Fire hooks for an event and collect responses
    pub async fn fire(&self, event: HookEvent) -> Result<HookResult> {
        let hooks = self.hooks.read().await;
        let matching_hooks = hooks
            .get(&event.event_type.to_string())
            .map(|h| h.iter().filter(|h| h.matches(&event)).collect::<Vec<_>>())
            .unwrap_or_default();

        if matching_hooks.is_empty() {
            return Ok(HookResult::Continue(None));
        }

        // Send event to all matching hooks in parallel
        let mut responses = Vec::new();
        for hook in matching_hooks {
            let event_with_hook_id = event.clone().with_hook_id(&hook.id);
            self.event_tx.send(event_with_hook_id).await?;

            // Wait for response with timeout
            match tokio::time::timeout(
                Duration::from_millis(hook.config.timeout_ms as u64),
                self.wait_for_response(&hook.id)
            ).await {
                Ok(Ok(response)) => responses.push(response),
                Ok(Err(e)) => {
                    tracing::warn!("Hook {} failed: {}", hook.id, e);
                }
                Err(_) => {
                    tracing::warn!("Hook {} timed out", hook.id);
                }
            }
        }

        // Aggregate responses (block wins over continue)
        self.aggregate_responses(responses)
    }

    fn aggregate_responses(&self, responses: Vec<HookResponse>) -> Result<HookResult> {
        for response in &responses {
            if response.action == HookAction::Block {
                return Ok(HookResult::Block(response.reason.clone()));
            }
        }

        // Merge modifications if any
        let modifications = responses
            .iter()
            .filter_map(|r| r.modified.as_ref())
            .fold(None, |acc, m| merge_modifications(acc, m));

        Ok(HookResult::Continue(modifications))
    }
}
```

### Integration with Agent Loop

```rust
// src/code/src/agent.rs

impl Agent {
    async fn execute_tool(&self, tool: &str, args: Value) -> Result<ToolOutput> {
        // Fire PreToolUse hooks
        let pre_event = PreToolUseEvent {
            tool: tool.to_string(),
            args: args.clone(),
            context: self.build_context(),
        };

        let hook_result = self.hooks.fire(HookEvent::PreToolUse(pre_event)).await?;

        match hook_result {
            HookResult::Block(reason) => {
                return Err(AgentError::HookBlocked { tool: tool.to_string(), reason });
            }
            HookResult::Continue(Some(modified)) => {
                // Use modified args
                args = modified.args.unwrap_or(args);
            }
            HookResult::Continue(None) => {}
        }

        // Execute the tool
        let start = Instant::now();
        let result = self.tool_registry.execute(tool, &args, &self.ctx).await;
        let duration = start.elapsed();

        // Fire PostToolUse hooks
        let post_event = PostToolUseEvent {
            tool: tool.to_string(),
            args,
            result: ToolResult {
                success: result.is_ok(),
                output: result.as_ref().map(|r| r.output.clone()).unwrap_or_default(),
                exit_code: result.as_ref().ok().and_then(|r| r.exit_code),
                duration: duration.as_millis() as u64,
            },
        };

        // PostToolUse hooks are fire-and-forget (don't block)
        let _ = self.hooks.fire(HookEvent::PostToolUse(post_event)).await;

        result
    }
}
```

## Execution Model

### Hook Execution Flow

```
Event Triggered
      │
      ▼
┌─────────────┐
│ Find Hooks  │  Match by event type + matcher patterns
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Sort by     │  Lower priority number = execute first
│ Priority    │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Execute in  │  All hooks run concurrently
│ Parallel    │  Wait for all with timeout
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Aggregate   │  Block wins > Retry > Continue
│ Responses   │  Merge modifications
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Apply       │  Continue: proceed (with modifications)
│ Result      │  Block: return error to agent
└─────────────┘
```

### Timeout Handling

| Scenario | Behavior |
|----------|----------|
| Hook responds in time | Process response normally |
| Hook times out | Log warning, treat as `continue` |
| All hooks time out | Proceed with original input |
| Network error | Log error, treat as `continue` |

### Error Handling

```typescript
// SDK handles reconnection automatically
client.registerHook("PreToolUse", async (event) => {
  try {
    // Hook logic
    return { action: "continue" };
  } catch (error) {
    // Errors in hooks should not block execution by default
    console.error("Hook error:", error);
    return { action: "continue" };  // Fail-open
  }
});

// Or configure fail-close behavior
client.registerHook("PreToolUse", {
  handler: async (event) => { /* ... */ },
  config: { failBehavior: "block" }  // Block on error
});
```

## Security Considerations

### Hook Isolation

1. **Hooks run in SDK process** (not in guest VM)
   - Cannot directly access guest filesystem
   - Cannot bypass VM isolation

2. **Rate limiting**
   - Maximum hooks per event type: 10
   - Maximum total hooks: 50
   - Hook execution timeout: 5 seconds (default)

3. **Input validation**
   - All hook responses validated before processing
   - Modified data must match expected schema

### Audit Trail

```typescript
// Enable hook audit logging
client.setHookAuditLevel("verbose");

// Events logged:
// - Hook registered/unregistered
// - Hook execution started/completed
// - Hook blocked an operation
// - Hook modified inputs
```

## Configuration

### Default Hook Configuration

```json
{
  "hooks": {
    "enabled": true,
    "maxHooksPerEvent": 10,
    "maxTotalHooks": 50,
    "defaultTimeout": 5000,
    "failBehavior": "continue",
    "auditLevel": "info"
  }
}
```

### Per-Hook Configuration

```typescript
client.registerHook("PreToolUse", {
  handler: async (event) => { /* ... */ },
  config: {
    priority: 10,        // Lower = earlier
    timeout: 3000,       // 3 seconds
    async: false,        // Wait for response
    failBehavior: "block"
  }
});
```

## Examples

### Security Policy Hook

```typescript
// Block execution of dangerous commands
client.registerHook("PreToolUse", {
  matcher: { tool: "Bash" },
  handler: async (event) => {
    const command = event.args.command as string;

    const dangerous = [
      /rm\s+-rf\s+\//,
      /mkfs\./,
      /dd\s+if=.*of=\/dev/,
      />\s*\/dev\/sd/
    ];

    for (const pattern of dangerous) {
      if (pattern.test(command)) {
        return {
          action: "block",
          reason: `Blocked dangerous command matching ${pattern}`
        };
      }
    }

    return { action: "continue" };
  }
});
```

### Auto-Format Hook

```typescript
// Auto-format code after writing
client.registerHook("PostToolUse", {
  matcher: { tool: "Write" },
  handler: async (event) => {
    if (!event.result.success) return { action: "continue" };

    const filePath = event.args.file_path as string;
    const ext = filePath.split('.').pop();

    const formatters: Record<string, string> = {
      'rs': 'rustfmt',
      'py': 'black',
      'ts': 'prettier --write',
      'js': 'prettier --write',
      'go': 'gofmt -w',
    };

    const formatter = formatters[ext];
    if (formatter) {
      await client.executeTool(event.sessionId, "Bash", {
        command: `${formatter} "${filePath}"`,
        timeout: 10000
      });
    }

    return { action: "continue" };
  }
});
```

### Cost Tracking Hook

```typescript
// Track token usage and costs
const usage = { totalTokens: 0, totalCost: 0 };

client.registerHook("GenerateEnd", async (event) => {
  const tokens = event.response.usage.total_tokens;
  const cost = calculateCost(event.model, tokens);

  usage.totalTokens += tokens;
  usage.totalCost += cost;

  // Alert if cost exceeds threshold
  if (usage.totalCost > 10.00) {
    console.warn(`Session cost exceeded $10: $${usage.totalCost.toFixed(2)}`);
  }

  return { action: "continue" };
});
```

### Permission Escalation Hook

```typescript
// Require explicit confirmation for sensitive operations
client.registerHook("PermissionRequest", async (event) => {
  const sensitiveTools = ["Bash", "Write", "Edit"];

  if (sensitiveTools.includes(event.tool)) {
    // Show custom confirmation UI
    const confirmed = await showConfirmationDialog({
      title: `Allow ${event.tool}?`,
      message: `The agent wants to execute: ${JSON.stringify(event.args)}`,
      timeout: 30000
    });

    if (!confirmed) {
      return { action: "block", reason: "User denied permission" };
    }
  }

  return { action: "continue" };
});
```

## Migration from Claude Code Hooks

| Claude Code | A3S Box | Notes |
|-------------|---------|-------|
| `settings.json` config | SDK registration | Programmatic instead of file-based |
| Shell command hooks | SDK function hooks | TypeScript/Python instead of shell |
| stdin/stdout JSON | gRPC streaming | More reliable, typed |
| Exit code 2 = block | `action: "block"` | Explicit action field |
| Exit code 0 + JSON | `action: "continue" + modified` | Same semantics |

## Future Enhancements

1. **Hook Templates**: Pre-built hooks for common patterns
2. **Hook Marketplace**: Share and discover community hooks
3. **Visual Hook Builder**: UI for creating hooks without code
4. **Hook Analytics**: Dashboard for hook execution metrics
5. **Conditional Hooks**: Enable/disable based on context
6. **Hook Chains**: Sequential hook execution with data passing
