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
  <a href="#start">Start</a> ·
  <a href="#choose-a-surface">Surfaces</a> ·
  <a href="#how-a-request-moves">Request path</a> ·
  <a href="#install">Install</a> ·
  <a href="#repository">Repository</a> ·
  <a href="https://a3s-lab.github.io/a3s/">Website</a>
</p>

A3S is an open-source, local-first runtime for agent work. A session names its
model, tools, and permissions; work runs through typed contracts; isolation and
Cloud stay off until that work needs them.

> [!IMPORTANT]
> This repository is the reviewed integration snapshot. Most products are pinned
> here as git submodules. The root owns installers, orchestration, compatibility
> locks, and this homepage. It is not a Rust workspace and not a second copy of
> each product.

## Start

The product command is **`a3s`**. An interactive session is **`a3s code`**.
Use one install channel.

```bash
brew tap a3s-lab/tap https://github.com/A3S-Lab/homebrew-tap
brew install a3s
cd /path/to/project
a3s code
```

A model-backed session needs an explicit provider:

```bash
a3s config init
a3s config validate
a3s model list
a3s model use <provider>/<model>
```

| Intent | Command |
| --- | --- |
| Interactive session | `a3s code` |
| One bounded task | `a3s code exec "Check the API boundary and run its focused tests."` |
| Evidence-backed research note | `a3s code research --web "Compare the implementation with its design"` |
| See isolated workloads | `a3s box ps` |
| See what is installed | `a3s list --installed` |

