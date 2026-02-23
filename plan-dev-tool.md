# a3s dev — Orchestration Tool Implementation Plan

## Overview

`a3s dev` is a local development orchestration tool for the A3S monorepo. It lives at `apps/dev/` as a standalone Rust binary, reads an `A3sfile.hcl` config, and manages the full lifecycle of A3S services during local development: start/stop/restart, dependency ordering, subdomain routing, file watching, log aggregation, and health checks.

---

## Architecture

### Core Components (6)

These are stable, non-replaceable. Everything else is an extension.

| # | Component | Responsibility |
|---|-----------|---------------|
| 1 | `Config` | Parse and validate `A3sfile.hcl`. Single source of truth for all service definitions. |
| 2 | `Supervisor` | Own the process lifecycle for every service. Spawn, kill, restart, track PID and state. |
| 3 | `DependencyGraph` | Topological sort of `depends_on` edges. Emit ordered start/stop sequences. Block start until upstream is healthy. |
| 4 | `HealthChecker` | Poll HTTP or TCP endpoints. Emit `healthy`/`unhealthy` transitions. Used by `DependencyGraph` to unblock dependents. |
| 5 | `LogAggregator` | Merge stdout/stderr from all child processes into a single stream, prefixed with service name and timestamp. |
| 6 | `ProxyRouter` | Bind one fixed port (default 7080). Route `<service>.localhost` hostnames to the correct upstream port. |

### Extension Points (traits with defaults)

| Trait | Default impl | Purpose |
|-------|-------------|---------|
| `HealthProbe` | `HttpProbe` + `TcpProbe` | Pluggable health check strategy |
| `Watcher` | `notify`-based `FsWatcher` | File change detection for hot reload |
| `LogSink` | `StdoutSink` (colored, prefixed) | Where aggregated logs go |
| `RestartPolicy` | `ExponentialBackoff` | How to handle crashed services |

---

## A3sfile.hcl Schema

```hcl
# A3sfile.hcl — local dev orchestration config

# Global settings
dev {
  proxy_port = 7080          # single port for all *.localhost routing
  log_level  = "info"        # trace | debug | info | warn | error
}

# Service definition block — one per service
service "gateway" {
  cmd       = "cargo run -p a3s-gateway -- --config crates/gateway/gateway.hcl"
  dir       = "crates/gateway"
  port      = 8080
  subdomain = "gateway"      # routes gateway.localhost:7080 -> localhost:8080

  env = {
    RUST_LOG = "info"
  }

  watch {
    paths   = ["crates/gateway/src", "crates/gateway/gateway.hcl"]
    ignore  = ["target", "*.lock"]
    restart = true
  }

  health {
    type     = "http"
    path     = "/health"
    interval = "2s"
    timeout  = "1s"
    retries  = 3
  }
}

service "power" {
  cmd       = "cargo run -p a3s-power"
  dir       = "crates/power"
  port      = 11434
  subdomain = "power"

  env = {
    RUST_LOG = "info"
  }

  watch {
    paths   = ["crates/power/src"]
    restart = true
  }

  health {
    type     = "http"
    path     = "/api/health"
    interval = "3s"
    timeout  = "2s"
    retries  = 5
  }
}

service "search" {
  cmd       = "cargo run -p a3s-search"
  dir       = "crates/search"
  port      = 8081
  subdomain = "search"

  watch {
    paths   = ["crates/search/src"]
    restart = true
  }

  health {
    type     = "tcp"
    interval = "2s"
    timeout  = "1s"
    retries  = 3
  }
}

service "event" {
  cmd       = "cargo run -p a3s-event"
  dir       = "crates/event"
  port      = 4222
  subdomain = "event"

  health {
    type     = "tcp"
    interval = "2s"
    timeout  = "1s"
    retries  = 3
  }
}

service "lane" {
  cmd        = "cargo run -p a3s-lane"
  dir        = "crates/lane"
  port       = 8082
  subdomain  = "lane"
  depends_on = ["event"]

  health {
    type     = "http"
    path     = "/health"
    interval = "2s"
    timeout  = "1s"
    retries  = 3
  }
}

service "safeclaw" {
  cmd        = "cargo run -p a3s-safeclaw"
  dir        = "crates/safeclaw"
  port       = 8083
  subdomain  = "safeclaw"
  depends_on = ["power"]

  watch {
    paths   = ["crates/safeclaw/src"]
    restart = true
  }

  health {
    type     = "http"
    path     = "/health"
    interval = "2s"
    timeout  = "1s"
    retries  = 3
  }
}

service "box" {
  cmd        = "cargo run -p a3s-box"
  dir        = "crates/box"
  port       = 8084
  subdomain  = "box"
  depends_on = ["safeclaw", "power"]

  health {
    type     = "http"
    path     = "/health"
    interval = "3s"
    timeout  = "2s"
    retries  = 5
  }
}

service "os" {
  cmd        = "pnpm dev"
  dir        = "apps/os"
  port       = 3000
  subdomain  = "os"
  depends_on = ["gateway", "power", "search"]

  watch {
    paths   = ["apps/os/src"]
    restart = false          # Next.js/NestJS handle their own HMR
  }

  health {
    type     = "http"
    path     = "/api/health"
    interval = "3s"
    timeout  = "2s"
    retries  = 5
  }
}
```

