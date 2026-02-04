# Hooks System

The A3S Box hooks system provides an extensible mechanism for intercepting and customizing agent behavior at various lifecycle points. Similar to middleware in web frameworks, hooks allow developers to validate, transform, or block operations through the SDK.

## Overview

Hooks enable you to:
- **Validate** tool inputs before execution
- **Transform** tool arguments or results
- **Block** dangerous operations
- **Log** all agent activities
- **Inject** custom behaviors at specific lifecycle points

## Event Types

The hooks system supports six event types that cover the complete agent lifecycle:

| Event Type | Timing | Use Cases |
|------------|--------|-----------|
| `PreToolUse` | Before tool execution | Input validation, security checks, argument modification |
| `PostToolUse` | After tool execution | Result logging, output transformation, cleanup |
| `GenerateStart` | Before LLM generation | Prompt injection, context modification |
| `GenerateEnd` | After LLM generation | Response filtering, token tracking |
| `SessionStart` | When session is created | Initialization, resource allocation |
| `SessionEnd` | When session is destroyed | Cleanup, audit logging |

### Event Payloads

Each event type carries specific payload data:

```typescript
// PreToolUse event
interface PreToolUseEvent {
  session_id: string;
  tool: string;           // Tool name (e.g., "Bash", "Write")
  args: object;           // Tool arguments
  working_directory: string;
  recent_tools: string[]; // Recent tool history for context
}

// PostToolUse event
interface PostToolUseEvent {
  session_id: string;
  tool: string;
  args: object;
  result: {
    success: boolean;
    output: string;
    exit_code?: number;   // For shell commands
    duration_ms: number;
  };
}

// GenerateStart event
interface GenerateStartEvent {
  session_id: string;
  prompt: string;
  system_prompt?: string;
  model_provider: string;
  model_name: string;
  available_tools: string[];
}

// GenerateEnd event
interface GenerateEndEvent {
  session_id: string;
  prompt: string;
  response_text: string;
  tool_calls: ToolCallInfo[];
  usage: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
  duration_ms: number;
}

// SessionStart/End events
interface SessionStartEvent {
  session_id: string;
  system_prompt?: string;
  model_provider: string;
  model_name: string;
}

interface SessionEndEvent {
  session_id: string;
  total_tokens: number;
  total_tool_calls: number;
  duration_ms: number;
}
```

## Hook Actions

When a hook is triggered, it can respond with one of four actions:

| Action | Description | Effect |
|--------|-------------|--------|
| `Continue` | Proceed with execution | Optionally includes modified data |
| `Block` | Stop execution | Returns error with reason |
| `Retry` | Retry after delay | Includes delay in milliseconds |
| `Skip` | Skip remaining hooks | Continue execution without further hooks |

## Matchers

Matchers filter which events trigger a hook. Multiple filters can be combined:

### Tool Matcher
Match events by tool name (exact match):

```typescript
// Only trigger for Bash tool
const matcher = { tool: "Bash" };
```

### Path Pattern Matcher
Match events by file path using glob patterns:

```typescript
// Match all Rust files
const matcher = { path_pattern: "*.rs" };

// Match files in src directory
const matcher = { path_pattern: "src/**/*.ts" };
```

Glob patterns support:
- `*` - Matches any characters (excluding `/`)
- `**` - Matches any path (including `/`)
- `*.ext` - Matches any file ending with `.ext` at any depth

### Command Pattern Matcher
Match Bash commands using regex:

```typescript
// Match dangerous rm commands
const matcher = { command_pattern: "rm\\s+-rf" };

// Match any git command
const matcher = { command_pattern: "^git\\s+" };
```

### Session Matcher
Match events from a specific session:

```typescript
const matcher = { session_id: "session-123" };
```

### Combined Matchers
Combine multiple filters (all must match):

```typescript
const matcher = {
  tool: "Bash",
  command_pattern: "rm",
  session_id: "session-123"
};
```

## Hook Configuration

Configure hook behavior with `HookConfig`:

```typescript
interface HookConfig {
  priority: number;        // Lower = higher priority (default: 100)
  timeout_ms: number;      // Handler timeout (default: 30000)
  async_execution: boolean; // Fire-and-forget mode (default: false)
  max_retries: number;     // Retry attempts (default: 0)
}
```

### Priority

Hooks are executed in priority order (lower values first):

```typescript
// Security check runs first (priority 10)
engine.register({
  id: "security-check",
  event_type: "pre_tool_use",
  config: { priority: 10 }
});

// Logging runs last (priority 1000)
engine.register({
  id: "audit-log",
  event_type: "pre_tool_use",
  config: { priority: 1000 }
});
```

### Async Execution

For fire-and-forget hooks (e.g., logging), enable async mode:

