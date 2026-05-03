# A3S

<p align="center">
  <strong>Agentic Adaptive Augmentation System</strong>
</p>

<p align="center">
  <em>An Agent Operating System — VM-isolated execution, embeddable coding agents, and agentic evolution</em>
</p>

---

## Overview

**A3S** is an **Agent Operating System**. It provides the full stack for declaring, packaging, deploying, securing, and evolving AI agents at scale.

```
a3s                  ← local dev orchestration + unified CLI for the A3S ecosystem
a3s-box              ← MicroVM runtime (standalone CLI or K8s RuntimeClass)
  └── MicroVM        ← TEE hardware encryption when available, VM isolation always
      ├── a3s-code   ← harness-driven agent runtime: ACL, tools, PTC, LLM, memory
      └── a3s-lane   ← shared scheduling for a3s-code
a3s-gateway          ← K8s Ingress Controller: routes traffic, app-agnostic
```

**a3s** is the developer CLI. One binary to start services, manage dependencies, deploy k3s, proxy to ecosystem tools, and scaffold agents — all from a single `A3sfile.hcl`.

**a3s-code** is a coding agent runtime — not a standalone service. Import it as a library (`a3s-code-core`, `a3s-code`, or `@a3s-lab/code`) and build agents from ACL config with `Agent::new("agent.acl")`. Tools, hooks, security policy, memory, MCP, explicit planning mode, run replay, QuickJS PTC, and task delegation are exposed through the runtime and SDKs. Complex deterministic workflows can move out of repeated LLM tool loops into bounded programs or delegated child runs.

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
              │  │              Your Agent                   │ │
              │  │                                          │ │
              │  │  ┌─────────────────────────────────┐    │ │
              │  │  │ a3s-code  (agent runtime)       │    │ │
              │  │  │ ACL · SDKs · Tools · PTC · LLM   │    │ │
              │  └──────────────────────────────────────────┘ │
              └────────────────────────────────────────────────┘
                     │              │              │
               ┌─────▼────┐  ┌─────▼────┐
               │ a3s-power│  │a3s-search│
               │ LLM Eng. │  │ Search   │
               └──────────┘  └──────────┘

  Shared: a3s-common (PII classification, tools, transport)
  Observability: OpenTelemetry spans · Prometheus metrics
```

| Layer | Component | Role |
|-------|-----------|------|
| Developer CLI | a3s | Service orchestration, brew deps, k3s, tool proxy, agent scaffolding |
| Ingress | a3s-gateway | K8s Ingress Controller: TLS, auth, privacy routing, load balancing |
| VM Runtime | a3s-box | MicroVM isolation + TEE (SEV-SNP/TDX), 52-command CLI, CRI for K8s |
| Agent Framework | a3s-code | Embeddable library: ACL config, Rust/Node/Python SDKs, tools, PTC, skills, task delegation, memory, parallel execution |
| Scheduling | a3s-lane | Per-session priority queue: 6 lanes, concurrency, retry, dead letter |
| Infrastructure | a3s-power / a3s-search | LLM inference / meta search |
| Shared | a3s-common | PII classification, tool types, vsock frame protocol |

## Projects

| Project | Version | Description | Docs |
|---------|---------|-------------|------|
| [a3s](crates/cli/) | 0.1.4 | CLI — service orchestration, brew deps, k3s, tool proxy, agent scaffolding | [README](crates/cli/README.md) |
| [a3s-box](crates/box/) | 0.8.8 | MicroVM sandbox runtime — VM isolation + TEE, Docker-like CLI (52 commands), CRI for K8s, no_fsync optimization | [README](crates/box/README.md) |
| [a3s-code](crates/code/) | 2.1.0 | Harness-driven coding agent runtime — ACL config, Rust/Node/Python SDKs, explicit planning mode, run replay, QuickJS PTC, task delegation, memory | [README](crates/code/README.md) |
| [a3s-gateway](crates/gateway/) | 0.2.3 | K8s Ingress Controller — reverse proxy, middlewares, privacy routing | [README](crates/gateway/README.md) |
| [a3s-lane](crates/lane/) | 0.4.0 | Per-session priority queue — 6 lanes, concurrency, retry/DLQ | [README](crates/lane/README.md) |
| [a3s-power](crates/power/) | 0.4.2 | Local LLM inference engine — Ollama + OpenAI compatible API | [README](crates/power/README.md) |
| [a3s-search](crates/search/) | 0.8.0 | Meta search engine — 8 engines, consensus ranking | [README](crates/search/README.md) |
| [a3s-memory](crates/memory/) | 0.1.1 | Long-term memory system — persistent agent memory across sessions | [README](crates/memory/README.md) |
| [a3s-ahp](crates/ahp/) | 0.1.0 | Agent Harness Protocol — universal protocol for supervising autonomous AI agents | [README](crates/ahp/README.md) |
| [a3s-updater](crates/updater/) | 0.2.0 | Self-update for CLI binaries via GitHub Releases | [Source](crates/updater/) |
| [a3s-faas](crates/faas/) | 0.1.0 | Serverless execution engine with MicroVM isolation | [README](crates/faas/README.md) |
| [a3s-lambda](crates/lambda/cli/) | 0.1.0 | K8s-compatible REST API — 20+ resource types, SSE watch, SQLite state store | [Source](crates/lambda/cli/) |

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
│   └── os/              # A3S platform (NestJS + React + CLI)
├── crates/
│   ├── box/             # a3s-box MicroVM runtime
│   ├── code/            # a3s-code agent framework
│   ├── common/          # Shared types: PII classification, tools, transport
│   ├── dev/             # a3s developer CLI
│   ├── faas/            # a3s-faas serverless execution engine
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
