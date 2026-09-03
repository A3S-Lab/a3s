<p align="center">
  <img src="assets/readme/hero.svg" width="100%" alt="A3S is a local-first, open-source runtime platform for agent work">
</p>

<p align="center">
  <a href="https://github.com/A3S-Lab/a3s/actions/workflows/installers.yml"><img alt="Installer tests" src="https://img.shields.io/github/actions/workflow/status/A3S-Lab/a3s/installers.yml?branch=main&amp;style=flat-square&amp;label=installers"></a>
  <a href="https://github.com/A3S-Lab/CLI/releases/latest"><img alt="Latest A3S CLI release" src="https://img.shields.io/github/v/release/A3S-Lab/CLI?display_name=tag&amp;sort=semver&amp;style=flat-square&amp;color=171717"></a>
  <a href="https://crates.io/crates/a3s"><img alt="a3s on crates.io" src="https://img.shields.io/crates/v/a3s?style=flat-square&amp;color=0d74ce"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust native" src="https://img.shields.io/badge/Rust-native-60646c?style=flat-square"></a>
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-171717?style=flat-square"></a>
</p>

<p align="center">
  <a href="#start-local">Start local</a> ·
  <a href="#how-a3s-fits-together">System model</a> ·
  <a href="#choose-a-surface">Products</a> ·
  <a href="#installation">Install</a> ·
  <a href="#repository-map">Repository</a> ·
  <a href="https://a3s-lab.github.io/a3s/">Website</a>
</p>

A3S is an open-source, local-first runtime platform for building, running, and
operating agent work. One interface connects sessions, models, tools,
permissions, durable workflows, isolated execution, and Cloud operations while
keeping authority boundaries and external dependencies explicit.

Start with one local Code session. Add capabilities, orchestration, isolation,
inference, or fleet control only when the work requires them.

> [!IMPORTANT]
> This repository is the reviewed integration snapshot for independently
> versioned A3S products. Most components are pinned here as git submodules;
> the root owns installers, orchestration, compatibility locks, and shared
> documentation. It is not a Rust workspace or a second copy of each product.

## Start local

Install the stable CLI on macOS or glibc Linux, enter a project, and launch an
interactive local session:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh

cd /path/to/project
a3s code
```

A model-backed session needs a configured provider or compatible local
account. Inspect and select the model explicitly:

```bash
a3s config init
a3s config validate
a3s model list
a3s model use <provider>/<model>
```

The same CLI grows from an interactive session into bounded automation,
research, isolation, and operations:

| Intent | First command |
| --- | --- |
| Work interactively | `a3s code` |
| Run one bounded task | `a3s code exec "Check the API boundary and run its focused tests."` |
| Produce evidence-backed research | `a3s code research --web "Compare the implementation with its design"` |
| Inspect isolated workloads | `a3s box ps` |
| Inspect installed products | `a3s list --installed` |

See [Installation](#installation) for Windows, Homebrew, Cargo, offline use,
and release-channel details. The complete command surface lives in the
[CLI reference](docs/cli-reference.md).

## How A3S fits together

<p align="center">
  <img src="assets/readme/architecture.svg" width="100%" alt="A3S architecture: product hosts set policy, agent and workflow contracts compose work, replaceable providers execute it, and evidence returns to the owning authority">
</p>

A3S grows progressively. Installing a component does not silently activate an
infrastructure dependency or grant it authority.

| Layer | Responsibility | Owners |
| --- | --- | --- |
| **Host** | Invocation, policy, models, tools, permissions | [CLI](crates/cli/), [Code](crates/code/), [Cloud](apps/cloud/) |
| **Extend** | Signed capabilities and typed content | [Use](crates/use/), [Browser](crates/browser/), [Search](crates/search/), [OCR](crates/ocr/), [Parser](crates/parser/), [Office](packages/office/), [Science](packages/science/) |
| **Coordinate** | Replay-safe workflows, events, queues, and evaluation | [Flow](crates/flow/), [Event](crates/event/), [Lane](crates/lane/), [Bench](crates/bench/), [Test](crates/test/) |
| **Execute** | Tasks, Services, isolation, and model serving | [Runtime](crates/runtime/), [Sandbox](crates/sandbox/), [Box](crates/box/), [OCI Runtime](crates/oci-runtime/), [Power](crates/power/), [MoE](crates/moe/) |
| **Scale** | Traffic, desired state, placement, and reconciliation | [Gateway](crates/gateway/), [Cloud](apps/cloud/), [ORM](crates/orm/) |
| **Govern** | Observation, enforcement decisions, and signed updates | [Observer](crates/observer/), [Sentry](crates/sentry/), [Updater](crates/updater/) |

Four rules keep the system composable:

1. **Hosts own policy.** Product hosts choose models, tools, providers, and
   permissions; backends do not invent authority.
2. **One concern has one owner.** Cloud owns desired state, Flow owns durable
   orchestration, Runtime owns provider-neutral lifecycles, and concrete
   providers own enforcement mechanics.
3. **Contracts stay replaceable.** Processes, containers, MicroVMs, model
   servers, and remote providers meet typed boundaries instead of becoming
   hidden defaults.
4. **Evidence closes the loop.** Identity, revisions, digests, health, and
   receipts distinguish installation, authorization, activation, execution,
   and publication.

Configuration uses [A3S ACL](crates/acl/), the Agent Configuration Language.
ACL is not HCL and must be parsed and generated with `a3s-acl`.

## Choose a surface

The umbrella CLI owns invocation context, shared configuration, credentials,
discovery, and output policy. Each product owns its behavior, release cadence,
and detailed support contract.

| Surface | Start here | Owns |
| --- | --- | --- |
| **Code** | `a3s code` | Local agent engine and terminal host |
| **Desktop** | [Download](https://a3s-lab.github.io/a3s/download/) · [Source](apps/desktop/) | Native workbench powered by the local Code kernel |
| **Use** | `a3s use capabilities --json` | Signed dependency graphs and hot-pluggable Tool, MCP, Flow, Skill, knowledge, and UI capabilities |
| **Box** | `a3s box ps` | Explicit local isolation and OCI workloads |
| **Power + MoE** | [Power](crates/power/) · [MoE](crates/moe/) | Model-neutral serving and residency plus model-specific equations and validation |
| **Flow + Cloud** | [`compat/cloud-stack.acl`](compat/cloud-stack.acl) | Durable orchestration and the self-hosted control plane under an exact compatibility lock |
| **Search** | `a3s search …` | Browser-first search with quality-gated fallbacks |
| **Bench** | `a3s bench …` | Evaluation runs and evidence |
| **Top** | `a3s top` | A local view of agents, containers, sessions, and events |

> [!NOTE]
> Discovery is not availability. A catalog record can describe installation
> policy without proving that every platform or release channel contains a
> compatible artifact.

## Release posture

A3S is a composed system, so it does not have one blanket maturity label.
Support claims live with the component that owns the behavior:

| Area | Current posture | Source of truth |
| --- | --- | --- |
| CLI and installers | The standalone CLI repository owns source, CI, releases, and detailed product documentation; this root pins a reviewed revision and relays only the required integration assets. | [CLI](https://github.com/A3S-Lab/CLI) · [installer CI](.github/workflows/installers.yml) |
| Local agent work | Code owns session, workspace, retrieval, context, memory, and provider qualification. Provider and public-network behavior is qualified separately from the local core. | [Code](crates/code/) · [retrieval roadmap](docs/retrieval-platform-roadmap.md) |
| Native command isolation | Sandbox owns the fail-closed macOS, Linux, and Windows command boundary. Code consumes its small Rust contract without Node.js or a legacy runtime compatibility layer. | [Sandbox](crates/sandbox/) · [security model](crates/sandbox/SECURITY.md) |
| Cloud and workflows | Exact component revisions, package versions, and protocol levels are locked together before an integration claim is published. | [Cloud stack lock](compat/cloud-stack.acl) · [workflow plan](compat/workflow-platform-development-plan.md) |
| Capabilities | Use is a preview. Discovery does not imply installation authority, activation, or operated Registry readiness. | [Use](crates/use/) · [Use Registry](use-registry/) |
| Isolation and inference | Platform, driver, accelerator, and artifact evidence is explicit. Model support does not imply that every optimized kernel or backend is complete. | [OCI Runtime](crates/oci-runtime/) · [Runtime](crates/runtime/) · [Power](crates/power/) · [MoE](crates/moe/) |
| Early-stage surfaces | Ash and Parser remain early-stage; Office remains pre-1.0. | [Ash](crates/ash/) · [Parser](crates/parser/) · [Office](packages/office/) |

Component READMEs, releases, roadmaps, and compatibility locks carry exact
versions, platforms, fixtures, and remaining gates. This page explains how the
parts compose; it is not a merged changelog.

The current Code retrieval pin includes a Memory-authoritative A3S Vec shadow
for differential qualification. Its ownership boundary, status contract, and
promotion gates are recorded in the
[Vec migration note](crates/code/manual/WORKSPACE_RETRIEVAL_VEC_MIGRATION.md).
Vec's public API review, reproducible candidate package, and remaining external
release gate are recorded in
[Vec release qualification](crates/vec/RELEASE.md).
The pinned Vec revision also carries Binary32/Binary64 exact L2/Hamming search,
the complete public feature matrix, a revision-bound 53-row smoke performance
CSV gate, five platform smoke CSVs (including lifecycle/resource/maintenance
metrics), and a configurable larger-corpus a3s-vec/zvec comparison harness;
the recorded same-host 100k x 128 directional comparison at Vec revision
`41283f6` shows zvec 0.7.0 at
about 5.6x lower flat p50, 5.1x lower HNSW p50, and 2.2x shorter total HNSW
build under the pinned one-worker controls. The newer Vec revision removes a
per-candidate dense conversion and repeats each cosine query norm only once;
that reduced the a3s-vec flat p50 by 20.4% and HNSW p50 by 21.3% on the same
fixture. The baseline uses Cargo's portable a3s-vec build against zvec's native
wheel; a3s-vec also exact-reranks HNSW candidates while the zvec optional
refiner is disabled, so these are not universal or compiler-level apples-to-
apples ratios. Both recall values require a higher `ef` for a production
target. See
[Vec benchmark evidence](crates/vec/BENCHMARKS.md).

## Installation

| Method | Command | Notes |
| --- | --- | --- |
| macOS / glibc Linux | `curl --proto '=https' --tlsv1.2 -LsSf https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh \| sh` | Installs to `~/.local/bin` by default; set `A3S_MODIFY_PATH=1` to edit shell profiles |
| Homebrew | `brew install a3s-lab/tap/a3s` | Supported on macOS and Linux |
| Windows PowerShell 5.1+ | `irm https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.ps1 \| iex` | Installs under `%LOCALAPPDATA%\Programs\a3s\bin` by default |
| Cargo | `cargo install a3s` | Builds the CLI only; install the optional WebView helper separately |

Release installers resolve one stable version, require an exact artifact for
the detected platform, verify its SHA-256 digest and staged binary version,
reject unsafe archive entries, and preserve the previous installation if
activation fails.

Set `A3S_OFFLINE=1` and `A3S_NO_AUTO_INSTALL=1` when setup must perform zero
network access and zero component mutation. Standalone macOS and Linux installs
can use `a3s self update`; Homebrew installations should update through
Homebrew. Windows upgrades currently rerun the installer.

## Repository map

The root is a monorepo integration point, not a Rust workspace. Most
components are external repositories tracked as git submodules; directly
tracked applications, integration assets, `crates/common`, and
`crates/updater` remain root-owned.

```text
a3s/
├── apps/          Cloud, Desktop, docs, and Windhole applications
├── packages/      Office, Science, and UI
├── crates/        product hosts, capabilities, runtimes, services, and SDKs
├── compat/        exact cross-project revisions and protocol locks
├── assets/        repository-native README visuals
├── use-registry/  official signed A3S Use Registry deployment
└── homebrew-tap/  release formulae
```

<details>
<summary><strong>Browse components by concern</strong></summary>

| Group | Projects |
| --- | --- |
| Product hosts | [CLI](crates/cli/), [Code](crates/code/), [Desktop](apps/desktop/), [Ash](crates/ash/), [Windhole](apps/windhole/), [Cloud](apps/cloud/) |
| Retrieval and workspace intelligence | [Vec](crates/vec/), [Code workspace retrieval](crates/code/manual/WORKSPACE_RETRIEVAL_OPERATIONS.md) |
| Capabilities and content | [Use](crates/use/), [Browser](crates/browser/), [Search](crates/search/), [OCR](crates/ocr/), [Parser](crates/parser/), [Office](packages/office/), [Science](packages/science/) |
| Runtime, inference, and coordination | [Runtime](crates/runtime/), [Sandbox](crates/sandbox/), [Box](crates/box/), [OCI Runtime](crates/oci-runtime/), [Power](crates/power/), [MoE](crates/moe/), [Flow](crates/flow/), [Event](crates/event/), [Lane](crates/lane/), [Memory](crates/memory/), [ORM](crates/orm/) |
| Verification | [Bench](crates/bench/), [Test](crates/test/) |
| Interfaces and operations | [Boot](crates/boot/), [Gateway](crates/gateway/), [AHP](crates/ahp/), [ACL](crates/acl/), [TUI](crates/tui/), [GUI](crates/gui/), [UI](packages/ui/), [WebView](crates/webview/), [Observer](crates/observer/), [Sentry](crates/sentry/), [Updater](crates/updater/) |

</details>

The root-level Use Registry is a pinned deployment repository, not a package
source monorepo. [Use](crates/use/) owns Registry formats and tooling; package
source and builds remain with each owning repository.

The [CLI migration record](docs/cli-repository-migration.md) explains the
temporary 0.11.x root migration and restored standalone ownership. The
interactive [project directory](https://a3s-lab.github.io/a3s/#ecosystem)
shows each project's role, stage, release channel, website, and source.

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
- [Desktop download](https://a3s-lab.github.io/a3s/download/)
- [CLI reference](docs/cli-reference.md)
- [Cloud compatibility lock](compat/cloud-stack.acl)
- [Workflow architecture](compat/workflow-platform-architecture.md) and
  [ordered development plan](compat/workflow-platform-development-plan.md)
- [Local retrieval architecture](docs/retrieval-platform-architecture.md),
  [review](docs/retrieval-platform-architecture-review.md), and
  [roadmap](docs/retrieval-platform-roadmap.md)
- [CLI releases](https://github.com/A3S-Lab/CLI/releases)
- [Discord](https://discord.gg/XVg6Hu6H)

## License

This integration repository is licensed under the [MIT License](LICENSE).
Independently versioned projects retain the license declared by their owning
repositories.
