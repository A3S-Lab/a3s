# A3S Box Architecture

## Overview

A3S Box provides two architectural approaches for running coding agents and business code:

1. **Current Architecture (Phase 1-2)**: Single VM + file mount - Simple and lightweight
2. **Target Architecture (Phase 3+)**: OCI Images + Linux Namespaces - Strong isolation with standard container format

This document describes both architectures and the migration path between them.

---

## Current Architecture: Single VM + File Mount

### Overview

The current implementation uses a **single-container + file mount** architecture:

1. **Coding Agent Container** - Pluggable coding agents (A3S Code, OpenCode, etc.)
2. **Business Agents** - Mounted as skills via virtio-fs

This design simplifies the architecture, reduces resource overhead, while maintaining flexibility.

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│  Coding Agent Container (VM)                                     │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Coding Agent (A3S Code / OpenCode / Custom)              │  │
│  │  - gRPC Server (4088)                                    │  │
│  │  - Built-in Tools (bash, read, write, edit, grep, glob)  │  │
│  │  - Skill System                                          │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Mounted Business Agents (via virtio-fs)                  │  │
│  │  /a3s/skills/                                            │  │
│  │    ├── order-agent/                                      │  │
│  │    │   ├── SKILL.md          (Skill definition)          │  │
│  │    │   ├── tools/            (Custom tools)              │  │
│  │    │   └── prompts/          (Prompt templates)          │  │
│  │    ├── data-agent/                                       │  │
│  │    └── ...                                               │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

**Architecture Benefits**:
- Only one VM needed, low resource overhead
- No inter-container communication required
- Fast startup time
- Simple configuration
- Seamless integration of business logic with coding capabilities

## Core Concepts

### 1. Coding Agent

The coding agent is the core of A3S Box, responsible for:
- Code generation and editing
- Tool execution
- Skill loading and management
- Session management

Supported coding agent types:
- `a3s_code` - A3S Code (default)
- `opencode` - OpenCode
- `oci_image` - Custom OCI image
- `local_binary` - Local binary
- `remote_binary` - Remote binary

### 2. Business Agent

Business agents are integrated into the coding agent via **file mounting**:

```
Host Filesystem                      Inside Container
/path/to/my-agent/          →    /a3s/skills/my-agent/
  ├── SKILL.md                     ├── SKILL.md
  ├── tools/                       ├── tools/
  │   ├── process_order.py         │   ├── process_order.py
  │   └── validate_data.py         │   └── validate_data.py
  └── prompts/                     └── prompts/
      └── system.md                    └── system.md
```

### 3. Skill System

Business agents run as **Skills**:

```yaml
# SKILL.md
---
name: order-agent
description: Order processing agent
version: 1.0.0
author: MyCompany

# Custom tools
tools:
  - name: process_order
    description: Process an order
    script: tools/process_order.py
    parameters:
      - name: order_id
        type: string
        required: true

  - name: validate_data
    description: Validate data
    script: tools/validate_data.py
    parameters:
      - name: data
        type: object
        required: true

# System prompt
system_prompt: prompts/system.md

# Required built-in tools
requires:
  - bash
  - read_file
  - write_file
---

# Order Agent

This is an order processing agent that can process orders, validate data, etc.
```

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     User Application                            │
│                                                                 │
│  from a3s_box import create_box, BoxConfig, AgentConfig        │
│                                                                 │
│  box = await create_box(BoxConfig(                             │
│      coding_agent=AgentConfig(                                 │
│          kind="a3s_code",                                      │
│          llm="/path/to/llm-config.json",                       │
│          skills_dir="/path/to/skills"                          │
│      ),                                                        │
│      skills=[                                                  │
│          SkillMount(name="custom", path="/path/to/custom")     │
│      ]                                                         │
│  ))                                                            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Python/TypeScript SDK                          │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Box                                                      │  │
│  │  - generate()        (Code generation)                   │  │
│  │  - use_skill()       (Activate skill)                    │  │
│  │  - remove_skill()    (Remove skill)                      │  │
│  │  - list_skills()     (List skills)                       │  │
│  │  - execute_tool()    (Execute tool)                      │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ gRPC over vsock:4088
┌─────────────────────────────────────────────────────────────────┐
│                    a3s-box-runtime (Rust)                       │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ BoxManager                                               │  │
│  │  - vm: VmManager           (Coding agent VM)             │  │
│  │  - skills_dir_mount        (Skills directory mount)      │  │
│  │  - skill_mounts            (Individual skill mounts)     │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ AgentRegistry                                            │  │
│  │  - Discover and load coding agents                       │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ SkillManager                                             │  │
│  │  - Manage skill mounts (directory + individual)          │  │
│  │  - Load/unload skills                                    │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ virtio-fs
┌─────────────────────────────────────────────────────────────────┐
│                      Coding Agent Container (VM)                 │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Coding Agent (A3S Code / OpenCode / Custom)              │  │
│  │                                                          │  │
│  │  Built-in Tools:                                         │  │
│  │  - bash, read_file, write_file, edit_file               │  │
│  │  - grep, glob, git_status, git_diff, git_commit         │  │
│  │  - web_search, web_fetch, ask_user                      │  │
│  │                                                          │  │
│  │  Skill System:                                           │  │
│  │  - Load skills from /a3s/skills/                        │  │
│  │  - Register custom tools defined in skills              │  │
│  │  - Apply skill system prompts                           │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Mounted Skills Directory (virtio-fs)                     │  │
│  │  /a3s/skills/                                            │  │
│  │    ├── order-agent/        (from skills_dir)            │  │
│  │    ├── data-agent/         (from skills_dir)            │  │
│  │    └── custom/             (from skills list)           │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Workspace (virtio-fs)                                    │  │
│  │  /a3s/workspace/                                         │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Workspace Crates

