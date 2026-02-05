# A3S

<p align="center">
  <strong>Autonomous Agent Adaptive System</strong>
</p>

<p align="center">
  <em>A modular Rust ecosystem for building secure, production-ready AI agents</em>
</p>

<p align="center">
  <a href="#architecture">Architecture</a> •
  <a href="#projects">Projects</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#development">Development</a>
</p>

---

## Overview

**A3S** is a collection of Rust crates designed to work together as a complete infrastructure for AI agents. Each component has a specific role and can be used independently or as part of the full stack.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           A3S Ecosystem                                 │
│                                                                         │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │  Application Layer                                                │  │
│  │  ┌─────────────────────────────────────────────────────────────┐  │  │
│  │  │  a3s-code (AI Coding Agent)                                 │  │  │
│  │  │  - Multi-session management    - Permission system          │  │  │
│  │  │  - Tool execution (8 tools)    - Human-in-the-loop          │  │  │
│  │  │  - Skills system               - Subagent delegation        │  │  │
│  │  │  - LSP integration             - MCP support                │  │  │
│  │  └─────────────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │  Utility Layer                                                    │  │
│  │  ┌─────────────────────────┐  ┌─────────────────────────────────┐ │  │
│  │  │  a3s-lane               │  │  a3s-context                    │ │  │
│  │  │  - Priority queues      │  │  - Hierarchical memory          │ │  │
│  │  │  - Async scheduling     │  │  - Knowledge management         │ │  │
│  │  │  - Dead letter queue    │  │  - Context providers            │ │  │
│  │  └─────────────────────────┘  └─────────────────────────────────┘ │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │  Infrastructure Layer                                             │  │
│  │  ┌─────────────────────────────────────────────────────────────┐  │  │
│  │  │  a3s-box (MicroVM Sandbox)                                  │  │  │
│  │  │  - Hardware-level isolation   - Secure agent execution      │  │  │
│  │  │  - Resource limits            - Network isolation           │  │  │
│  │  └─────────────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────┘
```

## Projects

### a3s-code — AI Coding Agent

**Role**: Application layer - the main AI agent that interacts with users and executes tasks.

| Feature | Description |
|---------|-------------|
| **Multi-Session** | Run multiple independent AI conversations |
| **Tool System** | 8 built-in tools (bash, read, write, edit, grep, glob, ls, web_fetch) |
| **Permissions** | Fine-grained Allow/Deny/Ask rules for tool access |
| **HITL** | Human-in-the-loop confirmation for sensitive operations |
| **Skills** | Extend with custom tools via Markdown definitions |
| **Subagents** | Delegate tasks to specialized child agents |
| **LSP** | Code intelligence (hover, definition, references) |
| **MCP** | Model Context Protocol for external tool integration |

```bash
# Run the agent
./a3s-code --config ~/.a3s/config.json
```

📦 [crates.io](https://crates.io/crates/a3s-code) · 📖 [Documentation](crates/code/README.md)

---

### a3s-lane — Priority Command Queue

**Role**: Utility layer - async task scheduling with priority-based execution.

| Feature | Description |
|---------|-------------|
| **Priority Lanes** | Multiple priority levels for task scheduling |
| **Async Runtime** | Built on Tokio for high-performance async execution |
| **Dead Letter Queue** | Failed tasks are preserved for retry or inspection |
| **Metrics** | Built-in metrics for monitoring queue health |

```rust
use a3s_lane::{Lane, Priority};

let lane = Lane::new();
lane.push(Priority::High, my_task).await;
let result = lane.pop().await;
```

📦 [crates.io](https://crates.io/crates/a3s-lane) · 📖 [Documentation](crates/lane/README.md)

---

### a3s-context — Hierarchical Context Management

**Role**: Utility layer - memory and knowledge management for AI agents.

| Feature | Description |
|---------|-------------|
| **Hierarchical Memory** | Working / Short-term / Long-term memory tiers |
| **Context Providers** | Pluggable providers for external knowledge sources |
| **Compaction** | Automatic summarization for long conversations |

```rust
use a3s_context::{Context, MemoryTier};

let ctx = Context::new();
ctx.store(MemoryTier::Working, "key", value).await;
let data = ctx.retrieve("key").await;
```

📦 [crates.io](https://crates.io/crates/a3s_context) · 📖 [Documentation](crates/context/README.md)

---

### a3s-box — MicroVM Sandbox Runtime

**Role**: Infrastructure layer - secure execution environment with hardware isolation.

| Feature | Description |
|---------|-------------|
| **MicroVM Isolation** | Hardware-level isolation using lightweight VMs |
| **Resource Limits** | CPU, memory, and disk quotas |
| **Network Isolation** | Controlled network access |
| **Secure Execution** | Run untrusted agent code safely |

```rust
use a3s_box_runtime::BoxRuntime;

