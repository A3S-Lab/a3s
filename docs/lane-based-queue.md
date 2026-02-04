# Lane-Based Command Queue

## Overview

The **Lane-Based Command Queue** is a priority-aware, concurrency-controlled task scheduling system designed specifically for AI coding agents. Unlike traditional single-queue systems, it organizes commands into separate "lanes" with different priorities and concurrency limits, enabling fine-grained control over resource allocation and ensuring critical operations are never starved by long-running tasks.

## Why Lane-Based Queuing?

### The Problem with Simple Queues

A naive approach to handling coding agent requests might use a single FIFO queue:

```
Request 1 (generate code)     →  [Queue]  →  Execute
Request 2 (cancel request)    →  [Queue]  →  Wait...
Request 3 (health check)      →  [Queue]  →  Wait...
```

This creates serious problems:

1. **Priority Inversion**: A simple health check must wait behind a 30-second code generation
2. **Head-of-Line Blocking**: Control commands (cancel, clear) blocked by prompt execution
3. **Resource Starvation**: System operations compete with user requests
4. **No Concurrency Control**: Either everything runs in parallel (resource exhaustion) or sequentially (underutilization)

### The Lane-Based Solution

Lane-based queuing solves these problems by partitioning commands into isolated lanes:

```
                    ┌─────────────────────────────────────────┐
                    │           Command Queue                  │
                    │                                         │
 Health Check  ───► │  [System Lane]     Priority: 0  Max: 5  │ ──► Execute immediately
                    │                                         │
 Cancel        ───► │  [Control Lane]    Priority: 1  Max: 3  │ ──► Execute quickly
                    │                                         │
 Get Context   ───► │  [Query Lane]      Priority: 2  Max: 10 │ ──► Execute soon
                    │                                         │
 Create Session ──► │  [Session Lane]    Priority: 3  Max: 5  │ ──► Execute normally
                    │                                         │
 Activate Skill ──► │  [Skill Lane]      Priority: 4  Max: 3  │ ──► Execute normally
                    │                                         │
 Generate Code ───► │  [Prompt Lane]     Priority: 5  Max: 2  │ ──► Execute when available
                    │                                         │
                    └─────────────────────────────────────────┘
```

## Core Concepts

### Lane

A **Lane** is an isolated command channel with its own:

- **Identity**: Unique identifier (e.g., "system", "prompt", "skill:web-search")
- **Priority**: Lower number = higher priority (0 is highest)
- **Concurrency Limits**: min_concurrency (reserved slots), max_concurrency (ceiling)
- **Pending Queue**: FIFO queue of waiting commands
- **Active Count**: Number of currently executing commands

```rust
pub struct Lane {
    id: LaneId,
    state: Arc<Mutex<LaneState>>,
}

struct LaneState {
    config: LaneConfig,           // min/max concurrency
    priority: Priority,           // 0-255, lower = higher
    pending: VecDeque<Command>,   // waiting commands
    active: usize,                // running commands
    semaphore: Arc<Semaphore>,    // concurrency control
}
```

### Priority

Priority determines scheduling order when multiple lanes have pending commands:

| Priority | Lane | Purpose |
|----------|------|---------|
| 0 | System | Health checks, diagnostics, internal operations |
| 1 | Control | Cancel, clear, compact — user control commands |
| 2 | Query | Get context, get history — read-only queries |
| 3 | Session | Create/destroy session — lifecycle management |
| 4 | Skill | Skill activation/deactivation |
| 5 | Prompt | Generate, stream — LLM inference (expensive) |

The scheduler always processes the highest-priority lane with pending commands first:

```rust
async fn schedule_next(&self) {
    // Sort lanes by priority (lower number = higher priority)
    lane_priorities.sort_by_key(|(priority, _)| *priority);

    // Execute from highest-priority lane with capacity
    for (_, lane) in lane_priorities {
        if let Some(command) = lane.try_dequeue().await {
            tokio::spawn(async move {
                let result = command.execute().await;
                // ... handle result
            });
            break;
        }
    }
}
```

### Concurrency Control

Each lane has independent concurrency limits:

```rust
pub struct LaneConfig {
    /// Minimum concurrency (reserved slots)
    pub min_concurrency: usize,

    /// Maximum concurrency (ceiling)
    pub max_concurrency: usize,
}
```

- **min_concurrency**: Reserved capacity — ensures this lane can always make progress
- **max_concurrency**: Hard limit — prevents resource exhaustion

Example configurations:

| Lane | Min | Max | Rationale |
|------|-----|-----|-----------|
| System | 1 | 5 | Always available, lightweight operations |
| Control | 1 | 3 | Must respond to cancel/clear quickly |
| Query | 1 | 10 | Read-only, can parallelize heavily |
| Session | 1 | 5 | Moderate — session creation is quick |
| Skill | 1 | 3 | Tool downloads may be slow |
| Prompt | 1 | 2 | Expensive — LLM inference is resource-intensive |

## Default Lanes

### System Lane (Priority 0)

**Purpose**: Internal system operations that must never be blocked.

**Commands**:
- Health checks
- Diagnostic queries
- Internal state inspection

**Configuration**: `max_concurrency: 5`

**Rationale**: Health checks from Kubernetes liveness probes must respond within seconds. If blocked behind a 60-second code generation, the pod gets killed.

### Control Lane (Priority 1)

**Purpose**: User control commands that modify system state.

**Commands**:
- `cancel` — Stop ongoing generation
- `clear` — Clear conversation history
- `compact` — Compress context window

**Configuration**: `max_concurrency: 3`

**Rationale**: When a user clicks "Cancel", they expect immediate response. The cancel command must not wait behind the very request it's trying to cancel.

### Query Lane (Priority 2)

**Purpose**: Read-only queries that don't modify state.

**Commands**:
- `getContextUsage` — Token counts and context stats
- `getHistory` — Conversation history
- `getCapabilities` — Available tools and features

**Configuration**: `max_concurrency: 10`

**Rationale**: Queries are fast and safe to parallelize. UI components frequently poll for context usage.

### Session Lane (Priority 3)

**Purpose**: Session lifecycle management.

**Commands**:
- `createSession` — Initialize new conversation
- `destroySession` — Clean up resources
- `configure` — Update session settings

**Configuration**: `max_concurrency: 5`

**Rationale**: Session operations are relatively quick but involve state changes. Moderate parallelism.

### Skill Lane (Priority 4)

**Purpose**: Skill activation and tool management.

**Commands**:
- `activateSkill` — Download tools, register with agent
- `deactivateSkill` — Unregister skill

**Configuration**: `max_concurrency: 3`

**Rationale**: Skill activation may involve downloading binaries, which is I/O bound. Limited parallelism prevents network saturation.

### Prompt Lane (Priority 5)

**Purpose**: LLM inference — the most expensive operation.

**Commands**:
- `generate` — Non-streaming text generation
- `stream` — Streaming text generation
- `generateObject` — Structured output generation
- `streamObject` — Streaming structured output

**Configuration**: `max_concurrency: 2`

**Rationale**: LLM inference consumes significant memory and compute. Limiting to 2 concurrent requests prevents resource exhaustion while allowing pipeline parallelism.

## Dynamic Skill Lanes

Skills can define custom lanes for specialized concurrency control:

```yaml
# SKILL.md frontmatter
---
name: web-search
description: Search the web for information
lane: skill:web-search  # Custom lane
tools:
  - name: search
    url: https://example.com/tools/search
---
```

When a skill specifies a custom lane, the queue manager creates a new lane with default skill priority. This enables:

1. **Isolation**: Heavy skills don't block lightweight ones
2. **Rate Limiting**: Per-skill concurrency limits
3. **Monitoring**: Per-skill metrics and health

## Architecture

### Components

