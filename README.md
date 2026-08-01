<p align="center">
  <img src="assets/readme/hero.svg" width="100%" alt="A3S CLI routes bundled Code, Web, and Research hosts plus managed Box, Search, Use, and Bench components">
</p>

<p align="center">
  <strong>One Rust CLI for A3S Code, Web, Research, shared configuration, and managed components.</strong>
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
  <a href="https://a3s-lab.github.io/a3s/">A3S CLI website</a>
</p>

---

A3S is the root-owned `a3s` CLI and the integration snapshot for independently
versioned A3S products and libraries. The CLI bundles Code, the Web host, and
the Research runner; it also manages configuration, accounts, models, component
lifecycle, diagnostics, and upgrades.

## Start with one command

Install the latest stable CLI on macOS or glibc Linux, then launch A3S Code in
the workspace it should inspect:

~~~bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh

cd /path/to/project
a3s code
~~~

The local CLI, Code, Web, Research, configuration, and component inspection do
not require an A3S OS login. A model-backed session still needs a configured
model provider or compatible local account.

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
| **Web** | `a3s web` | Bundled local Web API and release assets; `#home` opens the default workspace, Knowledge is a separate built-in destination, and `--api-only` needs no frontend |
| **Research** | `a3s code research --web "..."` | Bundled typed research runner with run-scoped Markdown and editable HTML artifacts |
| **Box** | `a3s box ps` | Managed `a3s-box` product; eligible first use may install it visibly |
| **Search** | `a3s search …` | Managed search product with quality-gated Browser-first discovery and lazy HTTP/RSS and API fallbacks |
| **Use** | `a3s use capabilities --json` | Managed capability facade with built-in Browser and OCR routes, a component-backed Box route, and external Office and Science packages |
| **Bench** | `a3s bench …` | Command and source are implemented; a compatible umbrella-CLI component release is not yet published, and local execution currently requires Docker |
| **Cloud** | Inspect the locked [Cloud compatibility manifest](compat/cloud-stack.acl) | Self-hosted control-plane project with separately documented maturity gates |

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

### A3S Web

`a3s web` serves the local A3S Web application when compatible assets are
available. The `#home` route opens its default workspace, while Knowledge is a
separate built-in destination. Research and Finance destinations are reviewed
A3S Use package contributions rather than hardcoded Web products.

The default workbench combines task conversations, Monaco editing, Git review,
local file management, and a resizable live-preview panel. It previews static
sites with debounced workspace reloads, loopback development servers, text,
images, PDFs, and Office files without opening a blocking dialog. Static-site
files remain confined to the active workspace and run in a sandboxed,
opaque-origin frame; URL targets are limited to localhost and loopback
addresses.

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

A3S Use owns the built-in Browser/OCR route projection, a component-backed Box
route, and the lifecycle and routing layer for external capability packages.
The independent Browser and OCR repositories own their provider contracts,
implementations, tests, and release assets. Office and Science remain external
packages with native CLI, MCP, and/or `SKILL.md` surfaces rather than depending
on a private extension protocol.

A3S Search ships quality-gated Browser-first discovery, lazy conventional
HTTP/RSS and native API fallbacks, and verifiable cascade receipts. A3S Office
currently exposes five browser-native surfaces plus native CLI, MCP, and Skill
automation; its Rust core also owns native OOXML semantics and optional
host-injected PDFium page rendering. It remains pre-1.0 and its first npm
release has not been published.
A3S Science currently indexes 472 catalog entries, including 35 A3S-native
Skills and 25 A3S-native MCP resources distributed through a TUF-signed package
registry.

A3S Parser is a pre-alpha agentic document parser built on A3S Code. It combines
A3S Office structure and source-layout rendering with A3S OCR through bounded,
source-bound governed tools; persists resumable Parser manifests and Code
snapshots; and emits a fine-grained canonical document graph whose immutable
canvases and normalized polygons support source highlighting in frontends. The
runnable CLI now covers native OOXML structure, direct images, exact-image
PPTX, and optional host-injected native PDF pages through Visual and Planned
routes. Rich Office layout, PDF text-layer fusion, broader imports, and
release-scale support claims remain roadmap work in the independently versioned
Parser repository.

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

