# A3S

<p align="center">
  <strong>Agentic Adaptive Augmentation System</strong>
</p>

<p align="center">
  <em>An Agent Operating System — VM-isolated execution, privacy-aware security proxy, and agentic evolution</em>
</p>

---

## Overview

**A3S** is an **Agent Operating System**. It provides the full stack for declaring, packaging, deploying, securing, and evolving AI agents at scale.

```
a3s                  ← local dev orchestration + unified CLI for the A3S ecosystem
a3s-box              ← MicroVM runtime (standalone CLI or K8s RuntimeClass)
  └── MicroVM        ← TEE hardware encryption when available, VM isolation always
      ├── SafeClaw   ← security proxy: classify, sanitize, audit
      ├── a3s-code   ← imperative agent framework: Agent::new(), 14 tools, LLM, memory
      ├── a3s-flow   ← workflow engine: FlowEngine · JSON DAG · pause/resume/terminate
      │     └── FlowEngine.start() / pause() / resume() / terminate() / node_types()
      └── a3s-lane   ← shared scheduling for a3s-code and a3s-flow
a3s-gateway          ← K8s Ingress Controller: routes traffic, app-agnostic
```

**a3s** is the developer CLI. One binary to start services, manage dependencies, deploy k3s, proxy to ecosystem tools, and scaffold agents — all from a single `A3sfile.hcl`.

**a3s-code** is a coding agent framework — not a standalone service. Import it as a library (`a3s-code-core`) and build agents with `Agent::new("agent.hcl")`. All subsystems (tools, hooks, security, memory, MCP, planning, subagents) are embedded and active by default. Complex tasks are decomposed into dependency graphs and independent steps execute **in parallel** via wave-based scheduling.

**a3s-flow** is a workflow engine for declarative, DAG-driven agentic pipelines. Define workflows as JSON arrays of typed nodes with explicit `depends_on` edges. `FlowEngine` validates the graph, executes waves of independent nodes concurrently, and exposes full lifecycle control — start, pause, resume, and terminate — for each running execution. Designed to power platforms similar to Dify and n8n.

**a3s-gateway** and **a3s-box** are infrastructure components. They are application-agnostic — they don't know or care what runs inside the VM.

## Architecture

```
  Developer Machine
  ┌─────────────────────────────────────────────────────────────┐
  │  a3s up / a3s box / a3s kube / a3s gateway                  │
  │  (unified CLI — auto-installs tools, manages services)      │
  └─────────────────────────────────────────────────────────────┘

                   External Traffic (Internet / Messaging Platforms)
        ┌───────┬───────┬───────┬───────┬───────┬──────────┐
        │Telegram│ Slack │ Feishu│DingTalk│WebChat│ HTTP/gRPC│
        └───┬───┴───┬───┴───┬───┴───┬───┴───┬───┴────┬─────┘
            └───────┴───────┴───────┴───────┘        │
                            │                        │
     Standalone: direct     │     A3S OS: via Ingress│
                            │                        │
              ┌─────────────▼────────────────────────▼───────┐
              │            a3s-gateway (optional)             │
              │         K8s Ingress Controller                │
              │  TLS/ACME · Auth · Rate Limit · CORS          │
              │  Privacy Routing · Load Balancing             │
              └──────────────────┬────────────────────────────┘
                                 │
              ┌──────────────────▼────────────────────────────┐
              │              a3s-box MicroVM                   │
              │  VM isolation always · TEE (SEV-SNP / TDX)    │
              │                                                │
              │  ┌──────────────────────────────────────────┐ │
              │  │         SafeClaw (security proxy)         │ │
              │  │  7 Channels · Classify · Inject Detect    │ │
              │  │  Taint Track · Output Sanitize · Audit    │ │
              │  └──────────────────┬───────────────────────┘ │
              │                     │ library API              │
              │  ┌──────────────────▼───────────────────────┐ │
              │  │              Your Agent                   │ │
              │  │                                          │ │
              │  │  ┌─────────────────────────────────┐    │ │
              │  │  │ a3s-code  (imperative agents)   │    │ │
              │  │  │ Agent::new() · 14 Tools · LLM   │    │ │
              │  │  │ Skills · Memory · Subagents      │    │ │
              │  │  └─────────────────────────────────┘    │ │
              │  │                                          │ │
              │  │  ┌─────────────────────────────────┐    │ │
              │  │  │ a3s-flow  (workflow agents)      │    │ │
              │  │  │ FlowEngine · JSON DAG · pause /  │    │ │
              │  │  │ resume / terminate · node types  │    │ │
              │  │  └─────────────────────────────────┘    │ │
              │  │                                          │ │
              │  │  a3s-lane scheduling (shared)            │ │
              │  └──────────────────────────────────────────┘ │
              └────────────────────────────────────────────────┘
                     │              │              │
               ┌─────▼────┐  ┌─────▼────┐  ┌─────▼────┐
               │ a3s-power│  │a3s-search│  │ a3s-event│
               │ LLM Eng. │  │ Search   │  │  pub/sub │
               └──────────┘  └──────────┘  └──────────┘

  Shared: a3s-common (PII classification, tools, transport)
  Observability: OpenTelemetry spans · Prometheus metrics
```