| Crate | Type | Purpose |
|-------|------|---------|
| `core` | lib | Foundational types: `BoxConfig`, `BoxError`, `BoxEvent`, `CommandQueue` |
| `runtime` | lib | VM lifecycle, session management, gRPC client, virtio-fs mounts |
| `code` | bin | Guest agent: LLM providers, tool execution, session management |
| `queue` | lib | `QueueManager` (builder pattern) and `QueueMonitor` (health checking) |
| `sdk/python` | cdylib | Python bindings via PyO3 |
| `sdk/typescript` | cdylib | TypeScript bindings via NAPI-RS |

## Skill Lifecycle

### 1. Mounting

When creating a Box, skill directories are mounted into the container via virtio-fs:

```
Host: /path/to/order-agent/
  ↓ virtio-fs
Container: /a3s/skills/order-agent/
```

### 2. Discovery

The coding agent scans `/a3s/skills/` directory to discover available skills:

```python
skills = await box.list_skills()
# ["order-agent", "data-agent", ...]
```

### 3. Activation

When activating a skill, the coding agent:
1. Parses the SKILL.md file
2. Registers custom tools
3. Applies the system prompt
4. Sets environment variables

```python
await box.use_skill("order-agent")
```

### 4. Usage

Once activated, the skill's tools can be called by the LLM:

```python
await box.generate("Process order #12345")
# LLM will call the process_order tool
```

### 5. Deactivation

When deactivating a skill, the coding agent:
1. Removes custom tools
2. Restores the system prompt
3. Cleans up environment variables

```python
await box.remove_skill("order-agent")
```

### Current Architecture Benefits

1. **Simplified Architecture** - Only one VM needed, reduced complexity
2. **Reduced Resources** - Lower memory and CPU usage (~2GB memory, 3s startup)
3. **Fast Startup** - Only one VM to start
4. **Seamless Integration** - Business logic integrates directly with coding capabilities
5. **Easy Development** - Business agents only need SKILL.md and tool scripts
6. **Hot Loading** - Skills can be dynamically loaded/unloaded

### Current Architecture Limitations

1. **Isolation** - Business agents run in the same VM as the coding agent
2. **Resource Sharing** - Business agents share resources with the coding agent
3. **Language Constraints** - Custom tools must be executable scripts
4. **Environment Conflicts** - Agent and business code share the same runtime environment

---

## Target Architecture: OCI Images + Linux Namespaces (Phase 3+)

### Overview

The target architecture enables **strong isolation** between coding agents and business code:

- **Each component has its own OCI image** with complete execution environment
- **Both images are loaded into a single VM** for efficiency
- **Linux Namespaces provide isolation** between the two processes
- **Standard OCI format** enables compatibility with Docker ecosystem

### Key Constraints