```
┌─────────────────────────────────────────────────────────────────┐
│                        Queue Manager                             │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                    Command Queue                         │    │
│  │                                                         │    │
│  │   ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │    │
│  │   │ System   │ │ Control  │ │ Query    │ │ Session  │  │    │
│  │   │ Lane     │ │ Lane     │ │ Lane     │ │ Lane     │  │    │
│  │   │ P:0 M:5  │ │ P:1 M:3  │ │ P:2 M:10 │ │ P:3 M:5  │  │    │
│  │   └──────────┘ └──────────┘ └──────────┘ └──────────┘  │    │
│  │                                                         │    │
│  │   ┌──────────┐ ┌──────────┐ ┌────────────────────────┐ │    │
│  │   │ Skill    │ │ Prompt   │ │ skill:custom-lane      │ │    │
│  │   │ Lane     │ │ Lane     │ │ (dynamic)              │ │    │
│  │   │ P:4 M:3  │ │ P:5 M:2  │ │ P:4 M:?                │ │    │
│  │   └──────────┘ └──────────┘ └────────────────────────┘ │    │
│  │                                                         │    │
│  │                    ┌──────────┐                         │    │
│  │                    │Scheduler │ (10ms tick)             │    │
│  │                    └──────────┘                         │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│  ┌───────────────────────────┼───────────────────────────────┐  │
│  │                    Queue Monitor                           │  │
│  │  • Health checking (10s interval)                         │  │
│  │  • Capacity warnings                                      │  │
│  │  • Metrics collection                                     │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                   │
│  ┌───────────────────────────┼───────────────────────────────┐  │
│  │                    Event Emitter                           │  │
│  │  • queue.lane.pressure                                    │  │
│  │  • queue.lane.idle                                        │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Queue Manager

The `QueueManager` provides a high-level API using the builder pattern:

```rust
let manager = QueueManagerBuilder::new(event_emitter)
    .with_default_lanes()
    .with_lane("skill:web-search", LaneConfig {
        min_concurrency: 1,
        max_concurrency: 2,
    }, priorities::SKILL)
    .build()
    .await?;

// Start the scheduler
manager.start().await?;

// Submit a command
let result_rx = manager.submit("prompt", generate_command).await?;
let result = result_rx.await??;
```

### Queue Monitor

The `QueueMonitor` provides health checking and observability:

```rust
let monitor = QueueMonitor::with_config(queue, MonitorConfig {
    interval: Duration::from_secs(10),
    pending_warning_threshold: 100,
    active_warning_threshold: 50,
});

// Start monitoring (logs warnings, emits events)
Arc::new(monitor).start().await;

// Get current statistics
let stats = monitor.stats().await;
println!("Total pending: {}", stats.total_pending);
println!("Total active: {}", stats.total_active);
```

### Event Integration

The queue emits events for observability:

| Event | Trigger |
|-------|---------|
| `queue.lane.pressure` | Lane at max capacity with pending commands |
| `queue.lane.idle` | Lane has no pending or active commands |

## Command Interface

Commands implement the `Command` trait:

```rust
#[async_trait]
pub trait Command: Send + Sync {
    /// Execute the command
    async fn execute(&self) -> Result<serde_json::Value>;

    /// Get command type (for logging/debugging)
    fn command_type(&self) -> &str;
}
```

Example implementation:

```rust
struct GenerateCommand {
    session_id: String,
    prompt: String,
    llm_client: Arc<LlmClient>,
}

#[async_trait]
impl Command for GenerateCommand {
    async fn execute(&self) -> Result<serde_json::Value> {
        let response = self.llm_client
            .generate(&self.session_id, &self.prompt)
            .await?;
        Ok(serde_json::to_value(response)?)
    }

