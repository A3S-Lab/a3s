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
a3s-box (VM runtime — standalone CLI or K8s RuntimeClass)
  └── MicroVM (TEE hardware encryption when available, VM isolation always)
      ├── SafeClaw (security proxy — classify, sanitize, audit)
      └── Your Agent (built with a3s-code framework + a3s-lane scheduling)

a3s-gateway (K8s Ingress Controller — routes traffic, app-agnostic)
```

**A3S Code** is a coding agent framework — not a standalone service. Import it as a library
(`a3s-code-core`) and build agents with `Agent::new("agent.hcl")` or `Agent::from_config(config)`.
All subsystems (tools, hooks, security, memory, MCP, planning, subagents) are embedded
in the library and active by default. Complex tasks are decomposed into dependency graphs and
independent steps execute **in parallel** via wave-based scheduling (`tokio::JoinSet`).

**A3S Gateway** and **A3S Box** are the two infrastructure components. They are application-agnostic — they don't know or care what runs inside the VM.

## Architecture

```
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
                │  TLS/ACME · Auth · Rate Limit · CORS         │
                │  Privacy Routing · Load Balancing             │
                │  App-agnostic: doesn't know what's behind it  │
                └──────────────────┬───────────────────────────┘
                                   │
                ┌──────────────────▼───────────────────────────┐
                │           a3s-box MicroVM (v0.4.0)            │
                │  VM isolation always · TEE (SEV-SNP / TDX)    │
                │                                              │
                │  ┌────────────────────────────────────────┐  │
                │  │       SafeClaw (security proxy)         │  │
                │  │  Channels(7) · Classify · Inject Detect │  │
                │  │  Taint Track · Output Sanitize · Audit  │  │
                │  │  TeeRuntime (self-detect /dev/sev-guest) │  │
                │  └──────────────────┬─────────────────────┘  │
                │                     │ library API              │
                │  ┌──────────────────▼─────────────────────┐  │
                │  │  Your Agent (built with a3s-code)        │  │
                │  │  Agent::new() · Tools · LLM Calls          │  │
                │  │  a3s-lane scheduling · Skills · Memory   │  │
                │  └────────────────────────────────────────┘  │
                └──────────────────────────────────────────────┘
                       │              │
                 ┌─────▼────┐  ┌─────▼────┐
                 │ a3s-power│  │a3s-search│
                 │ LLM Eng. │  │ Search   │
                 └──────────┘  └──────────┘

  Shared: a3s-common (PII classification, tools, transport) · a3s-transport (vsock framing)
  Observability: OpenTelemetry spans · Prometheus metrics · SigNoz dashboards
```

| Layer | Component | Role |
|-------|-----------|------|
| Ingress | a3s-gateway | K8s Ingress Controller: TLS, auth, privacy routing, load balancing, token metering |
| VM Runtime | a3s-box | MicroVM isolation + TEE (SEV-SNP/TDX), 52-command CLI, CRI for K8s, Prometheus metrics, audit logging |
| Security Proxy | SafeClaw | 7-channel routing, privacy classification, injection detection, taint tracking, output sanitization, audit |
| Agent Framework | a3s-code | Embeddable library: config-driven Agent/AgentSession, 14 tools, skills, subagents, memory, parallel plan execution |
| Scheduling | a3s-lane | Per-session priority queue: 6 lanes, concurrency, retry, dead letter |
| Infrastructure | a3s-power / a3s-search | LLM inference / meta search |
| Shared | a3s-common | PII classification, tool types & redaction / vsock frame protocol |

## Projects

| Project | Description | Docs |
|---------|-------------|------|
| [a3s-code](crates/code/) | AI coding agent framework — parallel plan execution, 14 tools, skills, subagents, memory | [README](crates/code/README.md) |
| [a3s-lane](crates/lane/) | Per-session priority queue — 6 lanes, concurrency, retry/DLQ | [README](crates/lane/README.md) |
| [a3s-box](crates/box/) | MicroVM sandbox runtime — VM isolation + TEE (SEV-SNP/TDX), Docker-like CLI (52 commands), CRI for K8s, 1,466 tests | [README](crates/box/README.md) |
| [SafeClaw](crates/safeclaw/) | Security proxy — privacy classification, taint tracking, injection detection | [README](crates/safeclaw/README.md) |
| [a3s-gateway](crates/gateway/) | K8s Ingress Controller — reverse proxy, middlewares, privacy routing | [README](crates/gateway/README.md) |
| [a3s-power](crates/power/) | Local LLM inference engine — Ollama + OpenAI compatible API | [README](crates/power/README.md) |
| [a3s-search](crates/search/) | Meta search engine — 8 engines, consensus ranking | [README](crates/search/README.md) |
| [a3s-event](crates/event/) | Pluggable event system — provider-agnostic pub/sub, encryption | [README](crates/event/README.md) |
| [a3s-updater](crates/updater/) | Self-update for CLI binaries via GitHub Releases | [Source](crates/updater/) |

## License

MIT — see [LICENSE](LICENSE).

---

<p align="center">
  Built by <a href="https://github.com/a3s-lab">A3S Lab</a>
</p>