A3S Box is the node-local OCI execution provider. Runtime integrations select
its concrete MicroVM or shared-kernel Sandbox backend explicitly, without an
automatic downgrade, while Cloud owns tenant state, placement, operations, and
cleanup for finite Executions. Box exposes local SDKs for Rust, Python,
TypeScript, and Go. Bench binds a Task, packaged Candidate adapter, and
task-owned Judge into an identity-bound result. Its local path currently
requires Docker and labels results `local_unofficial`; the source is present,
but the compatible umbrella-CLI component release is still pending. Runtime,
Flow, Event, Lane, Memory, ORM, Boot, and Gateway can also be used independently
as lower-level building blocks.

Isolation is explicit rather than implied: installing the umbrella CLI does not
make Docker, a hypervisor, browser engine, model, broker, database, or external
service available on an incompatible machine.

## Architecture

A3S is a collection of composable boundaries, not a mandatory vertical stack:

~~~text
terminal · browser · Rust / Node.js / Python SDKs
                         |
                 product hosts
        CLI · Code · Web · Research · Bench · Cloud · services
                         |
       governed agents · capabilities · durable state
        Code / Use       Flow / Event / Lane / Memory
                         |
                 Runtime contracts
                         |
        process · container · MicroVM · remote provider
~~~

The architecture follows five rules:

1. **Hosts own policy.** CLI, Code, Web, Research, Bench, and Cloud decide which
   models, tools, providers, permissions, and workflows are active.
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
| Web | Local-first and loopback by default; the default workspace and Knowledge are separate built-in destinations, and Office format fidelity depends on the exact editor and source feature |
| Research | Evidence is admitted only from fetched text or validated workspace sources; local-only mode remains network-free |
| Box | Requires a supported host and virtualization backend; platform-specific CRI, TEE, and Windows paths have separate gates |
| Bench | The source and `a3s bench …` route exist, but a compatible component release is not yet published; local execution requires Docker and produces `local_unofficial` results |
| Test | The deterministic Web runner, ACL admission, structured reports, and interrupt-safe browser cleanup are working; LLM planning, GUI/CUA, TUI/PTY, MCP, and Skill surfaces remain planned |
| Use | Domain readiness depends on installed runtimes and model assets; external packages own their compatibility |
| Office | Pre-1.0; five browser-native surfaces, native CLI/MCP/Skill automation, OOXML semantics, and optional host-injected PDFium page rendering exist, while the first npm package release is still pending |
| Parser | Pre-alpha runnable A3S Code parser; native OOXML structure, direct-image/exact-PPTX/native-PDF visual routes, canonical overlays, and durable resume are delivered, while richer formats and production-scale evidence remain gated |
| Cloud | R0–E0 is the verified cumulative baseline; G0, C0, and H0 are in progress; P0, A0, S0, and I0 remain planned in the [locked Cloud compatibility manifest](compat/cloud-stack.acl) |
| OCI Runtime | The default inventory remains `probe-only`; an explicit native Linux development path is experimental and is not the default workload-launch claim |
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

Cargo does not bundle the complete Web workspace or WebView helper. When
networking and automatic setup are allowed, the CLI can install exact-version
managed components on first use. Local Bash uses the built-in Rust guardrail;
no installation type downloads a local process sandbox. `A3S_OFFLINE=1` and
`A3S_NO_AUTO_INSTALL=1` keep automatic setup at zero network and zero mutation.

<details>
<summary><strong>What the release installers verify</strong></summary>

The Unix and Windows installers:

- resolve `latest` or an exact stable `vX.Y.Z` CLI release;
- require one exact artifact for the detected operating system and architecture;
- verify GitHub's SHA-256 release digest and the staged binary version;
- reject unexpected, duplicate, linked, or traversal archive entries;
- validate but never install the inert legacy self-update marker carried by
  current release archives;
- activate bundled Web and WebView payloads, plus support payloads from legacy
  releases, transactionally when the selected release includes them; and
- preserve the previous installation if staging or activation fails.

Supported controls include `A3S_VERSION`, `A3S_INSTALL_DIR`, `A3S_DATA_HOME`,
`A3S_MODIFY_PATH`, and `A3S_GITHUB_TOKEN`.