| Constraint | Description |
|------------|-------------|
| **One Agent per VM** | Maximum one coding agent (implementing a3s-code interface) |
| **One Business Code per VM** | Maximum one business application |
| **Independent Environments** | Each has its own complete runtime environment |
| **Single VM** | Both run in the same MicroVM for efficiency |

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         Host (macOS/Linux)                       │
│                                                                  │
│  1. Image Preparation                                            │
│  ┌──────────────────┐        ┌──────────────────┐               │
│  │ Agent OCI Image  │        │ Business OCI     │               │
│  │ agent-py:3.11    │        │ user-app:v1      │               │
│  │                  │        │                  │               │
│  │ Layers:          │        │ Layers:          │               │
│  │ - Python 3.11    │        │ - Node.js 20     │               │
│  │ - grpcio         │        │ - express        │               │
│  │ - agent.py       │        │ - app.js         │               │
│  └────────┬─────────┘        └────────┬─────────┘               │
│           │                           │                          │
│           └───────────┬───────────────┘                          │
│                       │                                          │
│  2. Image Extraction  ▼                                          │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │         OCI Image Parser & Extractor                    │    │
│  │  - Parse manifest.json                                  │    │
│  │  - Extract layers (tar.gz)                              │    │
│  │  - Merge into unified rootfs                            │    │
│  └─────────────────────────────────────────────────────────┘    │
│                       │                                          │
│                       ▼                                          │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              Merged Rootfs                              │    │
│  │  /                                                      │    │
│  │  ├── agent/          (from agent OCI image)             │    │
│  │  │   ├── venv/       (Python virtual environment)       │    │
│  │  │   └── agent.py                                       │    │
│  │  ├── workspace/      (from business OCI image)          │    │
│  │  │   ├── node_modules/                                  │    │
│  │  │   └── app.js                                         │    │
│  │  └── usr/            (shared system layer)              │    │
│  └─────────────────────────────────────────────────────────┘    │
│                       │                                          │
└───────────────────────┼──────────────────────────────────────────┘
                        │
                        ▼ Boot VM (libkrun)
┌─────────────────────────────────────────────────────────────────┐
│                    MicroVM (libkrun)                             │
│                                                                  │
│  3. Namespace Isolation                                          │
│                                                                  │
│  ┌──────────────────────────┐  ┌──────────────────────────┐    │
│  │  Namespace 1: Agent      │  │  Namespace 2: Business   │    │
│  │                          │  │                          │    │
│  │  Mount NS: /agent/       │  │  Mount NS: /workspace/   │    │
│  │  PID NS: isolated        │  │  PID NS: isolated        │    │
│  │  IPC NS: isolated        │  │  IPC NS: isolated        │    │
│  │  UTS NS: agent-host      │  │  UTS NS: business-host   │    │
│  │                          │  │                          │    │
│  │  Environment:            │  │  Environment:            │    │
│  │  PATH=/agent/venv/bin    │  │  PATH=/workspace/node/bin│    │
│  │  PYTHONPATH=/agent       │  │  NODE_PATH=/workspace/...│    │
│  │                          │  │                          │    │
│  │  Process:                │  │  Process:                │    │
│  │  python agent.py         │  │  node app.js             │    │
│  │  (listens vsock:4088)    │  │  (managed by agent)      │    │
│  └──────────────────────────┘  └──────────────────────────┘    │
│                                                                  │
│  Shared: Network namespace (enables communication)               │
│  Isolated: Mount, PID, IPC, UTS namespaces                      │
└─────────────────────────────────────────────────────────────────┘
```

### Execution Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ Step 1: Host prepares VM                                         │
│                                                                  │
│   a3s-box-runtime:                                               │
│   1. Pull agent OCI image (e.g., ghcr.io/a3s-lab/agent-py:3.11) │
│   2. Pull business OCI image (e.g., user-app:v1)                │
│   3. Extract both images to merged rootfs                        │
│   4. Boot MicroVM with merged rootfs                             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ Step 2: VM init process starts                                   │
│                                                                  │
│   /sbin/init (supervisor):                                       │
│   1. Mount virtio-fs shares                                      │
│   2. Create namespace for agent                                  │
│   3. Start agent process in isolated namespace                   │
│   4. Wait for agent to be ready (health check)                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ Step 3: Agent receives request from host                         │
│                                                                  │
│   Agent (in namespace 1):                                        │
│   1. Receive gRPC request via vsock:4088                         │
│   2. Process LLM interaction                                     │
│   3. When executing user code (bash tool):                       │
│      - Create new namespace for business code                    │
│      - Set business environment variables                        │
│      - Execute command in isolated namespace                     │
│      - Return result to host                                     │
└─────────────────────────────────────────────────────────────────┘
```

### Directory Layout (Inside VM)