    fn command_type(&self) -> &str {
        "generate"
    }
}
```

## Real-World Scenarios

### Scenario 1: Cancel During Generation

```
Timeline:
0ms    User sends "generate code" → Prompt Lane
100ms  LLM starts processing...
500ms  User clicks "Cancel" → Control Lane
510ms  Cancel executes (doesn't wait for generate)
520ms  Generation cancelled
```

Without lanes: Cancel would wait 30+ seconds behind generation.

### Scenario 2: Health Check During Heavy Load

```
Timeline:
0ms    10 prompt requests queued → Prompt Lane (max 2)
0ms    Kubernetes liveness probe → System Lane
10ms   Health check responds "OK"
...    Prompts continue processing
```

Without lanes: Kubernetes kills the pod after 30s timeout.

### Scenario 3: Concurrent UI Queries

```
Timeline:
0ms    UI polls getContextUsage → Query Lane
0ms    UI polls getHistory → Query Lane
0ms    UI polls getCapabilities → Query Lane
10ms   All 3 queries complete in parallel
```

Without lanes: Queries execute sequentially, UI feels sluggish.

### Scenario 4: Skill Activation with Rate Limiting

```
Timeline:
0ms    Activate skill:web-search → Skill Lane
0ms    Activate skill:image-gen → Skill Lane
0ms    Activate skill:database → Skill Lane
0ms    Activate skill:email → Skill Lane (queued, max 3)
500ms  First skill completes
510ms  Fourth skill starts
```

Without lanes: All skills download simultaneously, saturating network.

## Configuration

### BoxConfig

```rust
pub struct BoxConfig {
    /// Lane configurations
    pub lanes: HashMap<String, LaneConfig>,
    // ...
}
```

### Default Configuration

```rust
fn default_lanes() -> HashMap<String, LaneConfig> {
    let mut lanes = HashMap::new();

    lanes.insert("system".to_string(), LaneConfig {
        min_concurrency: 1,
        max_concurrency: 1,
    });

    lanes.insert("control".to_string(), LaneConfig {
        min_concurrency: 1,
        max_concurrency: 8,
    });

    lanes.insert("query".to_string(), LaneConfig {
        min_concurrency: 1,
        max_concurrency: 8,
    });

    lanes.insert("session".to_string(), LaneConfig {
        min_concurrency: 1,
        max_concurrency: 4,
    });

    lanes.insert("skill".to_string(), LaneConfig {
        min_concurrency: 1,
        max_concurrency: 4,
    });

    lanes.insert("prompt".to_string(), LaneConfig {
        min_concurrency: 1,
        max_concurrency: 1,
    });

    lanes
}
```

### Tuning Guidelines

| Factor | Adjustment |
|--------|------------|
| High memory usage | Reduce prompt lane max_concurrency |
| Slow health checks | Ensure system lane has capacity |
| Sluggish UI | Increase query lane max_concurrency |
| Network saturation | Reduce skill lane max_concurrency |
| Cancel unresponsive | Ensure control lane has dedicated capacity |

## Comparison with Alternatives

### vs. Single Thread (Sequential)

| Aspect | Single Thread | Lane-Based |
|--------|---------------|------------|
| Simplicity | Simple | Moderate |
| Priority | FIFO only | Priority-aware |
| Concurrency | None | Per-lane control |
| Throughput | Low | High |
| Responsiveness | Poor | Excellent |

### vs. Unlimited Threads (Parallel)

| Aspect | Unlimited Threads | Lane-Based |
|--------|-------------------|------------|
| Resource Usage | Unbounded | Controlled |
| Priority | None | Priority-aware |
| Predictability | Chaotic | Deterministic |
| Backpressure | None | Built-in |
| Debugging | Difficult | Easier |

### vs. Global Thread Pool

| Aspect | Global Pool | Lane-Based |
|--------|-------------|------------|
| Priority | Limited | Full control |
| Isolation | None | Per-lane |
| Fairness | Work-stealing | Priority-based |
| Observability | Generic | Domain-specific |

## Implementation Details

### Scheduler Loop

The scheduler runs on a 10ms tick:

```rust
pub async fn start_scheduler(self: Arc<Self>) {
    tokio::spawn(async move {
        loop {
            self.schedule_next().await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
}
```

10ms provides good responsiveness while avoiding busy-waiting.

### Command Lifecycle

```
1. Submit     → Command enters lane's pending queue
2. Dequeue    → Scheduler picks highest-priority lane with capacity
3. Execute    → Command runs in spawned task
4. Complete   → Result sent via oneshot channel, active count decremented
```

### Backpressure

When a lane is at capacity:
- New commands queue in pending
- Scheduler moves to next priority level
- Monitor emits `queue.lane.pressure` event
- Caller awaits result via oneshot channel

## Summary

The Lane-Based Command Queue provides:

1. **Priority Scheduling**: Critical operations (health, cancel) execute immediately
2. **Concurrency Control**: Per-lane limits prevent resource exhaustion
3. **Isolation**: Different command types don't interfere with each other
4. **Observability**: Per-lane metrics and health monitoring
5. **Extensibility**: Dynamic lanes for skills with custom requirements

This design is essential for production AI coding agents where responsiveness, reliability, and resource management are critical.