let runtime = BoxRuntime::new(config);
runtime.start_agent("a3s-code").await;
```

📦 [crates.io](https://crates.io/crates/a3s-box-runtime) · 📖 [Documentation](crates/box/README.md)

---

### a3s-cron — Cron Scheduling Library

**Role**: Utility layer - task scheduling with cron syntax and natural language support.

| Feature | Description |
|---------|-------------|
| **Cron Syntax** | Standard 5-field cron expressions (minute hour day month weekday) |
| **Natural Language** | Parse schedules from English/Chinese ("every 5 minutes", "每天凌晨2点") |
| **Persistence** | JSON file-based storage with pluggable backends |
| **CRUD Operations** | Create, pause, resume, update, and remove scheduled jobs |
| **Execution History** | Track job runs with output and status |

```rust
use a3s_cron::{CronManager, parse_natural};

// Parse natural language to cron expression
let cron = parse_natural("every day at 2am")?;  // "0 2 * * *"

// Create and manage jobs
let manager = CronManager::new(store);
manager.add_job("backup", "0 2 * * *", "backup.sh").await?;
```

📦 [crates.io](https://crates.io/crates/a3s-cron) · 📖 [Documentation](crates/cron/README.md)

---

### a3s-tools — Built-in Tools Binary

**Role**: Utility - standalone binary providing core tools for the agent.

| Tool | Description |
|------|-------------|
| `bash` | Execute shell commands |
| `read` | Read files with line numbers |
| `write` | Write content to files |
| `edit` | Edit files with string replacement |
| `grep` | Search files with ripgrep |
| `glob` | Find files by pattern |
| `ls` | List directory contents |
| `cron` | Manage scheduled tasks |

📦 [crates.io](https://crates.io/crates/a3s-tools) · 📖 [Documentation](crates/tools/README.md)

## Quick Start

### Clone Repository

```bash
git clone --recursive https://github.com/A3S-Lab/a3s.git
cd a3s

# Or update existing clone
git submodule update --init --recursive
```

### Build

```bash
# Build workspace crates
just build

# Build everything including box
just build-all
```

### Test

```bash
# Test workspace crates
just test

# Test everything
just test-all
```

### Run Agent

```bash
# Create config
mkdir -p ~/.a3s
cat > ~/.a3s/config.json << 'EOF'
{
  "defaultProvider": "anthropic",
  "defaultModel": "claude-sonnet-4-20250514",
  "providers": [{
    "name": "anthropic",
    "apiKey": "YOUR_API_KEY",
    "models": [{"id": "claude-sonnet-4-20250514", "toolCall": true}]
  }]
}
EOF

# Run
./target/release/a3s-code --config ~/.a3s/config.json
```

## Repository Structure

```
a3s/
├── Cargo.toml              # Workspace definition
├── justfile                # Build commands
├── README.md
├── sdk/
│   ├── python/             # Python SDK
│   └── typescript/         # TypeScript SDK
└── crates/
    ├── box/                # [submodule] MicroVM sandbox runtime
    ├── code/               # [submodule] AI coding agent
    ├── cron/               # [submodule] Cron scheduling library
    ├── lane/               # [submodule] Priority command queue
    ├── context/            # [submodule] Context management
    ├── tools/              # Built-in tools binary
    └── tools-core/         # Core types for tools
```

## Development

### Prerequisites

- Rust 1.75+
- [just](https://github.com/casey/just) command runner

### Commands

| Command | Description |
|---------|-------------|
| `just build` | Build workspace crates |
| `just build-all` | Build everything (including box) |
| `just test` | Test workspace crates |
| `just test-all` | Test everything |
| `just fmt` | Format all code |
| `just lint` | Run clippy on all code |
| `just ci` | Run full CI checks |
| `just publish` | Publish all crates |
| `just version` | Show all crate versions |

## SDKs

| Language | Package | Installation |
|----------|---------|--------------|
| TypeScript | `@a3s-lab/code` | `npm install @a3s-lab/code` |
| Python | `a3s-code` | `pip install a3s-code` |

## License

MIT License - see [LICENSE](LICENSE) for details.

---

<p align="center">
  Built by <a href="https://github.com/a3s-lab">A3S Lab</a>
</p>