Prefer a native window? Download [A3S Desktop](https://a3s-lab.github.io/a3s/download/).
It is a Tauri workbench over the local Code kernel, not a wrapper around the
`a3s` binary. Other install channels are in [Install](#install).

## Choose a surface

Start with Code. Add another surface only when the job needs that owner.

| Surface | Start | Owns | Does not own |
| --- | --- | --- | --- |
| **Code** | `a3s code` | Local agent sessions, workspace, memory, and providers | Isolation mechanics, control-plane desired state |
| **Desktop** | [Download](https://a3s-lab.github.io/a3s/download/) · [source](apps/desktop/) | Native workbench: sessions, documents, files, and research settings | A substitute for host Python or R. Analysis runs only on a published a3s-box image generation. A catalog name without a lock is not installed. |
| **Box** | `a3s box ps` | Local MicroVM isolation and OCI workloads | Silent activation just because Code is installed |
| **Use** | `a3s use capabilities --json` | Signed capability graphs | Installation, activation, or Registry readiness. Discovery is not a lease. Preview. |
| **Cloud** | [`compat/cloud-stack.acl`](compat/cloud-stack.acl) | Self-hosted control plane under an exact compatibility lock | The default path for a local session |

> [!NOTE]
> A catalog record can describe a component without proving that every platform
> or release channel contains a compatible artifact.

## How a request moves

<p align="center">
  <img src="assets/readme/workflow.svg" width="100%" alt="A request moves from a product host through explicit policy and capabilities to replaceable runtime providers and evidence">
</p>

The host chooses the authority. [ACL](crates/acl/) and [Use](crates/use/)
describe what the work may do. Code, Flow, Runtime, and Box carry it out.
Health, digests, revisions, and receipts come back as evidence the owner can
inspect. ACL is the Agent Configuration Language. It is not HCL; parse and
generate it with `a3s-acl`.

<p align="center">
  <img src="assets/readme/architecture.svg" width="100%" alt="Product hosts set policy, agent and workflow contracts compose work, replaceable providers execute it, and evidence returns to the owning authority">
</p>

Four rules keep the parts replaceable:

1. Hosts own policy. Backends do not invent authority.
2. One concern has one owner. Cloud owns desired state, Flow owns durable
   orchestration, Runtime owns provider-neutral lifecycles, and concrete
   providers own enforcement.
3. Processes, containers, MicroVMs, and remote providers meet typed contracts.
   None of them is a hidden default.
4. A digest, a revision, and a receipt are different facts. Installation is
   not authorization, and authorization is not a completed run.

Support claims live with the component that owns the behavior. This page does
not assign one maturity label to the whole system.

| Claim | Where the truth lives |
| --- | --- |
| CLI releases and installers | [CLI](https://github.com/A3S-Lab/CLI) · [installer CI](.github/workflows/installers.yml) |
| Desktop installers and updates | Tags `desktop-vX.Y.Z` on this repository. Signing, notarization, and the updater feed: [desktop release](docs/desktop-release.md). |
| Command isolation | [Sandbox](crates/sandbox/) |
| Cloud and workflow versions | [Cloud stack lock](compat/cloud-stack.acl) |
| Capabilities | [Use](crates/use/) · [Use Registry](use-registry/) |

## Install

Pick **one** channel and stay on it. A second copy earlier on `PATH` is the
usual reason `a3s` looks installed but is the wrong binary.

| OS | Architectures | Channels |
| --- | --- | --- |
| macOS 12+ | Apple Silicon, Intel | Installer, Homebrew, Cargo |
| Linux (glibc) | `x86_64`, `aarch64` | Installer, Homebrew, Cargo |
| Windows 10/11 | `x64` only | PowerShell installer, Cargo |

Not shipped for the umbrella CLI: musl/Alpine, Windows ARM, MinGW, or Cygwin.
Installers resolve one SemVer, require the platform artifact, verify SHA-256
and `a3s --version`, and keep the previous install if activation fails. They
do not use `sudo` or UAC.

### Homebrew (macOS and Linux)

Installs `a3s`, `a3s-webview`, the bundled `moli/` runtime, and `libzvec`.

```bash
brew tap a3s-lab/tap https://github.com/A3S-Lab/homebrew-tap
brew install a3s
a3s --version
```

```bash
brew update && brew upgrade a3s
brew uninstall a3s
```

Do **not** `brew install a3s-code`. That formula is the legacy standalone
binary and does not provide `a3s`. Other tap formulae (`a3s-box`,
`a3s-search`, `a3s-power`) are separate products.

If an upgrade fails on an `a3s-webview` symlink, or the old `a3s-code` formula
is still present:

```bash
brew uninstall a3s-code 2>/dev/null || true
brew uninstall a3s-webview 2>/dev/null || true
brew uninstall --force a3s 2>/dev/null || true
brew install a3s-lab/tap/a3s
```

### Installer

On macOS and glibc Linux, Homebrew is used when it is on `PATH`. Otherwise the
GitHub archive is installed into `~/.local/bin`.

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh
```

```bash
# Inspect only
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh -s -- --dry-run
```

`--channel binary` forces the GitHub archive. `--channel brew` fails if
Homebrew is missing. Overrides: `A3S_VERSION`, `A3S_INSTALL_DIR`,
`A3S_CHANNEL`, `A3S_YES`, `A3S_DRY_RUN`, `A3S_GITHUB_TOKEN`,
`A3S_MODIFY_PATH=1`.

Update the Homebrew channel with `brew upgrade a3s`. Update the binary channel
with `a3s self update` or by re-running the installer. Uninstall the binary
channel with:

```bash
rm -f ~/.local/bin/a3s ~/.local/bin/a3s-webview ~/.local/bin/a3s-code
rm -rf ~/.local/bin/moli
```

Windows x64 (PowerShell 5.1+) installs into `%LOCALAPPDATA%\Programs\a3s\bin`:

```powershell
irm https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.ps1 | iex
```

Set `$env:A3S_MODIFY_PATH = '1'` before running to update the user PATH.
Re-run the same command to update. Uninstall by closing `a3s`, removing that
directory, and deleting the PATH entry if you added one.

### Cargo

```bash
cargo install a3s --locked
```

Cargo installs the CLI from crates.io. Release companions (`a3s-webview`,
bundled `moli/`) are not guaranteed. Prefer Homebrew or the installer when you
need that layout.

### After install

```bash
a3s --version
which a3s
a3s code
```

Uninstalling the binary does not delete `~/.a3s/` (or the Windows equivalent).
Remove that directory only if you intend to wipe local config and caches.

`A3S_OFFLINE=1` and `A3S_NO_AUTO_INSTALL=1` stop runtime component installs and
network mutation. They do not turn an online installer into an offline one.

## Repository

```text
a3s/
├── apps/          Cloud, Desktop, docs, Windhole
├── packages/      Office, Science, UI
├── crates/        hosts, capabilities, runtimes, services
├── compat/        exact revisions and protocol locks
├── use-registry/  pinned signed Use Registry deployment
└── homebrew-tap/  release formulae
```

Clone the snapshot, not a partial tree:

```bash
git clone --recurse-submodules git@github.com:A3S-Lab/a3s.git
cd a3s
```

For an existing checkout: `git submodule update --init --recursive`.

> [!IMPORTANT]
> Do not add a root `Cargo.toml` or run `cargo init` here. Build and test
> inside the component that owns the change. Commit that component in its own
> repository before advancing its gitlink here.

The root `justfile` only orchestrates. Desktop JavaScript dependencies live in
`apps/desktop/package-lock.json`; run `cd apps/desktop && npm ci` once before
desktop recipes.

```bash
just desktop
just desktop-check
just code
just docs
just cloud-stack-check
```

<details>
<summary><strong>Component index</strong></summary>

| Group | Projects |
| --- | --- |
| Hosts | [CLI](crates/cli/), [Code](crates/code/), [Desktop](apps/desktop/), [Cloud](apps/cloud/), [Ash](crates/ash/), [Windhole](apps/windhole/) |
| Capabilities and content | [Use](crates/use/), [Browser](crates/browser/), [Search](crates/search/), [OCR](crates/ocr/), [Parser](crates/parser/), [Office](packages/office/), [Science](packages/science/) |
| Execution | [Runtime](crates/runtime/), [Sandbox](crates/sandbox/), [Box](crates/box/), [OCI Runtime](crates/oci-runtime/), [Power](crates/power/), [MoE](crates/moe/) |
| Coordination | [Flow](crates/flow/), [Event](crates/event/), [Lane](crates/lane/), [Memory](crates/memory/), [ORM](crates/orm/), [Gateway](crates/gateway/) |
| Interfaces | [ACL](crates/acl/), [Boot](crates/boot/), [TUI](crates/tui/), [GUI](crates/gui/), [WebView](crates/webview/), [UI](packages/ui/) |
| Verification and operations | [Bench](crates/bench/), [Test](crates/test/), [Observer](crates/observer/), [Sentry](crates/sentry/), [Updater](crates/updater/) |

</details>

The [project directory](https://a3s-lab.github.io/a3s/#ecosystem) lists role,
stage, and release channel for each project. [AGENTS.md](AGENTS.md) is the
contributor contract for this root.

## Further reading

- [Website](https://a3s-lab.github.io/a3s/)
- [Desktop download](https://a3s-lab.github.io/a3s/download/)
- [CLI reference](docs/cli-reference.md)
- [Desktop release](docs/desktop-release.md)
- [Cloud compatibility lock](compat/cloud-stack.acl)
- [Discord](https://discord.gg/XVg6Hu6H)

## License

This integration repository is [MIT](LICENSE). Pinned projects keep the license
of their owning repositories.
