# A3S

<p align="center">
  <strong>Agentic Adaptive Augmentation System</strong>
</p>

<p align="center">
  <em>An agent infrastructure stack: orchestration, VM-isolated execution, embeddable coding agents, and gateway/runtime components.</em>
</p>

---

## Overview

**A3S** is a monorepo for agent infrastructure. It contains the developer CLI, a MicroVM runtime, an embeddable coding-agent runtime, a gateway, scheduling, memory, search, and LLM-serving components.

```text
a3s                  <- local developer orchestration and ecosystem CLI
a3s-box              <- Docker-like MicroVM runtime for Linux OCI workloads
  └── MicroVM        <- VM isolation always; SEV-SNP TEE only on capable hosts
      ├── a3s-code   <- harness-driven agent runtime
      └── a3s-lane   <- scheduling primitives
a3s-gateway          <- application-agnostic ingress/reverse proxy layer
```

**a3s** is the developer CLI. It starts services, manages dependencies, deploys k3s, proxies to ecosystem tools, and scaffolds agents from a single `A3sfile.hcl`.

**a3s-code** is a coding-agent runtime library. It exposes ACL config, tools, hooks, security policy, memory, MCP, structured output (`generate_object`), explicit planning mode, run replay, QuickJS PTC, and task delegation through runtime APIs and SDKs.

**a3s-box** is a MicroVM runtime. Its local Docker-like CLI is the primary product surface today. Kubernetes CRI, hardware TEE, and Windows paths exist but are integration surfaces with explicit platform limits, not blanket production guarantees.

## Architecture

```text
Developer Machine
  a3s up / a3s box / a3s kube / a3s gateway
       |
       v
A3S Gateway (optional)
  TLS / auth / privacy routing / load balancing
       |
       v
A3S Box MicroVM
  VM isolation, optional SEV-SNP TEE on supported hardware
       |
       v
Your workload
  a3s-code agent runtime, web service, worker, or any supported Linux OCI image
```

| Layer | Component | Role |
| --- | --- | --- |
| Developer CLI | a3s | Service orchestration, dependency setup, k3s, tool proxy, agent scaffolding |
| Ingress | a3s-gateway | Reverse proxy and Kubernetes ingress integration |
| VM Runtime | a3s-box | MicroVM isolation, Docker-like CLI, OCI image lifecycle, experimental CRI |
| Agent Framework | a3s-code | Embeddable coding-agent runtime and SDKs |
| Scheduling | a3s-lane | Per-session priority queues and retry/dead-letter behavior |
| Infrastructure | a3s-power / a3s-search | LLM inference and meta search |
| Shared | a3s-common | Shared primitives and transport types |

## Projects

| Project | Version | Description | Docs |
| --- | --- | --- | --- |
| [a3s](crates/cli/) | 0.1.4 | CLI for orchestration, dependencies, k3s, tool proxy, and scaffolding | [README](crates/cli/README.md) |
| [a3s-box](crates/box/) | 2.0.4 | Docker-like MicroVM runtime for Linux OCI workloads; local CLI is primary, CRI/TEE/Windows paths are platform-gated | [README](crates/box/README.md) |
| [a3s-code](crates/code/) | 2.4.0 | Harness-driven coding-agent runtime with ACL config, SDKs, structured output, planning, run replay, PTC, delegation, and memory | [README](crates/code/README.md) |
| [a3s-gateway](crates/gateway/) | 0.2.3 | Kubernetes ingress/reverse proxy with middleware and privacy routing | [README](crates/gateway/README.md) |
| [a3s-lane](crates/lane/) | 0.4.0 | Priority queues with lanes, concurrency, retry, and DLQ | [README](crates/lane/README.md) |
| [a3s-power](crates/power/) | 0.4.2 | Local LLM inference engine with OpenAI-compatible API | [README](crates/power/README.md) |
| [a3s-search](crates/search/) | 0.8.0 | Meta search engine with consensus ranking | [README](crates/search/README.md) |
| [a3s-memory](crates/memory/) | 0.1.1 | Long-term memory storage for agents | [README](crates/memory/README.md) |
| [a3s-ahp](crates/ahp/) | 0.1.0 | Agent Harness Protocol primitives | [README](crates/ahp/README.md) |
| [a3s-updater](crates/updater/) | 0.2.0 | Self-update for CLI binaries via GitHub Releases | [Source](crates/updater/) |
| [a3s-faas](crates/faas/) | 0.1.0 | Serverless execution engine with MicroVM integration | [README](crates/faas/README.md) |
| [a3s-lambda](crates/lambda/cli/) | 0.1.0 | K8s-compatible REST API with SQLite state store | [Source](crates/lambda/cli/) |

## Quick start

```bash
# Install the a3s CLI
brew install a3s-lab/tap/a3s

# Create an A3sfile.hcl and start services
a3s init
a3s up

# Use a3s-box through the ecosystem CLI
a3s box run ubuntu:24.04 -- bash

# Deploy a local k3s cluster
a3s kube start
kubectl get nodes

# Scaffold an agent project
a3s code init ./my-agent
```

## Repository structure

```text
a3s/
├── apps/
│   └── docs/            # Documentation site
├── crates/              # Rust crates as submodules
│   ├── box/             # a3s-box MicroVM runtime
│   ├── code/            # a3s-code agent framework
│   ├── common/          # Shared types
│   ├── gateway/         # a3s-gateway
│   ├── lane/            # a3s-lane scheduling
│   ├── memory/          # a3s-memory
│   ├── power/           # a3s-power
│   ├── search/          # a3s-search
│   └── updater/         # a3s-updater
└── homebrew-tap/        # Homebrew formulae
```

## Community

Join us on [Discord](https://discord.gg/XVg6Hu6H) for questions, discussions, and updates.

## License

MIT — see [LICENSE](LICENSE).
