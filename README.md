<p align="center">
  <img src="assets/readme/hero.svg" width="100%" alt="A3S is a local-first, open-source runtime platform for agent work">
</p>

<p align="center">
  <strong>Build locally. Add capabilities, isolation, workflows, and fleet control only when the job needs them.</strong>
</p>

<p align="center">
  <a href="https://github.com/A3S-Lab/a3s/actions/workflows/installers.yml"><img alt="Installer tests" src="https://img.shields.io/github/actions/workflow/status/A3S-Lab/a3s/installers.yml?branch=main&amp;style=flat-square&amp;label=installers"></a>
  <a href="https://github.com/A3S-Lab/CLI/releases/latest"><img alt="Latest A3S CLI release" src="https://img.shields.io/github/v/release/A3S-Lab/CLI?display_name=tag&amp;sort=semver&amp;style=flat-square&amp;color=171717"></a>
  <a href="https://crates.io/crates/a3s"><img alt="a3s on crates.io" src="https://img.shields.io/crates/v/a3s?style=flat-square&amp;color=0d74ce"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust native" src="https://img.shields.io/badge/Rust-native-60646c?style=flat-square"></a>
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-171717?style=flat-square"></a>
</p>

<p align="center">
  <a href="#start-in-60-seconds">Quick start</a> ·
  <a href="#one-owner-per-concern">Architecture</a> ·
  <a href="#product-surfaces">Products</a> ·
  <a href="#repository-map">Repository map</a> ·
  <a href="https://a3s-lab.github.io/a3s/">Website</a>
</p>

---

A3S treats an agent as a unit that can be **built, run, and operated**, not as
a chat box attached to another system. One interface connects sessions,
models, tools, permissions, durable workflows, isolated execution, and Cloud
operations while keeping every authority boundary and external dependency
visible.

This repository is the reviewed integration snapshot for independently
versioned A3S products. The canonical [`a3s` CLI](crates/cli/) and most
components live in their own repositories and are pinned here as submodules.
The root owns installers, orchestration, compatibility locks, documentation,
and directly tracked applications; it is not a second Rust package.

## Start in 60 seconds

Install the stable CLI on macOS or glibc Linux, enter a project, and start a
local Code session:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh

cd /path/to/project
a3s code
```

A model-backed session needs a configured provider or compatible local
account. Configuration inspection, component discovery, the local Web host,
and local-only research do not require an A3S OS login.

```bash
a3s config init
a3s config validate
a3s model list
a3s model use <provider>/<model>
```

The same command surface scales from one interactive session to automation,
browser work, research, isolation, and evaluation:

| Intent | First command |
| --- | --- |
| Work interactively | `a3s code` |
| Run one bounded task | `a3s code exec "Check the API boundary and run its focused tests."` |
| Open the local browser workbench | `a3s web start --detach` |
| Produce evidence-backed research | `a3s code research --web "Compare the implementation with its design"` |
| Inspect isolated workloads | `a3s box ps` |
| Inspect installed products | `a3s list --installed` |

See [Installation](#installation) for Windows, Homebrew, Cargo, offline use,
and release-channel details. The complete command reference lives with the
[pinned CLI](crates/cli/docs/cli-reference.md).

## One owner per concern

<p align="center">
  <img src="assets/readme/architecture.svg" width="100%" alt="A3S architecture: product hosts set policy, agent and workflow contracts compose work, Runtime and Power execute through replaceable providers, and evidence returns to the owning authority">
</p>

A3S grows progressively. A local session does not require the whole platform,
and installing a package does not silently grant it authority or activate an
infrastructure dependency.

| Layer | Contract | Owners |
| --- | --- | --- |
| **Host** | Invocation, policy, models, tools, permissions | [CLI](crates/cli/), [Code](crates/code/), [Web](apps/web/), [Cloud](apps/cloud/) |
| **Extend** | Signed capabilities and typed content | [Use](crates/use/), [Browser](crates/browser/), [Search](crates/search/), [OCR](crates/ocr/), [Parser](crates/parser/), [Form](packages/form/), [Office](packages/office/), [Science](packages/science/) |
| **Coordinate** | Replay-safe workflows, events, queues, tests | [Flow](crates/flow/), [Event](crates/event/), [Lane](crates/lane/), [Bench](crates/bench/), [Test](crates/test/) |
| **Execute** | Tasks, Services, isolation, model serving | [Runtime](crates/runtime/), [Box](crates/box/), [OCI Runtime](crates/oci-runtime/), [Power](crates/power/), [MoE](crates/moe/), [Boot](crates/boot/) |
| **Scale** | Traffic, desired state, placement, reconciliation | [Gateway](crates/gateway/), [Cloud](apps/cloud/), [ORM](crates/orm/) |
| **Govern** | Observation, enforcement decisions, signed updates | [Observer](crates/observer/), [Sentry](crates/sentry/), [Updater](crates/updater/) |

The architecture follows five invariants:

1. **Hosts own policy.** Product hosts choose models, tools, providers, and
   permissions; execution backends do not invent authority.
2. **One authority owns each concern.** Cloud owns tenant and desired state,
   Flow owns durable orchestration, Runtime owns provider-neutral Task and
   Service lifecycles, and concrete providers own enforcement mechanics.
3. **Contracts are replaceable.** Local processes, containers, MicroVMs, model
   servers, and remote providers meet typed boundaries instead of becoming
   hidden defaults.
4. **State has identity.** Sessions, runs, tasks, artifacts, receipts, and
   operations retain stable identities, revisions, and digests across restart
   and replay.
5. **Evidence closes the loop.** Installation, authorization, activation,
   execution, health, and publication are distinct states with distinct proof.

Configuration is ACL parsed and generated by [`a3s-acl`](crates/acl/). ACL is
not HCL and must not be processed with an HCL parser.

## Product surfaces

The umbrella CLI owns invocation context, shared configuration, credentials,
discovery, and output policy. Each product continues to own its behavior,
release cadence, and detailed documentation.

| Surface | Start here | Boundary |
| --- | --- | --- |
| **Code** | `a3s code` | Bundled agent engine and terminal host |
| **Web** | `a3s web` | Loopback API and browser workbench included by compatible releases |
| **Use** | `a3s use capabilities --json` | Signed dependency graphs and hot-pluggable Tool, MCP, Flow, Skill, knowledge, and UI surfaces |
| **Box** | `a3s box ps` | Explicit local isolation and OCI workloads |
| **Power + MoE** | [Power](crates/power/) · [MoE](crates/moe/) | Power owns model-neutral serving, residency, verified artifact provisioning, admission, and speculative execution; model crates own architecture equations and checkpoint packing |
| **Flow + Cloud** | [`compat/cloud-stack.acl`](compat/cloud-stack.acl) | Durable orchestration composed by a self-hosted control plane under an exact compatibility lock |
| **Search** | `a3s search …` | Browser-first search with quality-gated fallbacks |
| **Bench** | `a3s bench …` | Evaluation runs and evidence; artifact availability remains platform-specific |
| **Top** | `a3s top` | Local view of agents, containers, sessions, and events |

> [!NOTE]
> Discovery is not availability. A catalog record can describe installation
> policy without proving that every platform or release channel contains a
> compatible artifact.

## What is verified today

- The standalone [`A3S-Lab/CLI`](https://github.com/A3S-Lab/CLI) repository
  owns CLI source, CI, tags, releases, and product documentation. This root
  pins the reviewed 0.12.1 release revision and retains an asset-only relay for
  legacy 0.11.x clients that briefly used the monorepo release endpoint.
- The pinned [Code](crates/code/) revision includes a capability ledger and
  release qualification for retrieval, code intelligence, context and memory,
  Flow and State Graph projection, persistence, and hermetic S3, browser, and
  OpenTelemetry boundaries. Remote-provider and public-network latency remain
  explicitly out of scope.
- The Cloud compatibility lock pins exact Cloud, Code, ACL, Boot, Box, Event,
  Flow, Form, Gateway, ORM, Runtime, Sentry, Updater, and Use revisions plus their
  shared protocol levels. Run `just cloud-stack-check` before publishing a new
  integration claim.
- Workflow Phases 0–2 have verified contract, Form lifecycle, and minimal run
  baselines. Phase 3 has a pinned internal HumanTask decision slice, including
  protected reads, claim/release, native Form submission, expiry, cancellation,
  and Outbox resume; the controlled inbox and full end-to-end exit gate remain
  open. See the [architecture](compat/workflow-platform-architecture.md) and
  [ordered plan](compat/workflow-platform-development-plan.md).
- Use is a preview, not a supported product release. Its exact accepted schema
  line and remaining cross-platform gates are owned by the pinned component
  README and the compatibility lock.
- MoE owns model-specific equations and validation; Power owns model-neutral
  execution. Model support evidence does not imply that every optimized kernel,
  accelerator backend, or speculative adapter is complete.
- Ash and Parser are early-stage, Office is pre-1.0, and OCI Runtime's native
  Linux path remains experimental rather than the default launch claim.

Component READMEs, releases, roadmaps, and the compatibility lock are the
sources of truth for exact feature flags, platforms, versions, and remaining
gates. This page describes the composition, not a merged changelog.

## Installation

| Method | Command | Notes |
| --- | --- | --- |
| macOS / glibc Linux | `curl --proto '=https' --tlsv1.2 -LsSf https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh \| sh` | Installs to `~/.local/bin` by default; set `A3S_MODIFY_PATH=1` to edit shell profiles |
| Homebrew | `brew install a3s-lab/tap/a3s` | Supported on macOS and Linux |
| Windows PowerShell 5.1+ | `irm https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.ps1 \| iex` | Installs under `%LOCALAPPDATA%\Programs\a3s\bin` by default |
| Cargo | `cargo install a3s` | CLI only; does not bundle the complete Web workspace or WebView helper |

Release installers resolve one stable version, require an exact artifact for
the detected platform, verify its SHA-256 release digest and staged binary
version, reject unsafe archive entries, and preserve the previous installation
if activation fails.

Set `A3S_OFFLINE=1` and `A3S_NO_AUTO_INSTALL=1` when setup must perform zero
network access and zero component mutation. Standalone macOS and Linux installs
can use `a3s self update`; Homebrew installations should update through
Homebrew. Windows upgrades currently rerun the installer.

## Repository map

The root is a monorepo integration point, not a Rust workspace. Most components
are external repositories tracked as git submodules; directly tracked
applications, integration assets, `crates/common`, and `crates/updater` remain
root-owned.

```text
a3s/
├── apps/       cloud control plane, docs, Web, and Windhole
├── packages/   Form, Office, Science, and UI
├── crates/     product hosts, capabilities, runtimes, services, and SDKs
├── compat/     exact cross-project revisions and protocol locks
├── assets/     repository-native README visuals
└── homebrew-tap/
```

| Group | Projects |
| --- | --- |
| Product hosts | [CLI](crates/cli/), [Code](crates/code/), [Ash](crates/ash/), [Web](apps/web/), [Windhole](apps/windhole/), [Cloud](apps/cloud/) |
| Capabilities and content | [Use](crates/use/), [Browser](crates/browser/), [Search](crates/search/), [OCR](crates/ocr/), [Parser](crates/parser/), [Form](packages/form/), [Office](packages/office/), [Science](packages/science/) |
| Runtime, inference, and coordination | [Runtime](crates/runtime/), [Box](crates/box/), [OCI Runtime](crates/oci-runtime/), [Power](crates/power/), [MoE](crates/moe/), [Flow](crates/flow/), [Event](crates/event/), [Lane](crates/lane/), [Memory](crates/memory/), [ORM](crates/orm/) |
| Verification | [Bench](crates/bench/), [Test](crates/test/) |
| Interfaces and services | [Boot](crates/boot/), [Gateway](crates/gateway/), [AHP](crates/ahp/), [ACL](crates/acl/), [Common](crates/common/), [TUI](crates/tui/), [GUI](crates/gui/), [UI](packages/ui/), [WebView](crates/webview/) |
| Operations and distribution | [Observer](crates/observer/), [Sentry](crates/sentry/), [Updater](crates/updater/), [Website](apps/docs/), [Homebrew Tap](homebrew-tap/) |

The [CLI migration record](docs/cli-repository-migration.md) explains the
temporary 0.11.x root migration and restored standalone ownership. The
interactive [project directory](https://a3s-lab.github.io/a3s/#ecosystem)
shows each project's role, stage, version or channel, website, and source.

## Development

Clone the exact integration snapshot:

```bash
git clone --recurse-submodules git@github.com:A3S-Lab/a3s.git
cd a3s
```

For an existing checkout, run `git submodule update --init --recursive`.

> [!IMPORTANT]
> Do not create a root `Cargo.toml`, run `cargo init` here, or treat the root as
> a Rust crate. Work and test inside the component that owns the change.

For example, validate the pinned CLI from its submodule:

```bash
cd crates/cli
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

The root `justfile` orchestrates integration workflows:

```bash
just code
just web
just docs
just windhole
just use-hotplug-e2e
just cloud-stack-check
```

Submodules and the root have separate histories. Commit a component change in
its owning repository before advancing its gitlink here, and read
[AGENTS.md](AGENTS.md) before changing repository structure.

## Documentation and community

- [A3S website](https://a3s-lab.github.io/a3s/)
- [CLI reference](docs/cli-reference.md)
- [Workflow platform architecture](compat/workflow-platform-architecture.md)
- [Workflow development plan](compat/workflow-platform-development-plan.md)
- [Cloud compatibility lock](compat/cloud-stack.acl)
- [CLI releases](https://github.com/A3S-Lab/CLI/releases)
- [Discord](https://discord.gg/XVg6Hu6H)

## License

This integration repository is licensed under the [MIT License](LICENSE).
Independently versioned projects retain the license declared by their owning
repositories.