</details>

On macOS and Linux, a standalone installation can check or apply CLI updates
with `a3s self update --check` and `a3s self update`. Homebrew-managed
installations should use Homebrew.

Standalone versions 0.9.9 through 0.10.10 can update directly to 0.11.1 or
later through the retained `A3S-Lab/CLI` compatibility endpoint. The update
then switches future release checks to this repository.

## Repository map

This repository is the integration point for independently versioned submodules,
directly tracked applications, shared crates, compatibility locks, and
distribution assets.

### Products and applications

| Project | Role |
| --- | --- |
| [A3S CLI](src/) | Root-owned umbrella `a3s` command, Code TUI host, local Web API, configuration, authentication, and component lifecycle |
| [A3S Code](crates/code/) | Governed async agent runtime with Rust Core plus Node.js and Python SDKs |
| [A3S Web](apps/web/) | Local browser product with a default `#home` workspace, a separate Knowledge destination, and reviewed Use-package destinations |
| [A3S Desktop](apps/desktop/) | Independently versioned native desktop host tracked by the integration snapshot |
| [A3S Windhole](apps/windhole/) | Local visual laboratory for A3S Bench catalog, run, result, validation, and Doctor workflows |
| [A3S Box](crates/box/) | Node-local Linux OCI provider with explicit MicroVM or shared-kernel Sandbox selection and local SDKs |
| [A3S Bench](crates/bench/) | Reproducible Task, Candidate, and Judge evaluation; source implemented, compatible umbrella component release pending |
| [A3S Test](crates/test/) | Agent-ready typed E2E runtime with a working Web driver and shared contracts for planned GUI and TUI testing |
| [A3S Search](crates/search/) | Quality-gated Browser-first discovery, lazy HTTP/RSS and native API fallbacks, ranking, deduplication, and cascade receipts |
| [A3S Browser](crates/browser/) | Provider-oriented typed rendering plus the process-isolated automation driver, Skills, and Dashboard |
| [A3S OCR](crates/ocr/) | Object-safe `OcrProvider` contract with bounded source evidence and PP-OCRv6 as the default local provider |
| [A3S Parser](crates/parser/) | A3S Code-governed Office/OCR document parsing, resumable manifests, and MinerU-class source-locatable canonical geometry |
| [A3S Use](crates/use/) | Built-in Browser/OCR routes, component-backed Box routing, and standard lifecycle for external packages |
| [A3S Office](packages/office/) | Pre-1.0 Office package with five browser-native surfaces, native automation, OOXML semantics, and optional host-injected PDFium pages |
| [A3S Science](packages/science/) | TUF-signed 472-entry catalog with 35 A3S-native Skills, 25 A3S-native MCP resources, and scientific research tooling |
| [A3S Cloud](apps/cloud/) | Self-hosted control plane with durable tenant-scoped finite Executions, Runtime placement, cancellation, and cleanup |
| [A3S CLI website](apps/docs/) | Bilingual Rspress product site for installing, configuring, diagnosing, and launching the root-owned `a3s` CLI |

The [CLI repository migration record](docs/cli-repository-migration.md)
documents the imported source revision, preserved legacy history, and
main-repository release ownership.

### Runtime, coordination, and data

| Project | Role |
| --- | --- |
| [A3S Runtime](crates/runtime/) | Provider-neutral finite Task and long-running Service lifecycle |
| [A3S OCI Runtime](crates/oci-runtime/) | Probe-only default inventory with an explicit experimental native Linux development path |
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
just docs              # start the A3S CLI website
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

## Website and community

- A3S CLI website: [a3s-lab.github.io/a3s](https://a3s-lab.github.io/a3s/)
- CLI releases: [A3S-Lab/a3s releases](https://github.com/A3S-Lab/a3s/releases)
- Questions and discussion: [Discord](https://discord.gg/XVg6Hu6H)

Each project README records its detailed APIs, feature flags, platform
requirements, verification commands, and remaining limitations.

## License

This integration repository is licensed under the [MIT License](LICENSE).
Independently versioned projects retain the license declared by their owning
repositories.
