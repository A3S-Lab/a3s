<p align="center">
  <img src="assets/readme/hero.svg" width="100%" alt="A3S is a local-first, open-source runtime platform for agent work">
</p>

<p align="center">
  <strong>Language / 语言:</strong>
  <a href="README.md">English</a> ·
  <a href="README.zh-CN.md">中文</a>
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
  <a href="#request-path">Request path</a> ·
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

Start with one local Code session. Add a signed capability, a durable workflow,
an isolated Box workload, or Cloud coordination only when the work requires it.

> [!IMPORTANT]
> This repository is the reviewed integration snapshot for independently
> versioned A3S products. Most components are pinned here as git submodules;
> the root owns installers, orchestration, compatibility locks, and shared
> documentation. It is not a Rust workspace or a second copy of each product.

## Start local

The product entry is **`a3s`**; interactive Code is **`a3s code`**. Prefer one
install channel. Homebrew (macOS/Linux):

```bash
brew tap a3s-lab/tap https://github.com/A3S-Lab/homebrew-tap
brew install a3s
cd /path/to/project
a3s code
```

Or the official installer (auto-prefers Homebrew when available, otherwise the
GitHub binary):

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

See [Installation](#installation) for install, update, and uninstall on every
supported platform (Homebrew, official installers, Cargo), plus offline notes.
The complete command surface lives in the [CLI reference](docs/cli-reference.md).

## Request path

<p align="center">
  <img src="assets/readme/workflow.svg" width="100%" alt="A3S request path from a product host through explicit policy and capabilities to replaceable runtime providers and evidence">
</p>

The host chooses the authority. ACL and Use describe what the work may do;
Code, Flow, Runtime, and Box carry it out through typed contracts. Health,
digests, revisions, and outcomes return as evidence that the owner can inspect.

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
| **Desktop** | [Download](https://a3s-lab.github.io/a3s/download/) · [Releases](https://github.com/A3S-Lab/a3s/releases) · [Source](apps/desktop/) | Native workbench powered by the local Code kernel; installers are published in this repository |
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

The root repository also owns A3S Desktop distribution. Tags named
`desktop-vX.Y.Z` publish platform installers and signed Tauri updater artifacts to the root GitHub Release;
the release workflow refreshes the `desktop-latest` aliases consumed by the website download
page and the in-app updater. Desktop updates install automatically and restart the application
after a verified bundle is downloaded.

For a local package, run `cd apps/desktop && npm run package:local`; this cleans stale bundle output,
builds the platform-native Tauri artifacts, and verifies the files before they are shared. The
complete signing, updater, alias, and public-feed sequence is documented in
[`docs/desktop-release.md`](docs/desktop-release.md).
| Area | Current posture | Source of truth |
| --- | --- | --- |
| CLI and installers | The standalone CLI repository owns source, CI, releases, and detailed product documentation; this root pins a reviewed revision and relays only the required integration assets. | [CLI](https://github.com/A3S-Lab/CLI) · [installer CI](.github/workflows/installers.yml) |
| Local agent work | Code owns session, workspace, retrieval, context, memory, and provider qualification, including typed provider/model generation capacity and bounded pool-health evidence admitted through its shared scheduler and projected consistently to Rust, Node.js, Python, and Go hosts. Provider and public-network behavior is qualified separately from the local core. | [Code](crates/code/) · [retrieval roadmap](docs/retrieval-platform-roadmap.md) |
| Native command isolation | Sandbox owns the fail-closed macOS, Linux, and Windows command boundary. Code consumes its small Rust contract without Node.js or a legacy runtime compatibility layer. | [Sandbox](crates/sandbox/) · [security model](crates/sandbox/SECURITY.md) |
| Cloud and workflows | Exact component revisions, package versions, and protocol levels are locked together before an integration claim is published. | [Cloud stack lock](compat/cloud-stack.acl) · [workflow plan](compat/workflow-platform-development-plan.md) |
| Capabilities | Use is a preview. Discovery does not imply installation authority, activation, or operated Registry readiness. | [Use](crates/use/) · [Use Registry](use-registry/) |
| Isolation and inference | Platform, driver, accelerator, and artifact evidence is explicit. Model support does not imply that every optimized kernel or backend is complete. | [OCI Runtime](crates/oci-runtime/) · [Runtime](crates/runtime/) · [Power](crates/power/) · [MoE](crates/moe/) |
| Early-stage surfaces | Ash and Parser remain early-stage; Office remains pre-1.0. | [Ash](crates/ash/) · [Parser](crates/parser/) · [Office](packages/office/) |

Component READMEs, releases, roadmaps, and compatibility locks carry exact
versions, platforms, fixtures, and remaining gates. This page explains how the
parts compose; it is not a merged changelog.

### Current integration focus

The latest reviewed component revisions keep the active work visible at the
same boundaries:

| Area | What is being hardened now |
| --- | --- |
| **Code** | Reviewer inputs reject malformed line boundaries, and findings stay bound to the admitted run and immutable evidence. |
| **Box** | `v3.2.6` Linux Sandbox GA: default `SandboxViaOci` for `--isolation sandbox` (no `A3S_BOX_OCI_MIGRATION`), host prep + evidence binder; warm-pool/CRI orphan reap on destroy failure. Not a MicroVM/`BX0.3` claim. |
| **Integration** | The root advances component gitlinks independently; [`compat/cloud-stack.acl`](compat/cloud-stack.acl) remains the source of truth for exact versions and protocol levels. |

These are component-level contracts, not a blanket support claim. Check the
owning repository for platform, release, and qualification details.

## Installation

The product entry is the **`a3s` CLI**. Interactive Code sessions start with
`a3s code`. Pick **one** install channel and keep updates on that channel so
PATH does not shadow a second copy.

### Supported platforms

| OS | Architectures | Delivery |
| --- | --- | --- |
| macOS 12+ | Apple Silicon (`aarch64`), Intel (`x86_64`) | Official installer, Homebrew, or Cargo |
| Linux (glibc) | `x86_64`, `aarch64` | Official installer, Homebrew, or Cargo |
| Windows 10/11 | `x64` (`x86_64`) only | PowerShell installer or Cargo |

Not shipped for the umbrella CLI today: musl/Alpine Linux, Windows ARM, Mingw,
or Cygwin. Use a supported host or build from source with Cargo where the
toolchain allows.

Release installers resolve one stable SemVer, require an exact artifact for the
detected platform, verify the published SHA-256 and staged `a3s --version`,
reject unsafe archive members, and keep the previous install if activation
fails. They never use `sudo` or UAC.

### Homebrew (macOS and Linux)

Preferred package-manager path. Installs `a3s`, `a3s-webview`, the bundled
`moli/` runtime, and `libzvec` from the CLI release archive.

```bash
brew tap a3s-lab/tap https://github.com/A3S-Lab/homebrew-tap
brew install a3s

# Equivalent one-liner:
# brew install a3s-lab/tap/a3s

a3s --version
a3s code
```

```bash
# Update
brew update && brew upgrade a3s

# Uninstall the CLI formula
brew uninstall a3s

# Optional: remove the tap after uninstalling its formulae
brew untap a3s-lab/tap
```

Do **not** use `brew install a3s-code` for this product. That formula is the
**legacy** standalone `a3s-code` binary and does **not** provide `a3s`.

Other tap formulae (`a3s-box`, `a3s-search`, `a3s-power`, …) are separate
products; see [homebrew-tap](homebrew-tap/README.md).

### Official installer — macOS and glibc Linux

Recommended one-liner. Detects OS/arch, inventories existing installs, and
chooses a channel:

- **`auto` (default):** if Homebrew is on `PATH`, cleans legacy conflicts
  (`a3s-code`, standalone `a3s-webview`), installs `a3s-lab/tap/a3s`, and removes
  shadowing `~/.local/bin` copies so `which a3s` resolves to Homebrew.
- **Otherwise:** installs the GitHub release archive into `~/.local/bin`.

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh
```

```bash
# Inspect only (no changes)
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh -s -- --dry-run

# Force GitHub binary even when brew exists
curl ... | sh -s -- --channel binary

# Force Homebrew (fails if brew is missing)
curl ... | sh -s -- --channel brew
```

Binary-channel options: `A3S_VERSION=vX.Y.Z`, `A3S_INSTALL_DIR=/absolute/path`,
`A3S_MODIFY_PATH=1` (append `~/.local/bin` to a shell profile). Also:
`A3S_CHANNEL`, `A3S_YES`, `A3S_DRY_RUN`, `A3S_GITHUB_TOKEN`.

```bash
# Update
# Homebrew channel: brew update && brew upgrade a3s
# Binary channel:   a3s self update   # or re-run install.sh --channel binary

# Uninstall binary channel
rm -f ~/.local/bin/a3s ~/.local/bin/a3s-webview ~/.local/bin/a3s-code
rm -rf ~/.local/bin/moli
# Homebrew channel: brew uninstall a3s
```

#### Migrating from older macOS / Linux installs

If `brew upgrade a3s` fails with a symlink error on `a3s-webview`, or you still
have the legacy `a3s-code` formula, re-run the official installer (or):

```bash
brew uninstall a3s-code 2>/dev/null || true
brew uninstall a3s-webview 2>/dev/null || true
brew uninstall --force a3s 2>/dev/null || true
brew tap a3s-lab/tap https://github.com/A3S-Lab/homebrew-tap
brew install a3s-lab/tap/a3s
```

Do **not** use `brew install a3s-code` for this product.

### Official installer — Windows x64 (PowerShell 5.1+)

```powershell
irm https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.ps1 | iex
```

Prints an inventory of existing `a3s` / legacy paths, then installs the GitHub
zip into `%LOCALAPPDATA%\Programs\a3s\bin`. Set `$env:A3S_MODIFY_PATH = '1'`
before running to update the user PATH. Overrides: `A3S_VERSION`,
`A3S_INSTALL_DIR`, `A3S_GITHUB_TOKEN`.

```powershell
# Update — re-run the installer
irm https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.ps1 | iex

# Uninstall — close running a3s processes, then remove the install tree
Remove-Item -Recurse -Force "$env:LOCALAPPDATA\Programs\a3s"
# If PATH was modified, remove that directory from the user PATH environment variable.
```

### Cargo (any host with a Rust toolchain)

```bash
cargo install a3s --locked
```

```bash
# Update
cargo install a3s --locked

# Uninstall
cargo uninstall a3s
```

Cargo builds the CLI crate from crates.io. Release companions (`a3s-webview`,
bundled `moli/`) are not always present the way they are in GitHub or Homebrew
archives; prefer the official installer or Homebrew when you need the full
release layout.

### Verify and first session

```bash
a3s --version
which a3s   # Unix: confirm a single expected path
a3s code
```

If `a3s` is missing or the wrong binary runs, another install is earlier on
`PATH` (common when Homebrew and `~/.local/bin` both have copies).

User configuration under `~/.a3s/` (or the Windows equivalent) is **not**
removed when you uninstall the binary. Delete that directory only if you want
to wipe local config and caches.

### Offline and no-mutation setup

Set `A3S_OFFLINE=1` and `A3S_NO_AUTO_INSTALL=1` when setup must perform zero
network access and zero component mutation. That constrains runtime component
installs; it does not replace choosing an offline-capable binary delivery.

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
just desktop
just desktop-check
just desktop-package
just code
just desktop-web
just docs
just windhole
just use-hotplug-e2e
just cloud-stack-check
```

Desktop keeps its JavaScript dependency graph in
`apps/desktop/package-lock.json`. Run `cd apps/desktop && npm ci` once before
using the desktop recipes; Tauri invokes Cargo for the native backend, while
`just` remains the repository task runner.

Submodules and the root have separate histories. Commit a component change in
its owning repository before advancing its gitlink here, and read
[AGENTS.md](AGENTS.md) before changing repository structure.

## Documentation and community

- [A3S website](https://a3s-lab.github.io/a3s/)
- [Desktop download](https://a3s-lab.github.io/a3s/download/)
- [Desktop release and update contract](docs/desktop-release.md)
- [Capability expansion implementation path](docs/capability-expansion-implementation-path.md)
- [Desktop Applet plugin path](docs/desktop-applet-plugin-path.md) (per-project stubs: [Desktop P0](apps/desktop/docs/applet-uihost-roadmap.md), [Use P1](crates/use/docs/applet-ui-supply.md), [Code P2](crates/code/docs/applet-ui-freeze.md), [CLI P3](crates/cli/docs/applet-ui-projection-parity.md), [Box P4](crates/box/docs/applet-backend-boundary.md), [WebView P4b](crates/webview/docs/applet-webview-substrate.md), [Registry P5](use-registry/docs/applet-ui-package-supply.md), [Cloud P6](apps/cloud/docs/applet-ui-assignment.md))
- [CLI reference](docs/cli-reference.md)
- [Cloud compatibility lock](compat/cloud-stack.acl)
- [Workflow architecture](compat/workflow-platform-architecture.md) and
  [ordered development plan](compat/workflow-platform-development-plan.md)
- [Local retrieval architecture](docs/retrieval-platform-architecture.md),
  [review](docs/retrieval-platform-architecture-review.md), and
  [roadmap](docs/retrieval-platform-roadmap.md)
- [Scientific discovery platform roadmap](docs/scientific-discovery-platform-roadmap.md)
- [A3S Code Core optimization roadmap](docs/a3s-code-core-optimization-roadmap.md)
- [Terminal-Bench 4.0 evaluation (Harbor + A3S Code)](scripts/harbor/EVALUATION.md)
- [CLI releases](https://github.com/A3S-Lab/CLI/releases)
- [Discord](https://discord.gg/XVg6Hu6H)

## License

This integration repository is licensed under the [MIT License](LICENSE).
Independently versioned projects retain the license declared by their owning
repositories.