| Layer | Component | Role |
|-------|-----------|------|
| Developer CLI | a3s | Service orchestration, brew deps, k3s, tool proxy, agent scaffolding |
| Ingress | a3s-gateway | K8s Ingress Controller: TLS, auth, privacy routing, load balancing |
| VM Runtime | a3s-box | MicroVM isolation + TEE (SEV-SNP/TDX), 52-command CLI, CRI for K8s |
| Security Proxy | SafeClaw | 7-channel routing, privacy classification, injection detection, taint tracking, audit |
| Agent Framework | a3s-code | Embeddable library: config-driven Agent, 14 tools, skills, subagents, memory, parallel execution |
| Workflow Engine | a3s-flow | JSON DAG workflow engine: FlowEngine lifecycle API (start / pause / resume / terminate), concurrent wave execution, pluggable node types |
| Scheduling | a3s-lane | Per-session priority queue: 6 lanes, concurrency, retry, dead letter |
| Infrastructure | a3s-power / a3s-search / a3s-event | LLM inference / meta search / pub-sub events |
| Shared | a3s-common | PII classification, tool types, vsock frame protocol |

## Projects

| Project | Version | Description | Docs |
|---------|---------|-------------|------|
| [a3s](crates/dev/) | 0.1.0 | Developer CLI — service orchestration, brew deps, k3s, tool proxy, agent scaffolding | [README](crates/dev/README.md) |
| [a3s-box](crates/box/) | 0.6.0 | MicroVM sandbox runtime — VM isolation + TEE, Docker-like CLI (52 commands), CRI for K8s | [README](crates/box/README.md) |
| [a3s-code](crates/code/) | 0.8.0 | AI coding agent framework — parallel plan execution, 14 tools, skills, subagents, memory | [README](crates/code/README.md) |
| [SafeClaw](apps/safeclaw/) | 0.1.0 | Secure personal AI assistant — TEE support, desktop app (Tauri), embedded gateway | [README](apps/safeclaw/crates/safeclaw/README.md) |
| [a3s-gateway](crates/gateway/) | 0.1.0 | K8s Ingress Controller — reverse proxy, middlewares, privacy routing | [README](crates/gateway/README.md) |
| [a3s-lane](crates/lane/) | 0.4.0 | Per-session priority queue — 6 lanes, concurrency, retry/DLQ | [README](crates/lane/README.md) |
| [a3s-power](crates/power/) | 0.2.0 | Local LLM inference engine — Ollama + OpenAI compatible API | [README](crates/power/README.md) |
| [a3s-search](crates/search/) | 0.8.0 | Meta search engine — 8 engines, consensus ranking | [README](crates/search/README.md) |
| [a3s-event](crates/event/) | 0.3.0 | Pluggable event system — provider-agnostic pub/sub, encryption | [README](crates/event/README.md) |
| [a3s-flow](crates/flow/) | 0.1.0 | Workflow engine — JSON DAG execution for agentic workflows (Dify/n8n style) | [README](crates/flow/README.md) |
| [a3s-updater](crates/updater/) | 0.2.0 | Self-update for CLI binaries via GitHub Releases | [Source](crates/updater/) |

## Quick start

```bash
# Install the a3s CLI
brew install a3s-lab/tap/a3s

# Create an A3sfile.hcl and start services
a3s init
a3s up

# Use a3s-box (auto-installed if missing)
a3s box run ubuntu:24.04 -- bash

# Deploy a local k3s cluster
a3s kube start
kubectl get nodes

# Scaffold an agent project
a3s code init ./my-agent
```

## Repository structure

```
a3s/
├── apps/
│   ├── os/              # A3S platform (NestJS + React + CLI)
│   └── safeclaw/        # SafeClaw desktop app (Tauri + React + embedded gateway)
├── crates/
│   ├── box/             # a3s-box MicroVM runtime
│   ├── code/            # a3s-code agent framework
│   ├── common/          # Shared types: PII classification, tools, transport
│   ├── dev/             # a3s developer CLI
│   ├── event/           # a3s-event pub/sub system
│   ├── flow/            # a3s-flow workflow engine (JSON DAG)
│   ├── gateway/         # a3s-gateway K8s Ingress Controller
│   ├── lane/            # a3s-lane scheduling
│   ├── memory/          # a3s-memory long-term memory
│   ├── power/           # a3s-power LLM inference
│   ├── search/          # a3s-search meta search
│   └── updater/         # a3s-updater self-update
├── docs/                # Architecture diagrams
└── homebrew-tap/        # Homebrew formula
```

## Community

Join us on [Discord](https://discord.gg/XVg6Hu6H) for questions, discussions, and updates.

## License

MIT — see [LICENSE](LICENSE).

---

<p align="center">
  Built by <a href="https://github.com/a3s-lab">A3S Lab</a>
</p>