```typescript
engine.register({
  id: "metrics-reporter",
  event_type: "post_tool_use",
  config: { async_execution: true }
});
```

Async hooks always return `Continue` immediately.

## Usage Examples

### Security: Block Dangerous Commands

```typescript
import { HookEngine, Hook, HookMatcher, HookEventType } from "@a3s-lab/box";

const engine = new HookEngine();

// Block rm -rf commands
engine.register(
  Hook.new("block-rm-rf", HookEventType.PreToolUse)
    .with_matcher(HookMatcher.command("rm\\s+-rf"))
    .with_config({ priority: 1 })
);

engine.register_handler("block-rm-rf", {
  handle: (event) => ({
    action: "Block",
    reason: "Recursive force deletion is not allowed"
  })
});
```

### Validation: Restrict File Access

```typescript
// Only allow writing to specific directories
engine.register(
  Hook.new("restrict-writes", HookEventType.PreToolUse)
    .with_matcher(HookMatcher.tool("Write"))
);

engine.register_handler("restrict-writes", {
  handle: (event) => {
    const path = event.payload.args.file_path;
    if (!path.startsWith("/workspace/")) {
      return {
        action: "Block",
        reason: `Write access denied: ${path} is outside workspace`
      };
    }
    return { action: "Continue" };
  }
});
```

### Transformation: Modify Tool Arguments

```typescript
// Add timeout to all Bash commands
engine.register(
  Hook.new("add-timeout", HookEventType.PreToolUse)
    .with_matcher(HookMatcher.tool("Bash"))
);

engine.register_handler("add-timeout", {
  handle: (event) => {
    const args = event.payload.args;
    return {
      action: "Continue",
      modified: {
        ...args,
        timeout: args.timeout || 30000 // Default 30s timeout
      }
    };
  }
});
```

### Logging: Audit Trail

```typescript
// Log all tool executions
engine.register(
  Hook.new("audit-log", HookEventType.PostToolUse)
    .with_config({ async_execution: true, priority: 1000 })
);

engine.register_handler("audit-log", {
  handle: (event) => {
    const { tool, args, result } = event.payload;
    console.log(JSON.stringify({
      timestamp: new Date().toISOString(),
      session: event.payload.session_id,
      tool,
      args,
      success: result.success,
      duration_ms: result.duration_ms
    }));
    return { action: "Continue" };
  }
});
```

### Cost Control: Token Budget

```typescript
// Track and limit token usage
let sessionTokens = new Map<string, number>();

engine.register(
  Hook.new("token-budget", HookEventType.GenerateEnd)
);

engine.register_handler("token-budget", {
  handle: (event) => {
    const { session_id, usage } = event.payload;
    const current = sessionTokens.get(session_id) || 0;
    const updated = current + usage.total_tokens;
    sessionTokens.set(session_id, updated);

    if (updated > 100000) {
      console.warn(`Session ${session_id} exceeded 100k tokens`);
    }
    return { action: "Continue" };
  }
});
```

### Retry Logic: Rate Limit Handling

```typescript
engine.register(
  Hook.new("rate-limit-retry", HookEventType.GenerateStart)
);

engine.register_handler("rate-limit-retry", {
  handle: (event) => {
    // Check if rate limited (example logic)
    if (isRateLimited(event.payload.session_id)) {
      return {
        action: "Retry",
        retry_delay_ms: 5000 // Wait 5 seconds
      };
    }
    return { action: "Continue" };
  }
});
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        SDK (Client)                          │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                    HookEngine                          │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌───────────────┐  │  │
│  │  │   Hooks     │  │  Handlers   │  │   Matchers    │  │  │
│  │  │  Registry   │  │  Registry   │  │   (Filters)   │  │  │
│  │  └─────────────┘  └─────────────┘  └───────────────┘  │  │
│  └───────────────────────────┬───────────────────────────┘  │
│                              │                               │
│                              ▼                               │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                  Event Channel                         │  │
│  │              (tokio::sync::mpsc)                       │  │
│  └───────────────────────────┬───────────────────────────┘  │
└──────────────────────────────┼──────────────────────────────┘
                               │ gRPC over vsock
┌──────────────────────────────┼──────────────────────────────┐
│                              ▼                               │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                    Agent Loop                          │  │
│  │                                                        │  │
│  │   ┌──────────┐    fire(PreToolUse)    ┌──────────┐   │  │
│  │   │   LLM    │ ◄───────────────────► │  Hooks   │   │  │
│  │   │  Client  │    fire(GenerateEnd)   │  Engine  │   │  │
│  │   └──────────┘                        └──────────┘   │  │
│  │        │                                    │         │  │
│  │        ▼                                    ▼         │  │
│  │   ┌──────────┐    fire(PostToolUse)  ┌──────────┐   │  │
│  │   │   Tool   │ ◄───────────────────► │ Handler  │   │  │
│  │   │ Executor │                        │ Executor │   │  │
│  │   └──────────┘                        └──────────┘   │  │
│  └───────────────────────────────────────────────────────┘  │
│                        Guest VM                              │
└──────────────────────────────────────────────────────────────┘
```

