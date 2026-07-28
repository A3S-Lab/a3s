<p align="center">
  <img src="assets/readme/hero.svg" width="100%" alt="A3S routes one local CLI into governed Code, Web and Work, Research, Use, Box, Bench, and composable Rust foundations">
</p>

<p align="center">
  <strong>A Rust-native platform for governed agents, local AI work, and composable infrastructure.</strong>
</p>

<p align="center">
  <a href="https://github.com/A3S-Lab/a3s/actions/workflows/installers.yml"><img alt="Installer tests" src="https://img.shields.io/github/actions/workflow/status/A3S-Lab/a3s/installers.yml?branch=main&amp;style=flat-square&amp;label=installers"></a>
  <a href="https://github.com/A3S-Lab/a3s/releases"><img alt="Latest A3S CLI release" src="https://img.shields.io/crates/v/a3s?style=flat-square&amp;color=2864e8&amp;label=CLI"></a>
  <a href="https://crates.io/crates/a3s"><img alt="a3s on crates.io" src="https://img.shields.io/crates/v/a3s?style=flat-square&amp;color=5420bd"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust native" src="https://img.shields.io/badge/Rust-native-a4a8b2?style=flat-square"></a>
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-17181a?style=flat-square"></a>
</p>

<p align="center">
  <a href="#start-with-one-command">Start</a> ·
  <a href="#one-entry-point-explicit-products">Products</a> ·
  <a href="#architecture">Architecture</a> ·
  <a href="#repository-map">Repository map</a> ·
  <a href="#development">Development</a> ·
  <a href="https://a3s-lab.github.io/a3s/">Documentation</a>
</p>

---

A3S combines a unified `a3s` command with independently useful Rust products
and libraries. Start with a governed local coding agent, then add browser work,
knowledge compilation, research, isolated workloads, evaluation, or service
infrastructure only when the workflow needs them.

## Start with one command

Install the latest stable CLI on macOS or glibc Linux, then launch A3S Code in
the workspace it should inspect:

~~~bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh

cd /path/to/project
a3s code
~~~

Local Code, Web, configuration, component management, and Bench workflows do not
require an A3S OS login. A model-backed session still needs a configured model
provider or compatible local account.

After installation, these commands show the active paths and create a starter
[A3S Agent Configuration Language](crates/acl/) configuration:

~~~bash
a3s config path
a3s config init
a3s config validate
a3s model list
~~~