```
/
├── usr/                      # Shared system runtime
│   ├── bin/
│   │   ├── python3.11
│   │   ├── node
│   │   └── ...
│   └── lib/
├── agent/                    # Agent isolated environment
│   ├── bin/
│   │   └── agent             # Agent executable (or script)
│   ├── venv/                 # Python virtual environment
│   │   ├── bin/python
│   │   └── lib/python3.11/site-packages/
│   │       ├── grpcio/
│   │       ├── anthropic/
│   │       └── ...
│   ├── node_modules/         # Node.js dependencies (if Node agent)
│   └── .env                  # Agent environment variables
├── workspace/                # Business code isolated environment
│   ├── app/                  # User application code
│   ├── venv/                 # User Python environment
│   │   └── lib/python3.11/site-packages/
│   │       ├── flask/
│   │       ├── numpy/
│   │       └── ...
│   ├── node_modules/         # User Node.js dependencies
│   │   ├── express/
│   │   └── ...
│   └── .env                  # User environment variables
└── etc/
    └── ...
```

### OCI Image Specification

#### Agent OCI Image

```dockerfile
# Dockerfile.agent (Python Agent Example)
FROM python:3.11-slim

# Install agent dependencies
WORKDIR /agent
COPY requirements.txt .
RUN python -m venv venv && \
    ./venv/bin/pip install --no-cache-dir -r requirements.txt

# Copy agent code
COPY agent.py .
COPY lib/ ./lib/

# Labels for a3s-box
LABEL a3s.type="agent"
LABEL a3s.interface="grpc"
LABEL a3s.port="4088"
LABEL a3s.runtime="python:3.11"
LABEL a3s.entrypoint="/agent/venv/bin/python /agent/agent.py"

# Entrypoint
ENTRYPOINT ["/agent/venv/bin/python", "/agent/agent.py", "--listen", "vsock://4088"]
```

```yaml
# requirements.txt
grpcio>=1.60.0
grpcio-tools>=1.60.0
anthropic>=0.18.0
openai>=1.12.0
```

#### Business Code OCI Image

```dockerfile
# Dockerfile.business (Node.js App Example)
FROM node:20-slim

# Install dependencies
WORKDIR /workspace
COPY package*.json ./
RUN npm ci --only=production

# Copy application code
COPY . .

# Labels for a3s-box
LABEL a3s.type="business"
LABEL a3s.runtime="nodejs:20"
LABEL a3s.workdir="/workspace"

# Default command (can be overridden by agent)
CMD ["node", "server.js"]
```

### Namespace Isolation Implementation

```rust
// guest/init/src/namespace.rs
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{fork, ForkResult};
use std::process::Command;

/// Namespace configuration for process isolation
pub struct NamespaceConfig {
    pub mount: bool,      // Separate filesystem view
    pub pid: bool,        // Separate process tree
    pub ipc: bool,        // Separate IPC
    pub uts: bool,        // Separate hostname
    pub net: bool,        // Separate network (usually false for communication)
}

impl Default for NamespaceConfig {
    fn default() -> Self {
        Self {
            mount: true,
            pid: true,
            ipc: true,
            uts: true,
            net: false,  // Share network for agent-business communication
        }
    }
}

/// Spawn a process in isolated namespaces
pub fn spawn_isolated(
    config: &NamespaceConfig,
    command: &str,
    args: &[&str],
    env: &[(&str, &str)],
    workdir: &str,
) -> Result<u32, Box<dyn std::error::Error>> {
    match unsafe { fork()? } {
        ForkResult::Child => {
            // Build namespace flags
            let mut flags = CloneFlags::empty();
            if config.mount { flags |= CloneFlags::CLONE_NEWNS; }
            if config.pid { flags |= CloneFlags::CLONE_NEWPID; }
            if config.ipc { flags |= CloneFlags::CLONE_NEWIPC; }
            if config.uts { flags |= CloneFlags::CLONE_NEWUTS; }
            if config.net { flags |= CloneFlags::CLONE_NEWNET; }

            // Create new namespaces
            unshare(flags)?;

            // Execute command
            let mut cmd = Command::new(command);
            cmd.args(args)
               .current_dir(workdir)
               .envs(env.iter().cloned());

            cmd.exec();
            unreachable!();
        }
        ForkResult::Parent { child } => {
            Ok(child.as_raw() as u32)
        }
    }
}
```

### Agent Bash Tool with Namespace Isolation

