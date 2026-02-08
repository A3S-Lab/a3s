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
│  │  │  - Tool execution (10 tools)   - Human-in-the-loop          │  │  │
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
│  │  ┌─────────────────────────┐  ┌─────────────────────────────────┐ │  │
│  │  │  a3s-cron               │  │  a3s-search                     │ │  │
│  │  │  - Cron scheduling      │  │  - Meta search engine           │ │  │
│  │  │  - Natural language     │  │  - Multi-engine aggregation     │ │  │
│  │  └─────────────────────────┘  └─────────���───────────────────────┘ │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │  Infrastructure Layer                                             │  │
│  │  ┌──────────────────────────────┐  ┌────────────────────────────┐ │  │
│  │  │  a3s-box (MicroVM Sandbox)   │  │  a3s-power (LLM Engine)   │ │  │
│  │  │  - Hardware-level isolation  │  │  - OpenAI + Ollama API    │ │  │
│  │  │  - VM snapshot/restore       │  │  - llama.cpp backend      │ │  │
│  │  │  - Warm pool management      │  │  - Cost tracking          │ │  │
│  │  └──────────────────────────────┘  └────────────────────────────┘ │  │
│  │  ┌──────────────────────────────┐                                 │  │
│  │  │  SafeClaw (Security Gateway) │                                 │  │
│  │  │  - 7-layer defense           │                                 │  │
│  │  │  - Runtime audit pipeline    │                                 │  │
│  │  │  - TEE support               │                                 │  │
│  │  └──────────────────────────────┘                                 │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │  Observability Layer (OpenTelemetry)                               │  │
│  │  - End-to-end distributed tracing across all components           │  │
│  │  - LLM cost tracking (model / tokens / cost per call)             │  │
│  │  - Metrics export: Prometheus / OTLP → SigNoz                     │  │
│  │  - Security audit event pipeline → NATS Stream                    │  │
│  └───────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────┘
```

## Projects

### a3s-code — AI Coding Agent

**Role**: Application layer - the main AI agent that interacts with users and executes tasks.

| Feature | Description |
|---------|-------------|
| **Multi-Session** | Run multiple independent AI conversations |
| **Tool System** | 10 built-in tools (bash, read, write, edit, grep, glob, ls, web_fetch, cron, parse) |
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

### a3s-power — Local LLM Inference Engine

**Role**: Infrastructure layer - local model management and serving with dual-protocol API.

| Feature | Description |
|---------|-------------|
| **Ollama-Compatible API** | Drop-in replacement with 12+ endpoints |
| **OpenAI-Compatible API** | `/v1/chat/completions`, `/v1/models`, `/v1/embeddings` |
| **llama.cpp Backend** | GGUF inference via Rust bindings |
| **Multi-Model** | Concurrent model loading with LRU eviction |
| **Cost Tracking** | Per-call token counting and cost recording |

```bash
a3s-power pull llama3.2:3b
a3s-power serve  # Start HTTP server
```

📖 [Documentation](crates/power/README.md)

---

### SafeClaw — Security Gateway with TEE Support

**Role**: Infrastructure layer - privacy-focused security gateway with hardware-isolated execution.

| Feature | Description |
|---------|-------------|
| **7-Layer Defense** | Hardware → namespace → container → permission → HITL → data → network |
| **PII Detection** | Regex + ML-augmented sensitive data detection |
| **Taint Tracking** | Track sensitive data flow through the system |
| **Runtime Audit** | Audit event pipeline → NATS Stream → alerting + persistence |
| **TEE Support** | AMD SEV-SNP encrypted execution environment |

📖 [Documentation](crates/safeclaw/README.md)

---

### a3s-search — Meta Search Engine

**Role**: Utility layer - aggregate search results from multiple engines.

| Feature | Description |
|---------|-------------|
| **Multi-Engine** | 8 built-in engines (DuckDuckGo, Wikipedia, Baidu, etc.) |
| **Consensus Ranking** | Results found by multiple engines rank higher |
| **Proxy Pool** | Dynamic proxy IP rotation |
| **Async-First** | Parallel search with per-engine timeout |

```bash
a3s-search "Rust programming" -e ddg,wiki,baidu
```

�� [Documentation](crates/search/README.md)

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
    ├── power/              # [submodule] Local LLM inference engine
    ├── safeclaw/           # [submodule] Security gateway with TEE
    ├── search/             # [submodule] Meta search engine
    ├── tools/              # Built-in tools binary
    └── tools-core/         # Core types for tools
```

## Roadmap

### Cross-Cutting Optimization Priorities

| Priority | Optimization | Scope | Timeline |
|----------|-------------|-------|----------|
| 🔴 P0 | **MicroVM Cold Start** — VM snapshot/restore + warm pool + layered model cache | a3s-box | 4-6 weeks |
| 🟡 P1 | **OpenTelemetry Integration** — End-to-end tracing across all crates | all crates | 2-3 weeks |
| 🟡 P1 | **LLM Cost Tracking** — Per-call token/cost recording → Cost Dashboard | a3s-power, a3s-code | 2-3 weeks |
| 🟡 P1 | **Runtime Security Audit** — Audit pipeline + drift detection + panic elimination | SafeClaw | 2-3 weeks |
| 🟢 P2 | **Distributed Scheduling** — Multi-node job distribution with leader election | a3s-cron | 3-4 weeks |
| 🟢 P2 | **ML-based Search Ranking** — Learning-to-rank for result quality | a3s-search | 3-4 weeks |

See each crate's README for detailed per-component roadmaps.

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
