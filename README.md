<p align="center">
  <img src="assets/readme/hero.svg" width="100%" alt="A3S is the open-source, local-first operating system for agents">
</p>

<p align="center">
  <strong>Build, run, and operate agents through one explicit interface—from a local workspace to Box and Cloud.</strong>
</p>

<p align="center">
  <a href="https://github.com/A3S-Lab/a3s/actions/workflows/installers.yml"><img alt="Installer tests" src="https://img.shields.io/github/actions/workflow/status/A3S-Lab/a3s/installers.yml?branch=main&amp;style=flat-square&amp;label=installers"></a>
  <a href="https://github.com/A3S-Lab/CLI/releases/latest"><img alt="Latest A3S CLI release" src="https://img.shields.io/github/v/release/A3S-Lab/CLI?display_name=tag&amp;sort=semver&amp;style=flat-square&amp;color=171717"></a>
  <a href="https://crates.io/crates/a3s"><img alt="a3s on crates.io" src="https://img.shields.io/crates/v/a3s?style=flat-square&amp;color=0d74ce"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust native" src="https://img.shields.io/badge/Rust-native-60646c?style=flat-square"></a>
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-171717?style=flat-square"></a>
</p>

<p align="center">
  <a href="#quick-start">Quick start</a> ·
  <a href="#how-a3s-grows-with-the-job">Architecture</a> ·
  <a href="#product-surfaces">Products</a> ·
  <a href="#repository-map">Repository map</a> ·
  <a href="https://a3s-lab.github.io/a3s/">Documentation</a>
</p>

---

A3S treats an agent as a unit that can be **built, deployed, and operated**—not
as a chat box attached to an existing system. It gives sessions, models, tools,
state, workflows, execution, and permissions one interface while keeping every
external dependency visible.

A3S is the integration snapshot for independently versioned products and
libraries. The canonical [`a3s` CLI](crates/cli/) is pinned as a submodule; its
standalone repository owns source, CI, tags, releases, and detailed product
documentation. The monorepo root owns orchestration, integration gates,
installers, compatibility locks, and directly tracked applications—not a
second Rust package.

## Quick start

Install the latest stable CLI on macOS or glibc Linux, then launch Code inside a
project:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh

cd /path/to/project
a3s code
```

A model-backed session needs a configured provider or compatible local account.
The CLI itself, local configuration, component inspection, Web host, and
local-only research do not require an A3S OS login.

```bash
a3s config init
a3s config validate
a3s model list
a3s model use <provider>/<model>
```

Once configured, the same entrypoint supports interactive work, automation, a
local browser workbench, and evidence-backed research:

```bash
a3s code
a3s code exec "Check the API boundary and run its focused tests."
a3s web -d
a3s code research --web "Compare the current implementation with its design"
```

See [Installation](#installation) for Windows, Homebrew, Cargo, offline use, and
release-channel details.

Runtime-hosted Agent releases use the same bundled Code engine through the one
native process declared by the release contract:

~~~bash
a3s code harness --manifest /app/.a3s/asset.acl
~~~

This process exposes only the bounded Code command, event-page, and declared
health routes. Cloud, Fleet, and the Node Agent transport those contracts; they
do not own another Harness loop, run store, scheduler, or event log. If the
manifest declares `persistent_data = "external"`, the active Code ACL config
must point `sessions_dir` at an existing absolute Runtime-mounted directory.
`persistent_data = "none"` keeps Harness session and memory state process-local.

The complete TUI, permission, session, and component command reference lives in
the [pinned CLI reference](crates/cli/docs/cli-reference.md).

## Why A3S

- **Local first.** Ordinary work stays on the developer machine. Box and Cloud
  are explicit execution choices, not hidden defaults.
- **One boundary model.** ACL configuration, permissions, accounts, models, and
  component lifecycle are resolved by the umbrella CLI.
- **Durable work.** Sessions, events, artifacts, memory, workflow checkpoints,
  and evaluation results survive one conversation or process.
- **Composable contracts.** Hosts own policy; Runtime, capability, data, and
  infrastructure providers remain replaceable.
- **Evidence over implication.** Installation, authorization, activation,
  execution, health, and publication are separate states with separate proof.

## How A3S grows with the job

<p align="center">
  <img src="assets/readme/architecture.svg" width="100%" alt="A3S architecture from local product hosts through agent capabilities and coordination to replaceable runtime providers">
</p>

The system expands progressively. A local Code session does not require the
whole platform, and adding a catalog entry does not silently activate a
capability or infrastructure dependency.

| Stage | Contract introduced | Projects that own it |
| --- | --- | --- |
| **Build** | Agent session, model, tools, context, permissions | [Code](crates/code/), [TUI](crates/tui/), [Web](apps/web/) |
| **Extend** | Typed capability and content packages | [Use](crates/use/), [Browser](crates/browser/), [Search](crates/search/), [OCR](crates/ocr/), [Parser](crates/parser/), [Office](packages/office/), [Science](packages/science/) |
| **Coordinate** | Replay-safe workflows, events, queues, verification | [Flow](crates/flow/), [Event](crates/event/), [Lane](crates/lane/), [Bench](crates/bench/), [Test](crates/test/) |
| **Execute** | Finite Tasks, long-running Services, isolation, model serving | [Runtime](crates/runtime/), [Box](crates/box/), [OCI Runtime](crates/oci-runtime/), [Power](crates/power/), [Boot](crates/boot/) |
| **Scale** | Traffic, desired state, placement, reconciliation | [Gateway](crates/gateway/), [Cloud](apps/cloud/), [ORM](crates/orm/) |
| **Govern** | Observation, enforcement decisions, signed updates | [Observer](crates/observer/), [Sentry](crates/sentry/), [Updater](crates/updater/) |

Five invariants keep those layers honest:

1. Product hosts own policy and select the models, tools, providers, and
   permissions they use.
2. Core interfaces stay replaceable; external services are never treated as
   invisible defaults.
3. Durable systems persist identity for sessions, workflows, runtime units,
   evaluations, and Cloud operations.
4. Policy and enforcement remain separate—for example, Code routes permission
   decisions while a sandbox provider enforces a local command boundary.
5. Configuration is ACL parsed and generated by [`a3s-acl`](crates/acl/). ACL
   is not HCL and must not be handled by an HCL parser.

The detailed command and process design lives in the
[CLI product design](crates/cli/docs/cli-product-design.md) and
[CLI technical architecture](crates/cli/docs/cli-technical-architecture.md).

## Product surfaces

The standalone CLI owns invocation context, shared configuration, output
policy, credentials, discovery, and lifecycle. Product behavior stays in the
component that implements it.

| Surface | Start here | Delivery boundary |
| --- | --- | --- |
| **Code** | `a3s code` | Bundled agent runtime and terminal host |
| **Web** | `a3s web` | Bundled loopback API plus Web assets when the release includes them |
| **Research** | `a3s code research --web "…"` | Bundled typed runner producing run-scoped Markdown and editable HTML evidence |
| **Top** | `a3s top` | Bundled view of agents, containers, sessions, and events |
| **Box** | `a3s box ps` | Managed product for explicit local isolation and OCI workloads |
| **Search** | `a3s search …` | Managed Browser-first search product with quality-gated fallbacks |
| **Use** | `a3s use capabilities --json` | Independently versioned AI-native package manager for signed dependency graphs and hot-pluggable Tool, MCP, OKF, A3S Flow, Skill, and UI surfaces |
| **Bench** | `a3s bench …` | Managed evaluation product; [v0.1.2](https://github.com/A3S-Lab/Bench/releases/tag/v0.1.2) ships compatible components for Linux x86_64 and macOS arm64 |
| **Cloud** | [`compat/cloud-stack.acl`](compat/cloud-stack.acl) | Self-hosted control plane governed by a revision and protocol compatibility lock |

Use publishes durable schema-v3 package-state generations for cognitive
packages containing any combination of Tool, Skill, MCP, UI, OKF, and A3S Flow
surfaces. The pinned CLI host observes that Use-owned generation and performs
reviewed enable or disable without reinstalling package bytes or rewriting the
dependency graph. CLI and Web persist the exact User-scoped plan, collect
confirmation, and apply only its operation ID and canonical digest; TUI
consumes the resulting hot-plug generations but still lacks a package-level
mutation panel. For a complete locked graph plan, the host
snapshots the exact User/Workspace Grant scope and durable revision, applies
host policy, binds Use's canonical Grant impact, and forwards the unchanged
authority and confirmation into Use-owned apply and replay. Permission-bearing
enablement uses the same Grant-before-publish and hide/drain-before-revoke saga.

Use the machine-readable commands before scripting an optional product:

```bash
a3s list --installed
a3s info box
a3s doctor
```

> [!NOTE]
> A catalog record describes discovery and installation policy. It does not
> prove that every platform or release channel currently contains a compatible
> artifact.

## Current release boundaries

The CLI release channel is owned by the standalone
[`A3S-Lab/CLI`](https://github.com/A3S-Lab/CLI) repository. These boundaries
describe the integration snapshot pinned by this repository's `main` branch:

| Area | Current boundary |
| --- | --- |
| Standalone CLI | Code, Web, Research, configuration, auth, models, diagnostics, reviewed plugin plan/apply, component lifecycle, CI, tags, and releases are owned by `A3S-Lab/CLI`; this repository pins one reviewed gitlink |
| Managed products | Box and Search install and run as independently versioned components; artifact availability is platform- and channel-specific |
| Use | `main` pins schema-v3 package-state generations, replaceable TUF registries, exact SemVer dependency locks, atomic graph lifecycle, canonical host-reviewed Grant composition, reviewed permission-bearing enablement, and restart-safe hot-plug across Tool, MCP, OKF, A3S Flow, Skill, and UI. Production Knowledge/OKF, Runtime Service, HTTP MCP/Gateway, a TUI package-level mutation panel, distributed Flow recovery/retention, complete host graph upgrade/uninstall, and real-process cross-platform E2E remain release gates. |
| Bench | [v0.1.2](https://github.com/A3S-Lab/Bench/releases/tag/v0.1.2) is installable on Linux x86_64 and macOS arm64; local runs require Docker and remain `local_unofficial` by governance |
| Cloud | R0–E0 is the verified cumulative baseline; later milestones remain tracked by the canonical [Cloud compatibility manifest](compat/cloud-stack.acl) |
| Early projects | Ash is pre-release, Parser is pre-alpha, Office is pre-1.0, and OCI Runtime's native Linux path is experimental rather than the default launch claim |

Release-bearing projects publish on their own cadence. Their manifests and
READMEs are the source of truth for feature flags, supported platforms, tests,
and maturity; this root README describes integration boundaries rather than
duplicating every component changelog.

## Installation

| Method | Command | Notes |
| --- | --- | --- |
| macOS / glibc Linux | `curl --proto '=https' --tlsv1.2 -LsSf https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh \| sh` | Installs to `~/.local/bin` by default; does not edit shell profiles unless `A3S_MODIFY_PATH=1` |
| Homebrew | `brew install a3s-lab/tap/a3s` | Supported on macOS and Linux |
| Windows PowerShell 5.1+ | `irm https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.ps1 \| iex` | Installs under `%LOCALAPPDATA%\Programs\a3s\bin` by default |
| Cargo | `cargo install a3s` | CLI-only installation; it does not bundle the complete Web workspace or WebView helper |

The release installers resolve one stable version, require one exact artifact
for the detected platform, verify GitHub's SHA-256 release digest and the staged
binary version, reject unsafe archive entries, and preserve the previous
installation if activation fails.

Use `A3S_OFFLINE=1` and `A3S_NO_AUTO_INSTALL=1` when automatic setup must perform
zero network access and zero component mutation. Standalone macOS and Linux
installs can use `a3s self update --check` and `a3s self update`; Homebrew-managed
installs should update through Homebrew. Windows currently upgrades by rerunning
the installer.

Full installer controls and platform notes are in the
[CLI documentation](https://a3s-lab.github.io/a3s/).

Standalone versions 0.9.9 through 0.10.10 already read `A3S-Lab/CLI` releases.
Versions 0.11.0 and 0.11.1 briefly used the monorepo endpoint; a verified,
asset-only compatibility relay remains here so those clients can perform one
update back to the CLI-owned release channel.

## Repository map

The repository root is a monorepo integration point, not a Rust package or
Cargo workspace. Most components are external repositories tracked as git
submodules, while directly tracked applications and integration assets remain
root-owned.

| Group | Projects |
| --- | --- |
| Product hosts | [CLI](crates/cli/), [Code](crates/code/), [Ash](crates/ash/), [Web](apps/web/), [Windhole](apps/windhole/), [Cloud](apps/cloud/) |
| Capabilities and content | [Use](crates/use/), [Browser](crates/browser/), [Search](crates/search/), [OCR](crates/ocr/), [Parser](crates/parser/), [Office](packages/office/), [Science](packages/science/) |
| Runtime, coordination, and data | [Runtime](crates/runtime/), [Box](crates/box/), [OCI Runtime](crates/oci-runtime/), [Flow](crates/flow/), [Event](crates/event/), [Lane](crates/lane/), [Memory](crates/memory/), [ORM](crates/orm/) |
| Verification | [Bench](crates/bench/), [Test](crates/test/) |
| Services and interfaces | [Boot](crates/boot/), [Gateway](crates/gateway/), [Power](crates/power/), [AHP](crates/ahp/), [ACL](crates/acl/), [Common](crates/common/), [TUI](crates/tui/), [GUI](crates/gui/), [UI](packages/ui/), [WebView](crates/webview/) |
| Operations and distribution | [Observer](crates/observer/), [Sentry](crates/sentry/), [Updater](crates/updater/), [CLI website](apps/docs/), [Homebrew Tap](homebrew-tap/) |

The [CLI repository migration record](docs/cli-repository-migration.md)
documents the temporary 0.11.x root migration, restored standalone ownership,
and legacy-client release relay. The interactive
[architecture atlas](https://a3s-lab.github.io/a3s/#architecture) exposes each
project's entrypoints, state, adapters, security boundaries, and control flow.

## Development

Clone the exact integration snapshot:

```bash
git clone --recurse-submodules git@github.com:A3S-Lab/a3s.git
cd a3s
```

For an existing checkout, run `git submodule update --init --recursive`.

> [!IMPORTANT]
> The repository root is not a Rust package or Cargo workspace. Run Rust
> commands from the submodule that owns the code; the root `justfile` only
> orchestrates cross-project development and verification.

A typical CLI validation runs from the pinned CLI submodule:

```bash
cd crates/cli
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

Validate another Rust project from its owning submodule instead:

```bash
cd crates/<project>
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

The root `justfile` provides the common integration entrypoints:

```bash
just code
just web
just docs
just windhole
just use-hotplug-e2e
just marketplace-science-e2e
just marketplace-science-browser-e2e
just cloud-stack-check
```

Submodules and the root repository have separate Git histories. Commit a
component change in its owning repository before updating its gitlink here, and
read [AGENTS.md](AGENTS.md) before changing repository structure.

## Documentation and community

- [A3S CLI documentation](https://a3s-lab.github.io/a3s/)
- [CLI reference](docs/cli-reference.md)
- [CLI releases](https://github.com/A3S-Lab/CLI/releases)
- [Discord](https://discord.gg/XVg6Hu6H)

Each project README records its detailed APIs, feature flags, platform
requirements, verification commands, and remaining limitations.

## License

This integration repository is licensed under the [MIT License](LICENSE).
Independently versioned projects retain the license declared by their owning
repositories.