```rust
// code/src/tools/builtin/bash.rs
impl BashTool {
    pub async fn execute(&self, command: &str) -> Result<Output> {
        // Build business code environment
        let mut env = vec![
            ("PATH", "/workspace/venv/bin:/workspace/node_modules/.bin:/usr/bin:/bin"),
            ("HOME", "/workspace"),
        ];

        // Load user .env if exists
        if Path::new("/workspace/.env").exists() {
            let user_env = load_dotenv("/workspace/.env")?;
            env.extend(user_env);
        }

        // Add Python venv if exists
        if Path::new("/workspace/venv").exists() {
            env.push(("VIRTUAL_ENV", "/workspace/venv"));
            env.push(("PYTHONPATH", "/workspace"));
        }

        // Add Node.js modules if exists
        if Path::new("/workspace/node_modules").exists() {
            env.push(("NODE_PATH", "/workspace/node_modules"));
        }

        // Execute in isolated namespace
        let config = NamespaceConfig::default();
        let pid = spawn_isolated(
            &config,
            "bash",
            &["-c", command],
            &env,
            "/workspace",
        )?;

        // Wait for completion and collect output
        wait_for_process(pid).await
    }
}
```

### Resource Limits with Cgroups

```rust
// guest/init/src/cgroups.rs
pub struct ResourceLimits {
    pub memory_mb: u32,
    pub cpu_shares: u32,
    pub pids_max: u32,
}

impl ResourceLimits {
    pub fn apply(&self, pid: u32, name: &str) -> Result<()> {
        let cgroup_path = format!("/sys/fs/cgroup/a3s/{}", name);
        std::fs::create_dir_all(&cgroup_path)?;

        // Memory limit
        std::fs::write(
            format!("{}/memory.max", cgroup_path),
            format!("{}", self.memory_mb * 1024 * 1024),
        )?;

        // CPU shares
        std::fs::write(
            format!("{}/cpu.weight", cgroup_path),
            self.cpu_shares.to_string(),
        )?;

        // PID limit
        std::fs::write(
            format!("{}/pids.max", cgroup_path),
            self.pids_max.to_string(),
        )?;

        // Add process to cgroup
        std::fs::write(
            format!("{}/cgroup.procs", cgroup_path),
            pid.to_string(),
        )?;

        Ok(())
    }
}

// Usage
let agent_limits = ResourceLimits {
    memory_mb: 512,
    cpu_shares: 512,
    pids_max: 100,
};
agent_limits.apply(agent_pid, "agent")?;

let business_limits = ResourceLimits {
    memory_mb: 1024,
    cpu_shares: 1024,
    pids_max: 200,
};
business_limits.apply(business_pid, "business")?;
```

### Target Architecture Benefits

1. **Strong Isolation** - Agent and business code run in separate namespaces
2. **Independent Environments** - Each has its own complete runtime (Python, Node.js, etc.)
3. **No Dependency Conflicts** - Separate virtual environments / node_modules
4. **Standard Format** - OCI images compatible with Docker ecosystem
5. **Resource Control** - Cgroups enable CPU/memory limits per component
6. **Security** - Process isolation prevents unauthorized access
7. **Flexibility** - Agent and business code can use different languages/runtimes

### Target Architecture Limitations

1. **Complexity** - More complex than simple file mount approach
2. **Image Size** - Two OCI images increase storage requirements
3. **Startup Time** - Image extraction adds to boot time
4. **Implementation Effort** - Requires OCI parser and namespace management

### Comparison: Current vs Target Architecture

| Aspect | Current (Phase 1-2) | Target (Phase 3+) |
|--------|---------------------|-------------------|
| **Isolation** | Weak (shared process space) | Strong (namespace isolation) |
| **Environment** | Shared runtime | Independent runtimes |
| **Format** | File mount | OCI images |
| **Complexity** | Simple | Moderate |
| **Startup Time** | Fast (~3s) | Moderate (~5s) |
| **Resource Control** | None | Cgroups |
| **Use Case** | Development, trusted code | Production, untrusted code |

---

## Migration Path

### Phase 1-2 (Current)
```
VM → Single Agent Binary → /workspace (virtio-fs)
```

### Phase 3 (Target)
```
Host: Pull OCI Images → Extract to Rootfs → Boot VM
VM: Init → Namespace 1 (Agent) + Namespace 2 (Business)
```

### Backward Compatibility

The target architecture will maintain backward compatibility:

1. **Existing skills** continue to work via virtio-fs mount
2. **Simple deployments** can use current architecture
3. **OCI mode** is opt-in for users requiring strong isolation

---

**Last Updated**: 2026-02-04
