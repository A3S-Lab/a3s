# A3S

<p align="center">
  <strong>Agentic Adaptive Augmentation System</strong>
</p>

<p align="center">
  <em>An Agent Operating System — from hardware-isolated execution to multi-agent orchestration and agentic evolution</em>
</p>

<p align="center">
  <a href="#installation">Installation</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#projects">Projects</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#a3s-deep--agentic-deep-research-agent">Deep Research</a> •
  <a href="#sdks">SDKs</a> •
  <a href="#development">Development</a>
</p>

---

## Overview

**A3S** is not just a collection of crates — it is an **Agent Operating System**. It provides the full stack for declaring, packaging, deploying, securing, and evolving AI agents at scale.

The core data flow:

```
a3s-gateway (OS external gateway, single entry point for all traffic)
    → SafeClaw (OS main application, runs inside a3s-box MicroVM)
        → A3sfile DSL (orchestrates multiple a3s-code agents + models + tools)
            → a3s-code instances (each with its own a3s-lane priority queue)
                → Reflection system (error classification, adaptive strategy, memory-based evolution)
```

Each component can also be used independently as a standalone Rust crate.

## Installation

### Homebrew (macOS / Linux)

```bash
# Add the A3S tap
brew tap a3s-lab/tap

# Install individual components
brew install a3s-code       # AI coding agent
brew install a3s-search     # Meta search engine
brew install a3s-power      # Local LLM inference engine
brew install a3s-tools      # Built-in tools binary

# Or install everything
brew install a3s
```

### From Source

```bash
git clone --recursive https://github.com/A3S-Lab/a3s.git
cd a3s
just build          # Debug build
just release        # Release build
```

### Cargo

```bash
cargo install a3s-code
cargo install a3s-search
```

### Pre-built Binaries

