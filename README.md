# A3S

<p align="center">
  <strong>Autonomous Agent Adaptive System</strong>
</p>

<p align="center">
  <em>A modular ecosystem for building and running secure AI agents</em>
</p>

---

## Overview

A3S is a collection of Rust crates that work together to provide a complete infrastructure for AI agents:

```
┌─────────────────────────────────────────────────────────────┐
│                      A3S Ecosystem                          │
│                                                             │
│  Infrastructure    a3s-box       MicroVM sandbox runtime    │
│  (基础设施层)         │          (hardware isolation)        │
│                       │                                     │
│  Application       a3s-code      AI coding agent            │
│  (应用层)            / \         (runs inside box)           │
│                     /   \                                   │
│  Utility        a3s-lane  a3s-context                       │
│  (工具层)        (queue)   (memory/knowledge)                │
└─────────────────────────────────────────────────────────────┘
```

## Crates

| Crate | Description | crates.io | Tests |
|-------|-------------|-----------|-------|
| [`a3s-lane`](crates/lane) | Priority-based command queue for async task scheduling | [![crates.io](https://img.shields.io/crates/v/a3s-lane.svg)](https://crates.io/crates/a3s-lane) | - |
| [`a3s_context`](crates/context) | Hierarchical context management for AI memory/knowledge | [![crates.io](https://img.shields.io/crates/v/a3s_context.svg)](https://crates.io/crates/a3s_context) | - |
| [`a3s-code`](crates/code) | AI coding agent with tool execution | [![crates.io](https://img.shields.io/crates/v/a3s-code.svg)](https://crates.io/crates/a3s-code) | 359 ✅ |
| [`a3s-box-core`](crates/box) | Core types for MicroVM sandbox | [![crates.io](https://img.shields.io/crates/v/a3s-box-core.svg)](https://crates.io/crates/a3s-box-core) | - |
| [`a3s-box-runtime`](crates/box) | MicroVM sandbox runtime | [![crates.io](https://img.shields.io/crates/v/a3s-box-runtime.svg)](https://crates.io/crates/a3s-box-runtime) | - |
| [`a3s-tools-core`](crates/tools-core) | Core types for tool execution | [![crates.io](https://img.shields.io/crates/v/a3s-tools-core.svg)](https://crates.io/crates/a3s-tools-core) | - |
| [`a3s-tools`](crates/tools) | Built-in tools binary (bash, read, write, edit, grep, glob, ls) | [![crates.io](https://img.shields.io/crates/v/a3s-tools.svg)](https://crates.io/crates/a3s-tools) | - |

## Quick Start

### Clone with Submodules

```bash
git clone --recursive https://github.com/a3s-lab/a3s.git
cd a3s

# Or if already cloned:
git submodule update --init --recursive
```

### Build

```bash
# Build workspace crates (lane, code, context)
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

### Publish

```bash
# Dry run (verify all packages)
just publish-dry

# Publish all crates to crates.io
just publish

# Publish single crate
just publish-crate a3s-lane
```

## Repository Structure

```
a3s/
├── Cargo.toml          # Workspace definition
├── justfile            # Build commands
├── README.md
└── crates/
    ├── box/            # [submodule] MicroVM sandbox runtime
    ├── code/           # [submodule] AI coding agent
    ├── lane/           # [submodule] Priority command queue
    └── context/        # [submodule] Context management
```

Each crate is maintained in its own Git repository but can be developed together through this workspace.

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
| `just publish-dry` | Verify all crates (dry run) |
| `just version` | Show all crate versions |
| `just update-submodules` | Update submodules to latest |

## License

MIT