---

## File Structure

```
apps/dev/
├── Cargo.toml              # binary: a3s, lib: a3s_dev
├── README.md
├── justfile                # just run, just build, just test
└── src/
    ├── main.rs             # CLI entry: clap subcommands (up, down, restart, status, logs)
    ├── lib.rs              # pub re-exports
    ├── config.rs           # A3sfile.hcl parsing (hcl-rs + serde)
    ├── supervisor.rs       # process spawn/kill/restart, PID tracking
    ├── graph.rs            # dependency graph, topological sort, readiness gating
    ├── health.rs           # HealthChecker + HttpProbe + TcpProbe
    ├── log.rs              # LogAggregator, StdoutSink, colored prefix formatting
    ├── proxy.rs            # ProxyRouter: single-port *.localhost -> upstream
    ├── watcher.rs          # FsWatcher wrapping notify crate
    ├── error.rs            # DevError (thiserror), Result<T> alias
    └── state.rs            # ServiceState enum: Pending -> Starting -> Running -> Unhealthy -> Stopped
```

---

## CLI Commands

```
a3s up [services...]        # start all (or named) services in dependency order
a3s down [services...]      # stop all (or named) services in reverse order
a3s restart <service>       # stop + start a single service
a3s status                  # table: service | state | pid | port | health | uptime
a3s logs [service]          # tail aggregated logs (all or one service)
a3s validate                # parse A3sfile.hcl and report errors without starting anything
```

---

## Key Data Structures

```rust
// config.rs
pub struct DevConfig {
    pub dev:      GlobalSettings,
    pub services: IndexMap<String, ServiceDef>,
}

pub struct GlobalSettings {
    pub proxy_port: u16,   // default: 7080
    pub log_level:  String,
}

pub struct ServiceDef {
    pub cmd:        String,
    pub dir:        PathBuf,
    pub port:       u16,
    pub subdomain:  Option<String>,
    pub env:        HashMap<String, String>,
    pub depends_on: Vec<String>,
    pub watch:      Option<WatchConfig>,
    pub health:     Option<HealthConfig>,
}

pub struct WatchConfig {
    pub paths:   Vec<PathBuf>,
    pub ignore:  Vec<String>,
    pub restart: bool,
}

pub struct HealthConfig {
    pub kind:     HealthKind,   // Http | Tcp
    pub path:     Option<String>,
    pub interval: Duration,
    pub timeout:  Duration,
    pub retries:  u32,
}

// state.rs
pub enum ServiceState {
    Pending,
    Starting,
    Running { pid: u32, since: Instant },
    Unhealthy { pid: u32, failures: u32 },
    Stopped,
    Failed { exit_code: Option<i32> },
}

// supervisor.rs
pub struct Supervisor {
    services: Arc<RwLock<HashMap<String, ServiceHandle>>>,
    config:   Arc<DevConfig>,
    events:   broadcast::Sender<SupervisorEvent>,
}

pub enum SupervisorEvent {
    StateChanged { service: String, state: ServiceState },
    LogLine      { service: String, line: String, stream: Stream },
    HealthChange { service: String, healthy: bool },
}

// graph.rs — Kahn's algorithm; cycle detected when sorted.len() < services.len()
pub struct DependencyGraph {
    order: Vec<String>,   // topological start order
}

impl DependencyGraph {
    pub fn from_config(cfg: &DevConfig) -> Result<Self>;
    pub fn start_order(&self) -> &[String];
    pub fn stop_order(&self) -> impl Iterator<Item = &str>;  // reverse
}

// health.rs
#[async_trait]
pub trait HealthProbe: Send + Sync {
    async fn check(&self, svc: &ServiceDef) -> bool;
}

pub struct HealthChecker {
    probe:  Box<dyn HealthProbe>,
    config: HealthConfig,
}

// proxy.rs — minimal hyper reverse proxy, no TLS needed for local dev
pub struct ProxyRouter {
    port:   u16,
    routes: Arc<RwLock<HashMap<String, SocketAddr>>>,  // subdomain -> upstream
}
```

---

## Implementation Phases

### Phase 1 — Core (ship this first)