Download from [GitHub Releases](https://github.com/A3S-Lab/a3s/releases) for your platform.

## Architecture

```
                         External Traffic (Internet / Messaging Platforms)
          ┌───────┬───────┬───────┬───────┬───────┬──────────┐
          │Telegram│ Slack │ Feishu│DingTalk│WebChat│ HTTP/gRPC│
          └───┬───┴───┬───┴───┬───┴───┬───┴───┬───┴────┬─────┘
              └───────┴───────┴───────┴───────┘        │
                              │                        │
                ┌─────────────▼────────────────────────▼───────┐
                │              a3s-gateway                      │
                │           (OS External Gateway)               │
                │  TLS/ACME · Auth/JWT · Rate Limit · CORS     │
                │  7-Platform Webhook Normalization              │
                │  Privacy-Aware Routing · Token Metering        │
                │  Load Balancing · Agent Health Probe            │
                └──────────────────┬───────────────────────────┘
                                   │
                ┌──────────────────▼───────────────────────────┐
                │              a3s-box MicroVM                  │
                │         (Hardware-Level Sandbox)              │
                │                                              │
                │  ┌────────────────────────────────────────┐  │
                │  │          SafeClaw (OS Application)      │  │
                │  │   Message Routing · Multi-Agent Coord   │  │
                │  │   Privacy Escalation · TEE Upgrade      │  │
                │  │                                        │  │
                │  │  ┌── A3sfile (Agent Resource DSL) ──┐  │  │
                │  │  │  model "qwen3" { ... }           │  │  │
                │  │  │  tool "search" { ... }           │  │  │
                │  │  │  agent "architect" { ... }       │  │  │
                │  │  │  agent "coder" { ... }           │  │  │
                │  │  │  orchestration { hierarchical }  │  │  │
                │  │  └──────────────────────────────────┘  │  │
                │  │               │                        │  │
                │  │    ┌──────────┼──────────┐             │  │
                │  │    ▼          ▼          ▼             │  │
                │  │ ┌────────┐┌────────┐┌────────┐        │  │
                │  │ │a3s-code││a3s-code││a3s-code│        │  │
                │  │ │Agent A ││Agent B ││Agent C │        │  │
                │  │ │a3s-lane││a3s-lane││a3s-lane│        │  │
                │  │ └────────┘└────────┘└────────┘        │  │
                │  └────────────────────────────────────────┘  │
                └──────────────────────────────────────────────┘
                       │              │              │
                 ┌─────▼────┐  ┌─────▼────┐  ┌─────▼────┐
                 │ a3s-power│  │a3s-search│  │a3s-context│
                 │ LLM Eng. │  │ Search   │  │ Context  │
                 └──────────┘  └──────────┘  └──────────┘

  Shared: a3s-privacy (PII classification) · a3s-transport (vsock framing)
  Observability: OpenTelemetry spans · Prometheus metrics · SigNoz dashboards
```

### Layer Responsibilities

| Layer | Component | Role |
|-------|-----------|------|
| Gateway | a3s-gateway | OS single entry point: TLS, auth, 7-platform webhook normalization, privacy routing, token metering, load balancing, agent health probes |
| Sandbox | a3s-box | MicroVM hardware isolation, WarmPool, CRI for Kubernetes |
| Application | SafeClaw | OS main application: message channel routing, multi-agent session management, privacy escalation, TEE upgrade |
| Orchestration | A3sfile DSL + Super Factory | Agent resource DSL for SafeClaw's underlying a3s-code agents: declares models, tools, agents, and collaboration topology |
| Execution | a3s-code | Individual AI agent: tool calling, reflection, adaptive strategy, skills, subagents |
| Scheduling | a3s-lane | Per-session priority queue: 6 lanes, concurrency control, retry, dead letter |
| Infrastructure | a3s-power / a3s-search / a3s-context / a3s-cron | LLM inference / meta search / context management / cron scheduling |
| Shared | a3s-privacy / a3s-transport | PII classification & redaction / vsock frame protocol |
| Observability | OpenTelemetry + Prometheus | OTLP spans, metrics, W3C/B3 trace propagation, SigNoz dashboards |

## Projects

### a3s-code — AI Coding Agent

Execution layer — the individual AI agent that SafeClaw orchestrates. Multiple a3s-code instances run in-process within SafeClaw, each with its own session, priority queue, and reflection system.

- **Multi-Session Management**: Run multiple independent AI conversations with file/memory storage
- **11 Built-in Tools**: bash, read, write, edit, patch, grep, glob, ls, web_fetch, web_search, cron — all workspace-sandboxed
- **Permission System**: Fine-grained Allow/Deny/Ask rules for tool access
- **HITL Confirmation**: Human-in-the-loop for sensitive operations with configurable timeout policies
- **Skills & Subagents**: Extend with Markdown skill definitions (Claude Code Skills format); delegate tasks to 5 built-in specialized child agents
- **Server-Side Agentic Loop**: Full agentic loop execution on server with streaming events; server-side delegation to subagents
- **LSP Integration**: Code intelligence (hover, definition, references, symbols, diagnostics) for Rust, Go, TypeScript, Python, C/C++
- **MCP Support**: Model Context Protocol with stdio/HTTP transport, OAuth config, `mcp__<server>__<tool>` naming
- **Reflection System**: 10 error categories, 4 adaptive strategies (Direct/Planned/Iterative/Parallel), confidence tracking
- **Memory System**: Episodic/Semantic/Procedural memory with importance scoring and access tracking
- **Planning & Goals**: LLM-based execution plans, goal extraction, achievement tracking
- **Hooks System**: 8 lifecycle events (PreToolUse, PostToolUse, GenerateStart/End, SessionStart/End, SkillLoad/Unload)
- **Enhanced Health Check**: Subsystem diagnostics (version, uptime, session count, store health)
- **Pluggable Session Persistence**: `SessionStore` trait with `Custom` backend for external stores (PostgreSQL, etc.)
- **Structured Generation**: JSON Schema constrained output, both unary and streaming
- **Cron Scheduling**: 10 cron RPCs for scheduled task management
- **OpenTelemetry**: OTLP spans (agent → turn → llm → tool → subagent), LLM cost tracking, cross-session cost aggregation
- **SDKs**: Python & TypeScript covering all 85 RPCs, with high-level `Session` API (`send()`, `stream()`, `delegate()`)
- **1,716 unit tests**

```bash
# Install
brew install a3s-code

# Run
a3s-code --config ~/.a3s/config.json
```

📦 [crates.io](https://crates.io/crates/a3s-code) · 📖 [Documentation](crates/code/README.md)

---

### a3s-lane — Per-Session Priority Queue

Scheduling layer — each a3s-code agent session gets its own a3s-lane instance for priority-based command scheduling. Control commands (pause/cancel) always preempt LLM generation tasks.

- **6 Priority Lanes**: system (P0) → control (P1) → query (P2) → session (P3) → skill (P4) → prompt (P5)
- **Per-Lane Concurrency**: Configurable min/max concurrency per lane
- **Command Timeout**: Configurable timeout per lane with automatic cancellation
- **Retry Policies**: Exponential backoff, fixed delay, or custom retry strategies
- **Dead Letter Queue**: Capture permanently failed commands for inspection
- **Persistent Storage**: Pluggable storage backend (LocalStorage included)
- **Rate Limiting**: Token bucket and sliding window per lane
- **Priority Boosting**: Deadline-based automatic priority escalation
- **Metrics & Alerts**: Latency histograms (p50/p90/p95/p99), queue depth alerts
- **OpenTelemetry**: OTLP spans on submit/execute/retry, OtelMetricsBackend bridging MetricsBackend trait
- **230 tests** with 96% line coverage

```rust
use a3s_lane::{QueueManagerBuilder, EventEmitter, LaneConfig};

let emitter = EventEmitter::new(100);
let manager = QueueManagerBuilder::new(emitter)
    .with_default_lanes()
    .build()
    .await?;
manager.start().await?;
```

📦 [crates.io](https://crates.io/crates/a3s-lane) · 📖 [Documentation](crates/lane/README.md)

---

### a3s-box — MicroVM Sandbox Runtime

Sandbox layer — hardware-isolated execution environment. SafeClaw runs inside a3s-box MicroVMs, providing hardware-level security boundaries for all agent operations.

- **MicroVM Isolation**: Each sandbox runs in its own MicroVM via libkrun (~200ms cold start)
- **Docker-like CLI**: 29 commands: run, stop, exec, cp, images, build, push, network, volume, attest...
- **OCI Images**: Pull/push standard container images, full Dockerfile build support
- **WarmPool**: Pre-warmed VM pool for instant agent deployment (`min_idle` / `max_size` / `idle_ttl`)
- **Bridge Networking**: passt-based networking, custom networks, DNS service discovery, container-to-container communication
- **Named Volumes**: Volume CRUD, tmpfs mounts, anonymous volumes with auto-cleanup
- **CRI Integration**: Kubernetes Container Runtime Interface for native K8s scheduling
- **TEE Support**: AMD SEV-SNP hardware memory encryption + remote attestation
- **Cross-Platform**: macOS Apple Silicon (HVF) + Linux x86_64/ARM64 (KVM), no root required

```bash
a3s-box run --rm -it ubuntu:22.04 /bin/bash
a3s-box build -t my-agent .
```

📦 [crates.io](https://crates.io/crates/a3s-box-runtime) · 📖 [Documentation](crates/box/README.md)

---

### SafeClaw — OS Main Application

The central application of the A3S operating system. Runs inside a3s-box MicroVM, proxies message channels, and coordinates multiple a3s-code agent instances.

- **Multi-Channel Routing**: 7 platform adapters (Telegram, Feishu, DingTalk, WeCom, Slack, Discord, WebChat) via a3s-gateway
- **Multi-Agent Coordination**: In-process a3s-code library integration via `AgentEngine`
- **Privacy Escalation**: Session-level sensitivity ratchet (Normal → Sensitive → HighlySensitive → Critical → TEE upgrade)
- **A3sfile Orchestration**: Declares and orchestrates underlying a3s-code agents, models, tools, and collaboration topology
- **4-Layer Security**: Hardware TEE → Channel encryption → Protocol auth → Application classification
- **Distributed TEE**: Split-Process-Merge: Coordinator TEE decomposes tasks, Workers process, Validator verifies
- **Taint Tracking**: Follow sensitive data through base64/hex transformations, block leakage vectors
- **Desktop UI**: Tauri v2 + React + TypeScript native desktop application

📖 [Documentation](crates/safeclaw/README.md)

---

### a3s-gateway — OS External Gateway

The single entry point for all external traffic into the A3S operating system. SafeClaw and agent backends are never exposed to the public network.

- **Reverse Proxy**: HTTP/HTTPS/WebSocket/gRPC/TCP/UDP/SSE proxying
- **Dynamic Routing**: Traefik-style rule engine (`Host()`, `PathPrefix()`, `Headers()`, `HostSNI()`)
- **Load Balancing**: Round-robin, weighted, least-connections, random + sticky sessions
- **10 Middlewares**: Auth, JWT, rate-limit, CORS, headers, strip-prefix, retry, circuit-breaker, IP allow, compress
- **7-Platform Webhooks**: Telegram, Slack, Discord, Feishu, DingTalk, WeCom, WebChat → unified `ChannelMessage`
- **Privacy-Aware Routing**: Content classification → route to Local or TEE based on sensitivity
- **Token Metering**: Sliding window limits per user/agent/session/global
- **Agent Health Probe**: AI-specific states: Loading → Ready → Busy → Error → Unreachable
- **TLS + ACME**: rustls TLS termination with Let's Encrypt auto-certificate
- **Hot Reload**: File-watch based config reload without restart

📖 [Documentation](crates/gateway/README.md)

---

### a3s-power — Local LLM Inference Engine

Infrastructure layer — local model management and serving with dual-protocol API.

- **Ollama Registry**: Pull any model from `registry.ollama.ai` by name with auto-resolved metadata
- **Ollama-Compatible API**: Drop-in replacement with 12+ endpoints and NDJSON streaming
- **OpenAI-Compatible API**: `/v1/chat/completions`, `/v1/models`, `/v1/embeddings` with JSON Schema structured output
- **llama.cpp Backend**: GGUF inference via Rust bindings with KV cache reuse and context window auto-detection
- **Multi-Model**: Concurrent model loading with LRU eviction and keep-alive
- **Multi-GPU**: Tensor split across GPUs with layer-based distribution
- **Tool Calling**: Streaming tool calls with indexed deltas and parallel tool call support
- **Cost Tracking**: Per-call token counting, cost recording, and Prometheus metrics

```bash
brew install a3s-power
a3s-power pull llama3.2:3b
a3s-power serve
```

📖 [Documentation](crates/power/README.md)

---

### a3s-search — Meta Search Engine

Utility layer — aggregate search results from multiple engines.

- **8 Built-in Engines**: DuckDuckGo, Wikipedia, Baidu, Bing, Google, Brave, Searx, Yandex
- **Consensus Ranking**: Results found by multiple engines rank higher
- **Proxy Pool**: Dynamic proxy IP rotation
- **Async-First**: Parallel search with per-engine timeout
- **267 tests**

```bash
brew install a3s-search
a3s-search "Rust programming" -e ddg,wiki,baidu
```

📖 [Documentation](crates/search/README.md)

---

### a3s-context — Hierarchical Context Management

Utility layer — memory and knowledge management for AI agents.

- **Hierarchical Memory**: Working / Short-term / Long-term memory tiers
- **Pathway URI Addressing**: Structured content organization with namespace system
- **Multi-level Digests**: Automatic summarization for long conversations
- **Embedding Support**: Pluggable embedders with 4 reranker providers (Cohere/Jina/OpenAI/Mock)
- **OpenTelemetry**: OTLP spans on ingest/query/embed/rerank/digest, metrics for query latency and node ingestion
- **114 tests**

📖 [Documentation](crates/context/README.md)

---

### a3s-event — Pluggable Event System

Infrastructure layer — provider-agnostic event publish, subscribe, and persistence across the A3S ecosystem.

- **Provider-Agnostic API**: `EventProvider` trait abstracts all backends — publish, subscribe, query with a single interface
- **Pluggable Backends**: Swap providers (NATS JetStream, in-memory, or custom) without changing application code
- **Publish/Subscribe**: Dot-separated subject hierarchy (`events.<category>.<topic>`) with wildcard routing
- **Durable Subscriptions**: Consumers survive disconnects and server restarts (provider-dependent)
- **At-Least-Once Delivery**: Explicit ack/nak via `PendingEvent` with automatic redelivery on failure
- **Payload Encryption**: AES-256-GCM with key rotation — protect sensitive payloads at the application layer
- **State Persistence**: Subscription filters survive restarts via pluggable `StateStore` (JSON file or custom)
- **Observability**: Lock-free `EventMetrics` counters for publish/subscribe/error/latency
- **83 tests**

```rust
use a3s_event::{EventBus, provider::memory::MemoryProvider};

let bus = EventBus::new(MemoryProvider::default());
bus.publish("market", "forex.usd_cny", "Rate update", "reuters",
    serde_json::json!({"rate": 7.3521})).await?;
```

📖 [Documentation](crates/event/README.md)

---

### a3s-cron — Cron Scheduling Library

Utility layer — task scheduling with cron syntax and natural language support.

- **Cron Syntax**: Standard 5-field cron expressions (minute hour day month weekday)
- **Natural Language**: Parse schedules from English/Chinese ("every 5 minutes", "每天凌晨2点")
- **Persistence**: JSON file-based storage with pluggable backends
- **CRUD Operations**: Create, pause, resume, update, and remove scheduled jobs
- **Execution History**: Track job runs with output and status
- **OpenTelemetry**: OTLP spans on job execution and scheduler ticks
- **79 tests**

```rust
use a3s_cron::{CronManager, parse_natural};

let cron = parse_natural("every day at 2am")?;  // "0 2 * * *"
let manager = CronManager::new(store);
manager.add_job("backup", "0 2 * * *", "backup.sh").await?;
```

📦 [crates.io](https://crates.io/crates/a3s-cron) · 📖 [Documentation](crates/cron/README.md)

---

### A3S Deep — Agentic Deep Research Agent

Application layer — a TypeScript agent that leverages A3S Code (with built-in Search + Lane) to perform iterative deep research and produce comprehensive reports.

- **Iterative Research Loop**: Plan → Search+Analyze → Reflect → repeat until confident
- **Interactive Clarification**: Multi-round questioning to refine ambiguous queries (`-i` mode)
- **Real-Time Steering**: Control Lane (P0) commands during research: `/focus`, `/add`, `/adjust`, `/skip`, `/stop`
- **Workspace Persistence**: All artifacts saved as `.md` files (plan, iterations, report)
- **Project Configuration**: `.a3s/config.json` for LLM/server settings, `.a3s/skills/` for custom skills, `.a3s/agents/` for subagents
- **Pluggable Output**: Built-in Markdown/JSON + skill-based renderers for Word, PDF, PPT, Remotion, HTML
- **Single SDK**: Only depends on `@a3s-lab/code` — Search, Lane, Tools, Skills all built-in

```bash
# Install
npm install @a3s-lab/deep

# Basic research
a3s-deep -q "What are the latest advances in quantum error correction?"

# Interactive mode with workspace
a3s-deep -q "Compare modern AI frameworks" -i -w ~/research/ai

# Output as PDF via skill
a3s-deep -q "State of WebAssembly in 2025" -o pdf

# Real-time steering (in interactive mode, type while running):
#   /focus quantum error correction with topological codes
#   /add "surface code threshold 2024 paper"
#   /skip
```

📦 [npm](https://www.npmjs.com/package/@a3s-lab/deep) · 📖 [Documentation](a3s-deep/README.md)

---

### a3s-tools — Built-in Tools Binary

Utility — standalone binary providing core tools for the agent.

- `bash` — Execute shell commands
- `read` — Read files with line numbers
- `write` — Write content to files
- `edit` — Edit files with string replacement
- `grep` — Search files with ripgrep
- `glob` — Find files by pattern
- `ls` — List directory contents
- `cron` — Manage scheduled tasks

📦 [crates.io](https://crates.io/crates/a3s-tools) · 📖 [Documentation](crates/tools/README.md)

---

### a3s-updater — Self-Update Library

Utility — self-update for A3S CLI binaries via GitHub Releases.

- **GitHub Releases**: Fetch latest release from GitHub API
- **Binary Replacement**: Download and replace running binary in-place
- **Semver Check**: Skip update if already on latest version

```rust
use a3s_updater::check_update;

let update = check_update("a3s-code", current_version).await?;
if update.available {
    update.apply().await?;
}
```

📖 [Source](crates/updater/)

## Quick Start

### 1. Install

```bash
brew tap a3s-lab/tap && brew install a3s-code
```

### 2. Configure

```bash
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
```

### 3. Run

```bash
a3s-code --config ~/.a3s/config.json
```

### 4. Use the SDK

```python
# pip install a3s-code
from a3s_code import A3sClient, create_provider

anthropic = create_provider(name="anthropic", api_key="YOUR_API_KEY")

async with A3sClient(address="localhost:4088") as client:
    async with await client.session(
        model=anthropic("claude-sonnet-4-20250514"),
        workspace="/tmp/demo",
        system="You are a helpful coding assistant.",
    ) as session:
        # Simple question
        result = await session.send("Write hello world in Rust")
        print(result.text)

        # Streaming
        async for event in session.stream("Explain this codebase"):
            if event.type == "text":
                print(event.content, end="", flush=True)
```

```typescript
// npm install @a3s-lab/code
import { A3sClient, createProvider } from '@a3s-lab/code';

const client = new A3sClient({ address: 'localhost:4088' });
const anthropic = createProvider({ name: 'anthropic', apiKey: 'YOUR_API_KEY' });

await using session = await client.createSession({
  model: anthropic('claude-sonnet-4-20250514'),
  workspace: '/tmp/demo',
  system: 'You are a helpful coding assistant.',
});

const { text } = await session.send('Write hello world in Rust');
console.log(text);
```

## SDKs

| Crate | Language | Package | RPCs | Location |
|-------|----------|---------|------|----------|
| a3s-code | Python | `a3s-code` | 85 RPCs | `crates/code/sdk/python/` |
| a3s-code | TypeScript | `@a3s-lab/code` | 85 RPCs | `crates/code/sdk/typescript/` |
| a3s-search | Python | `a3s-search` | — | `crates/search/sdk/python/` |
| a3s-search | Node.js | `@a3s-lab/search` | — | `crates/search/sdk/node/` |
| a3s-deep | TypeScript | `@a3s-lab/deep` | — | `a3s-deep/` |

SDK documentation covers every feature category: sessions, generation, structured output, skills, permissions, HITL, events, context, todos, providers, planning, memory, MCP, LSP, cron, and observability. Both Python and TypeScript SDKs provide a high-level `Session` API with `send()`, `stream()`, `delegate()`, and `async with` / `await using` auto-cleanup.

## Test Coverage

**Total: 3,846+ tests**

| Crate | Tests | Coverage | Status |
|-------|------:|----------|--------|
| a3s-code | 1,716 | — | ✅ |
| a3s-power | 888 | — | ✅ |
| a3s-gateway | 625 | — | ✅ |
| a3s-search | 267 | — | ✅ |
| a3s-lane | 230 | 96% line | ✅ |
| a3s-context | 114 | — | ✅ |
| a3s-event | 83 | — | ✅ |
| a3s-cron | 79 | — | ✅ |
| a3s-tools | 51 | — | ✅ |
| a3s-tools-core | 32 | — | ✅ |
| a3s-updater | 8 | — | ✅ |

```bash
just test       # Run all workspace tests
just test-all   # Run everything including box
```

## Roadmap

### In Progress 🚧

- [ ] **Unified Transport Layer** (P0, ~50%) — `a3s-transport` crate with `Transport` trait, frame protocol, MockTransport. Consumer migration (safeclaw TeeClient, box exec/PTY) pending.
- [ ] **MicroVM Cold Start** (P0, ~70%) — RootfsCache, LayerCache, WarmPool implemented; VM snapshot/restore pending (requires libkrun API support).
- [ ] **Gateway Integration Reversal** (P1, ~0%) — Replace SafeClaw's TOML config generation with health-based service discovery in a3s-gateway.
- [ ] **LLM Cost Dashboard** (P1, ~80%) — a3s-code complete (per-call recording, cross-session aggregation, OTLP, SigNoz dashboard); a3s-power needs aggregation endpoint.

### Completed ✅

- [x] AI Coding Agent — multi-session, 11 tools, permissions, HITL, skills, subagents, LSP, MCP, reflection, memory, planning, server-side agentic loop
- [x] Per-Session Priority Queue — 6 lanes, concurrency, retry/DLQ, rate limiting, priority boosting, metrics, OpenTelemetry
- [x] MicroVM Sandbox — VM management, OCI images, Docker CLI (29 commands), WarmPool, CRI, TEE, networking, volumes
- [x] OS Main Application — 7 channel adapters, multi-agent coordination, privacy escalation, A3sfile DSL, Tauri desktop UI
- [x] OS External Gateway — reverse proxy, 10 middlewares, 7-platform webhooks, privacy routing, token metering, TLS/ACME
- [x] Local LLM Engine — Ollama + OpenAI compatible API, llama.cpp backend, multi-model, multi-GPU, tool calling, cost tracking
- [x] Meta Search Engine — 8 engines, consensus ranking, proxy pool, async parallel search
- [x] Hierarchical Context — pathway URI, multi-level digests, namespace system, embedding, 4 reranker providers, OpenTelemetry
- [x] Event System — pluggable pub/sub with NATS JetStream and in-memory providers, AES-256-GCM payload encryption, state persistence, observability
- [x] Cron Scheduling — standard cron + natural language (EN/CN), pluggable storage, execution history, OpenTelemetry
- [x] OpenTelemetry Cross-Crate — structured spans and OTLP metrics in a3s-cron, a3s-lane, a3s-context, a3s-event
- [x] SDKs — Python & TypeScript with full 85 RPC coverage, unified skill API, aligned high-level Session API (`send()`, `stream()`, `delegate()`)
- [x] Deep Research Agent — iterative research with interactive steering, workspace persistence, pluggable output formats
- [x] Infrastructure — GitHub Actions CI/CD, crates.io publishing, Homebrew tap
- [x] Session Merge, Shared Privacy Types, Security Module Rename, Box Networking, Box Volumes, Box Registry Push, Box Resource Limits, Box Dockerfile Completion

### Planned

- [ ] Box TeeRuntime API — high-level `spawn_verified()` combining VM boot + attestation + secure channel (P1)
- [ ] Runtime Security Audit — NATS Stream audit pipeline + drift detection + panic elimination (P1)
- [ ] Box Logging Drivers — json-file/syslog/journald drivers, log rotation, structured JSON output (P2)
- [ ] Box Security Hardening — Seccomp profiles, Linux capabilities, read-only rootfs, no-new-privileges (P2)
- [ ] Distributed Scheduling — multi-node job distribution with leader election (P2)
- [ ] ML-based Search Ranking — learning-to-rank for result quality (P2)
- [ ] Distributed Queue Backend — real multi-machine backend (Redis/NATS) for a3s-lane (P2)
- [ ] Context Remote Storage — remote storage backend + session persistence for a3s-context (P2)

See each crate's README for detailed per-component roadmaps.

## Repository Structure

```
a3s/
├── Cargo.toml              # Workspace definition
├── justfile                # Build commands
├── README.md
├── crates/
│   ├── box/                # [submodule] MicroVM sandbox runtime (runs SafeClaw)
│   ├── code/               # [submodule] AI coding agent (orchestrated by SafeClaw)
│   │   └── sdk/            #   Python & TypeScript SDKs
│   ├── cron/               # [submodule] Cron scheduling library
│   ├── event/              # [submodule] Pluggable event system
│   ├── gateway/            # [submodule] OS external gateway
│   ├── lane/               # [submodule] Per-session priority queue (used by a3s-code)
│   ├── context/            # [submodule] Context management
│   ├── power/              # [submodule] Local LLM inference engine
│   ├── safeclaw/           # [submodule] OS main application (multi-agent coordination)
│   ├── safeclaw-ui/        # [submodule] SafeClaw desktop UI (React + Tauri)
│   ├── search/             # [submodule] Meta search engine
│   │   └── sdk/            #   Python & Node.js SDKs
│   ├── privacy/            # Shared PII classification types
│   ├── transport/          # Shared vsock transport protocol
│   ├── tools/              # Built-in tools binary
│   ├── tools-core/         # Core types for tools
│   └── updater/            # Self-update via GitHub Releases
├── a3s-deep/               # Agentic deep research agent (TypeScript)
│   ├── .a3s/               #   Project config, skills, agents
│   └── src/                #   Agent source (planner, analyzer, synthesizer, etc.)
└── os/                     # [submodule] Agent OS platform
    ├── src/apps/cli/       #   A3S CLI (a3s up/deploy/logs)
    ├── src/apps/api/       #   Platform API (NestJS)
    ├── src/apps/ui/        #   Platform UI (React, Super Factory 3D visualization)
    ├── python/agents/      #   Agent templates with A3sfile
    ├── docs/architecture/  #   A3sfile DSL spec
    └── infra/dev/helm/     #   Kubernetes Helm charts
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

## License

MIT License - see [LICENSE](LICENSE) for details.

---

<p align="center">
  Built by <a href="https://github.com/a3s-lab">A3S Lab</a>
</p>