## Execution Flow

1. **Event Occurs**: Agent performs an action (tool call, generation, etc.)
2. **Matching**: Engine finds all hooks matching the event
3. **Priority Sort**: Hooks are sorted by priority (ascending)
4. **Sequential Execution**: Each hook handler is called in order
5. **Result Handling**:
   - `Continue`: Move to next hook, apply modifications
   - `Block`: Stop immediately, return error
   - `Retry`: Wait and retry the operation
   - `Skip`: Stop hook processing, continue operation

## Best Practices

### 1. Use Appropriate Priorities

```typescript
// Security hooks: 1-10 (run first)
// Validation hooks: 10-50
// Transformation hooks: 50-100
// Logging hooks: 100-1000 (run last)
```

### 2. Keep Handlers Fast

Handlers block the agent loop. For slow operations:
- Use `async_execution: true` for fire-and-forget
- Set appropriate `timeout_ms`
- Consider `Retry` action for rate limits

### 3. Be Specific with Matchers

```typescript
// Bad: triggers on every tool
const matcher = {};

// Good: specific tool and pattern
const matcher = {
  tool: "Bash",
  command_pattern: "^sudo\\s+"
};
```

### 4. Handle Errors Gracefully

```typescript
engine.register_handler("my-hook", {
  handle: (event) => {
    try {
      // Your logic
      return { action: "Continue" };
    } catch (error) {
      console.error("Hook error:", error);
      return { action: "Continue" }; // Don't block on errors
    }
  }
});
```

### 5. Use Async for Logging

Logging shouldn't slow down the agent:

```typescript
engine.register(
  Hook.new("logger", HookEventType.PostToolUse)
    .with_config({ async_execution: true })
);
```

## API Reference

### HookEngine

```typescript
class HookEngine {
  // Create a new engine
  new(): HookEngine;

  // Register a hook
  register(hook: Hook): void;

  // Unregister a hook by ID
  unregister(hookId: string): Hook | null;

  // Register a handler for a hook
  register_handler(hookId: string, handler: HookHandler): void;

  // Unregister a handler
  unregister_handler(hookId: string): void;

  // Fire an event (called internally by agent)
  fire(event: HookEvent): Promise<HookResult>;

  // Get all registered hooks
  all_hooks(): Hook[];

  // Get hook by ID
  get_hook(id: string): Hook | null;

  // Get hook count
  hook_count(): number;
}
```

### Hook

```typescript
interface Hook {
  id: string;
  event_type: HookEventType;
  matcher?: HookMatcher;
  config: HookConfig;
}

// Builder methods
Hook.new(id: string, eventType: HookEventType): Hook;
hook.with_matcher(matcher: HookMatcher): Hook;
hook.with_config(config: HookConfig): Hook;
```

### HookMatcher

```typescript
interface HookMatcher {
  tool?: string;           // Exact tool name match
  path_pattern?: string;   // Glob pattern for file paths
  command_pattern?: string; // Regex for Bash commands
  session_id?: string;     // Exact session ID match
}

// Factory methods
HookMatcher.new(): HookMatcher;           // Matches all
HookMatcher.tool(name: string): HookMatcher;
HookMatcher.path(pattern: string): HookMatcher;
HookMatcher.command(pattern: string): HookMatcher;
HookMatcher.session(id: string): HookMatcher;

// Builder methods (chainable)
matcher.with_tool(name: string): HookMatcher;
matcher.with_path(pattern: string): HookMatcher;
matcher.with_command(pattern: string): HookMatcher;
matcher.with_session(id: string): HookMatcher;
```

### HookResponse

```typescript
interface HookResponse {
  action: "Continue" | "Block" | "Retry" | "Skip";
  reason?: string;         // For Block action
  modified?: object;       // For Continue with modifications
  retry_delay_ms?: number; // For Retry action
}

// Factory methods
HookResponse.continue_(): HookResponse;
HookResponse.continue_with(modified: object): HookResponse;
HookResponse.block(reason: string): HookResponse;
HookResponse.retry(delayMs: number): HookResponse;
HookResponse.skip(): HookResponse;
```

## Related Documentation

- [Permission System](./configuration-guide.md#permission-system) - Declarative tool permissions
- [Human-in-the-Loop](./code-agent-interface.md#human-in-the-loop) - Interactive confirmation
- [Lane-Based Queue](./lane-based-queue.md) - Priority command scheduling