1. `error.rs` — `DevError` enum with `thiserror`. Variants: `Config`, `Io`, `Process`, `Cycle`.
2. `state.rs` — `ServiceState` enum.
3. `config.rs` — HCL parsing with `hcl-rs`. Mirror the pattern in `crates/gateway/src/config/mod.rs`: `from_file()` async, `validate()` sync.
4. `graph.rs` — Kahn's algorithm for topological sort. Detect cycles and return `DevError::Cycle`.
5. `supervisor.rs` — `tokio::process::Command` for spawning. `broadcast::channel` for events. `RwLock<HashMap>` for state.
6. `log.rs` — Read child stdout/stderr via `tokio::io::BufReader`. Prefix each line with `[service_name]` in a distinct color per service.
7. `main.rs` — `a3s up` and `a3s down` wired to supervisor. `a3s status` prints a table.

### Phase 2 — Health + Dependencies

8. `health.rs` — `HttpProbe` (reqwest) and `TcpProbe` (tokio TcpStream connect). `HealthChecker` polls on interval, emits `SupervisorEvent::HealthChange`.
9. Wire `DependencyGraph` into `Supervisor::start_all`: after spawning a service, wait for its health check to pass before starting dependents.

### Phase 3 — File Watching + Hot Reload

10. `watcher.rs` — Wrap `notify::RecommendedWatcher`. Debounce 500ms (same constant as gateway's `file_watcher.rs`). On change, call `Supervisor::restart(service)`.

### Phase 4 — Proxy Router

11. `proxy.rs` — Hyper-based reverse proxy. Parse `Host` header, strip `.localhost` suffix, look up upstream port, forward request. Reuse hyper already in the workspace.

### Phase 5 — Polish

12. `a3s logs` — tail mode with `--follow`, filter by service name.
13. `a3s validate` — parse + graph cycle check + port conflict detection, no processes started.
14. `a3s restart` — graceful SIGTERM, wait 5s, then SIGKILL.

---

## Dependencies (Cargo.toml sketch)

```toml
[package]
name    = "a3s"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "a3s"
path = "src/main.rs"

[dependencies]
tokio             = { version = "1", features = ["rt-multi-thread", "macros", "process", "sync", "time", "io-util", "net", "signal"] }
hcl-rs            = "0.18"
serde             = { version = "1", features = ["derive"] }
clap              = { version = "4", features = ["derive"] }
thiserror         = "1"
tracing           = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
notify            = "7"
reqwest           = { version = "0.12", default-features = false, features = ["rustls-tls"] }
hyper             = { version = "1", features = ["http1", "server", "client"] }
hyper-util        = { version = "0.1", features = ["server-auto", "tokio"] }
http-body-util    = "0.1"
async-trait       = "0.1"
indexmap          = { version = "2", features = ["serde"] }
colored           = "2"
tokio-util        = { version = "0.7", features = ["io"] }

[dev-dependencies]
tempfile = "3"
tokio    = { version = "1", features = ["test-util"] }
```

---

## Design Decisions

**Why `apps/dev/` not `crates/dev/`?**
`crates/` is for reusable library crates registered as submodules. This is an app binary with no external consumers — same rationale as `apps/os/`.

**Why a single proxy port (7080) instead of per-service ports?**
Portless-style routing (`gateway.localhost:7080`) is the goal. One port to remember, one firewall rule. The proxy reads the `Host` header — standard HTTP/1.1, no magic.

**Why not reuse `a3s-gateway` for the proxy?**
Gateway is a production K8s ingress controller with TLS, ACME, rate limiting, auth. For local dev we need ~50 lines of hyper, not a full gateway. Pulling in gateway as a dep would be over-engineering.

**Why `IndexMap` for services?**
Preserves declaration order from the HCL file, which is the intuitive start order when no `depends_on` is specified. Topological sort respects this as a tiebreaker.

**Cycle detection**
Kahn's algorithm naturally detects cycles: if the sorted output length < number of services, a cycle exists. Return `DevError::Cycle` with the names of the involved services.

**Restart policy**
Default: exponential backoff starting at 1s, cap at 30s, reset after 60s of healthy uptime. Implemented as the `RestartPolicy` trait so it can be swapped without touching `Supervisor`.

---

## What This Is NOT

- Not a production process manager (no systemd units, no daemonization)
- Not a Docker Compose replacement (no container management)
- Not a build tool (commands in `A3sfile.hcl` handle building)
- Not a replacement for `a3s-gateway` in production

---

## Reference Patterns in This Codebase

| Pattern needed | Where to look |
|---------------|--------------|
| HCL config parsing | `crates/gateway/src/config/mod.rs` — `from_file()`, `validate()` |
| File watcher + debounce | `crates/gateway/src/provider/file_watcher.rs` |
| `thiserror` error enum | `crates/gateway/src/error.rs` |
| Tokio process + signal handling | `crates/gateway/src/main.rs` (`wait_for_shutdown`) |
| Health check structs | `crates/gateway/src/config/service.rs` (`HealthCheckConfig`) |
| Clap CLI structure | `crates/gateway/src/main.rs` (`Cli`, `Commands`) |
| Broadcast channel for events | `crates/gateway/src/gateway.rs` |