Select a validated source-qualified model with
`a3s model use <provider>/<model>`. See [Installation options](#installation-options)
for Windows, Homebrew, Cargo, offline preparation, and update behavior.

## One entry point, explicit products

The umbrella CLI owns configuration, authentication, component discovery, and
command routing. Product behavior remains with the component that implements it.

| Surface | First action | Delivery boundary |
| --- | --- | --- |
| **Code** | `a3s code` | Bundled governed agent runtime and terminal workspace |
| **Web + Work** | `a3s web` | Local browser workspace; full release bundles carry matching Web assets, while API-only mode needs no frontend |
| **Research** | `a3s code research --web "..."` | Bundled typed research runner with run-scoped Markdown and editable HTML artifacts |
| **Box** | `a3s box ps` | Optional isolated-workload product; eligible first use may install it visibly |
| **Bench** | `a3s install bench` | Optional evaluation product with explicit installation |
| **Search** | `a3s install search` | Optional meta-search product with explicit installation |
| **Use** | `a3s install use` | Optional capability facade; Browser and OCR routes are built in from independently versioned repositories, while Office and other domains keep independent package contracts |
| **Cloud** | Follow the [versioned Cloud guide](apps/docs/content/docs/en/cloud/v0.1.0/) | Self-hosted control-plane project with separately documented maturity gates |

A catalog entry describes discovery and installation policy. It is not proof
that every platform or release channel currently contains a compatible artifact.
Use `a3s list` and `a3s doctor` to inspect the machine before scripting an
optional product.

## What A3S provides

### Governed agent work

A3S Code runs interactive or headless agent sessions with workspace tools,
risk-aware permissions, persistence, context management, memory, delegation,
verification, and dynamic workflows. The terminal and browser hosts share Code
Core while keeping their own presentation.

Default, Plan, and Auto modes express different execution boundaries. Project
permissions are explicit ACL data, delegated work remains visible, and session
state can be resumed instead of reconstructed from a transcript.

~~~bash
a3s code
a3s code resume
a3s code exec "Summarize the public API and run its focused tests."
~~~

The complete TUI, permission, session, and component command reference lives in
the [CLI reference](docs/cli-reference.md).

### Local Web, Work, and knowledge

`a3s web` serves the local Code workspace and the Work product when compatible
assets are available. The current Web surface combines task conversations,
Monaco editing, Git review, local file management, knowledge-library creation
and compilation workflows, and native document, spreadsheet, presentation, and
PDF work backed by A3S Office.

Work can keep a resizable live-preview panel beside the file manager or code
editor. It previews static sites with debounced workspace reloads, loopback
development servers, text, images, PDFs, and Office files without opening a
blocking dialog. Static-site files remain confined to the active workspace and
run in a sandboxed, opaque-origin frame; URL targets are limited to localhost
and loopback addresses.

~~~bash
a3s web
a3s web status
a3s web logs
a3s web stop

# Run only the loopback Code API.
a3s web --api-only
~~~

The server binds to loopback by default. Do not expose workspace APIs directly
to an untrusted network; put an authenticated gateway in front of any deliberate
remote deployment.

### Evidence-first research

The CLI, TUI, and Web use the same typed DeepResearch runner. Web research admits
fetched evidence; local-only research stays within validated workspace sources.
Each run publishes a bounded event journal, `report.md`, and an editable
`index.html` under `.a3s/research/`.

~~~bash
a3s code research --web "Compare Tokio and async-std"
a3s code research --local-only "Map this repository's release process"
~~~

The runner reports whether the result is synthesized, qualified, source-backed,
or explicit no-evidence output. It does not silently turn missing evidence into
a confident answer.

### Typed capabilities and components

A3S Use owns the built-in Browser/OCR route projection plus the lifecycle and
routing layer for external capability packages. The independent Browser and OCR
repositories own their provider contracts, implementations, tests, and release
assets. External packages retain standard native CLI, MCP, and/or `SKILL.md`
surfaces rather than depending on a private extension protocol.

~~~bash
a3s list
a3s doctor

a3s install use
a3s use capabilities --json
a3s use browser doctor
a3s use ocr doctor --json

a3s upgrade
a3s upgrade --all --yes
~~~

Component mutations resolve typed IDs, verify provenance, and modify only
component-owned files. They are not a general-purpose package manager.

### Isolation, evaluation, and services

A3S Box runs Linux OCI workloads through Docker-like MicroVM workflows on
supported virtualization hosts. Bench binds a Task, packaged Candidate adapter,
and task-owned Judge into an identity-bound result. Runtime, Flow, Event, Lane,
Memory, ORM, Boot, and Gateway can also be used independently as lower-level
building blocks.

Isolation is explicit rather than implied: installing the umbrella CLI does not
make Docker, a hypervisor, browser engine, model, broker, database, or external
service available on an incompatible machine.

## Architecture

A3S is a collection of composable boundaries, not a mandatory vertical stack:

~~~text
terminal · browser · Rust / Node.js / Python SDKs
                         |
                 product hosts
        CLI · Code Web · Bench · Cloud · services
                         |
       governed agents · capabilities · durable state
        Code / Use       Flow / Event / Lane / Memory
                         |
                 Runtime contracts
                         |
        process · container · MicroVM · remote provider
~~~

The architecture follows five rules:

1. **Hosts own policy.** CLI, Web, Bench, and Cloud decide which models, tools,
   providers, permissions, and workflows are active.
2. **Core contracts stay replaceable.** Runtime drivers, event providers, memory
   stores, SQL executors, HTTP adapters, and capability providers use explicit
   interfaces.
3. **Durable systems persist identity.** Sessions, workflow runs, runtime units,
   evaluation results, and Cloud operations do not rely only on process memory.
4. **External dependencies stay visible.** Accounts, credentials, browsers,
   databases, brokers, hypervisors, hardware, and model providers are never
   treated as hidden defaults.
5. **Policy and enforcement remain separate.** Code owns permission routing,
   sandbox providers enforce local command boundaries, Runtime owns lifecycle,
   Box owns OCI product policy, and concrete drivers own infrastructure.

Configuration is ACL parsed and generated by `a3s-acl`. Do not treat ACL as HCL
or feed it to an HCL parser.

## Product boundaries

A3S is actively developed, and the repository includes both production-facing
surfaces and explicit foundations. The following boundaries prevent a directory,
type, or parsed configuration from being mistaken for a finished deployment.

| Area | Current boundary |
| --- | --- |
| Code | Model execution requires a configured provider or compatible account; remote OS actions require login |
| Web + Work | Local-first and loopback by default; Office format fidelity depends on the exact editor and source feature |
| Research | Evidence is admitted only from fetched text or validated workspace sources; local-only mode remains network-free |
| Box | Requires a supported host and virtualization backend; platform-specific CRI, TEE, and Windows paths have separate gates |
| Bench | The local path requires Docker and produces `local_unofficial` results; official evaluation requires matching admission and Runtime evidence |
| Use | Domain readiness depends on installed runtimes and model assets; external packages own their compatibility |
| Cloud | Delivered, experimental, and planned behavior is separated in the [versioned Cloud documentation](apps/docs/content/docs/en/cloud/v0.1.0/) |
| OCI Runtime | Experimental: every current platform driver remains `probe-only`; host detection is not workload-launch support |
| Infrastructure libraries | Optional features expose integrations; external brokers, stores, providers, and services must still be operated |

A3S is also not one root Cargo workspace, one monolithic binary, or one shared
release version. Release-bearing projects publish on their owning cadence, and
their local manifests and READMEs remain the source of truth.

## Installation options

### macOS and Linux

The release installer writes to `~/.local/bin` by default and does not edit the
shell profile unless requested:

~~~bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh

# Opt in to a persistent PATH update.
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | A3S_MODIFY_PATH=1 sh
~~~

Homebrew remains available on macOS and Linux:

~~~bash
brew install a3s-lab/tap/a3s
~~~

### Windows

Run the installer from PowerShell 5.1 or newer:

~~~powershell
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
irm https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.ps1 | iex
~~~

It writes to `%LOCALAPPDATA%\Programs\a3s\bin` by default and prints the exact
`PATH` instruction when needed. Rerun the installer to upgrade on Windows;
in-place self-update is not yet supported there.

### Cargo

Use Cargo when a binary-only installation is intentional:

~~~bash
cargo install a3s
~~~

Cargo does not bundle the complete Web workspace, WebView helper, or managed
sandbox support tree. When networking and automatic setup are allowed, the CLI
can install exact-version managed components on first use. `A3S_OFFLINE=1` and
`A3S_NO_AUTO_INSTALL=1` keep that boundary at zero network and zero mutation.

<details>
<summary><strong>What the release installers verify</strong></summary>

The Unix and Windows installers:

- resolve `latest` or an exact stable `vX.Y.Z` CLI release;
- require one exact artifact for the detected operating system and architecture;
- verify GitHub's SHA-256 release digest and the staged binary version;
- reject unexpected, duplicate, linked, or traversal archive entries;
- activate bundled Web, WebView, and support payloads transactionally when the
  selected release includes them; and
- preserve the previous installation if staging or activation fails.

Supported controls include `A3S_VERSION`, `A3S_INSTALL_DIR`, `A3S_DATA_HOME`,
`A3S_MODIFY_PATH`, and `A3S_GITHUB_TOKEN`.

</details>

On macOS and Linux, a standalone installation can check or apply CLI updates
with `a3s self update --check` and `a3s self update`. Homebrew-managed
installations should use Homebrew.

## Repository map

This repository is the integration point for independently versioned submodules,
directly tracked applications, shared crates, compatibility locks, and
distribution assets.

### Products and applications

| Project | Role |
| --- | --- |
| [A3S CLI](src/) | Root-owned umbrella `a3s` command, Code TUI host, local Web API, configuration, authentication, and component lifecycle |
| [A3S Code](crates/code/) | Governed async agent runtime with Rust Core plus Node.js and Python SDKs |
| [A3S Web](apps/web/) | Local Code, Work, file, knowledge, Office, and research browser surfaces |
| [A3S Windhole](apps/windhole/) | Local visual laboratory for A3S Bench catalog, run, result, validation, and Doctor workflows |
| [A3S Box](crates/box/) | Docker-like MicroVM product for Linux OCI workloads |
| [A3S Bench](crates/bench/) | Reproducible Task, Candidate, and Judge evaluation |
| [A3S Search](crates/search/) | Multi-engine retrieval, ranking, deduplication, and optional browser rendering |
| [A3S Browser](crates/browser/) | Provider-oriented typed rendering plus the process-isolated automation driver, Skills, and Dashboard |
| [A3S OCR](crates/ocr/) | Object-safe `OcrProvider` contract with bounded source evidence and PP-OCRv6 as the default local provider |
| [A3S Use](crates/use/) | Built-in Browser/OCR route facade and standard lifecycle for external capability packages |
| [A3S Office](packages/office/) | Native OOXML engine, Office editors, CLI, MCP, Skill, and A3S Use package |
| [A3S Science](packages/science/) | Independently versioned scientific Skills, MCP data services, compute workflows, and research tooling |
| [A3S Cloud](apps/cloud/) | Self-hosted control plane for desired state, durable operations, nodes, and verified OCI deployment |
| [Documentation](apps/docs/) | Documentation, tutorials, project references, and versioned Cloud operations guidance |

The [CLI repository migration record](docs/cli-repository-migration.md)
documents the imported source revision, preserved legacy history, and
main-repository release ownership.

### Runtime, coordination, and data

| Project | Role |
| --- | --- |
| [A3S Runtime](crates/runtime/) | Provider-neutral finite Task and long-running Service lifecycle |
| [A3S OCI Runtime](crates/oci-runtime/) | Experimental cross-platform OCI lifecycle and isolation driver foundation |
| [A3S Flow](crates/flow/) | Event-sourced durable workflows with replay-safe steps, waits, retries, and workers |
| [A3S Event](crates/event/) | Provider-neutral publish, subscribe, history, and persistence |
| [A3S Lane](crates/lane/) | Priority-lane async scheduling with bounded concurrency and retry |
| [A3S Memory](crates/memory/) | Pluggable agent memory with optional SQLite full-text and vector search |
| [A3S ORM](crates/orm/) | Immutable, parameterized, type-safe SQL builder and async drivers |
| [A3S Common](crates/common/) | Shared privacy, tool, transport, and protocol types |

### Services, interfaces, and operations

| Project | Role |
| --- | --- |
| [A3S Boot](crates/boot/) | Adapter-first modular async service framework |
| [A3S Gateway](crates/gateway/) | Local AI traffic and protocol data plane |
| [A3S Power](crates/power/) | Privacy-oriented model inference components |
| [A3S AHP](crates/ahp/) | Transport-neutral Agent Harness Protocol supervision |
| [A3S ACL](crates/acl/) | Parser and generator for the A3S Agent Configuration Language |
| [A3S TUI](crates/tui/) | TEA-style terminal UI framework |
| [A3S GUI](crates/gui/) | Browser-free native RSX and reducer runtime |
| [A3S WebView](crates/webview/) | Authenticated RemoteUI and native Agent Island helper |
| [A3S Observer](crates/observer/) | Language-neutral observations and Linux eBPF collection |
| [A3S Sentry](crates/sentry/) | Tiered runtime security controls over observed activity |
| [A3S Updater](crates/updater/) | Self-update and signed, health-gated fleet lifecycle primitives |
| [Homebrew Tap](homebrew-tap/) | Formulae for released A3S commands and helpers |

## Development

Clone the exact integration snapshot with its registered submodules:

~~~bash
git clone --recurse-submodules git@github.com:A3S-Lab/a3s.git
cd a3s

# For an existing checkout.
git submodule update --init --recursive
~~~

The root `justfile` orchestrates common entry points:

~~~bash
just code              # build the local helper and run A3S Code
just web               # build and run the browser workspace
just docs              # start the documentation site
just windhole          # start the Bench visual laboratory
just use-hotplug-e2e   # verify Use hot-plug and release-shaped first use
just cloud-stack-check # verify the locked Cloud integration stack
~~~

> [!IMPORTANT]
> The repository root is the `a3s` CLI package, not a Cargo workspace.
> Root-level Cargo commands validate the CLI only. Work inside the relevant
> submodule, package, or application for every other project.

A typical CLI validation runs from the repository root:

~~~bash
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
~~~

Other Rust project validation starts from that project's workspace:

~~~bash
cd crates/<project>
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
~~~

The directly tracked Web application uses Bun:

~~~bash
cd apps/web
bun install --frozen-lockfile
bun run format:check
bun run lint:check
bun run typecheck
bun run test
bun run build
~~~

Installer validation is self-contained:

~~~bash
bash scripts/test-install.sh
~~~

~~~powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/test-install.ps1
~~~

Submodules and the root repository have separate Git histories. Commit a
submodule change in its owning repository before updating its gitlink here.
Read [AGENTS.md](AGENTS.md) before adding crates or changing repository
structure.

## Documentation and community

- Documentation: [a3s-lab.github.io/a3s](https://a3s-lab.github.io/a3s/)
- CLI releases: [A3S-Lab/a3s releases](https://github.com/A3S-Lab/a3s/releases)
- Questions and discussion: [Discord](https://discord.gg/XVg6Hu6H)

Each project README records its detailed APIs, feature flags, platform
requirements, verification commands, and remaining limitations.

## License

This integration repository is licensed under the [MIT License](LICENSE).
Independently versioned projects retain the license declared by their owning
repositories.
