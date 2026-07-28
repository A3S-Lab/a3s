# A3S Cross-Platform Install Product Design

- Status: Proposed, pre-1.0
- Date: 2026-07-15
- Parent: [A3S Component Management Design](component-management-design.md)
- Related: [Technical Architecture](cross-platform-install-architecture.md) and
  [A3S Use Extension Design](a3s-use-extension-design.md)

## 1. Decision

`a3s install` is the cross-platform lifecycle entry point for registered A3S
components. The current delivery contract covers macOS and Linux. Windows uses
the same product model but remains a roadmap target until its artifacts and
lifecycle conformance gates pass.

It is not a universal frontend for arbitrary operating-system packages. A3S
installs only a component that is present in the built-in catalog, a trusted
signed registry, or an explicit local package selected by the user.

The architecture makes three guarantees:

1. The user receives one consistent plan, approval, status, upgrade, and
   uninstall experience.
2. The native package manager remains the authority when Homebrew, Winget,
   APT, DNF, Pacman, or another supported manager performs the installation.
3. A3S reports backend capability differences instead of pretending that every
   ecosystem supports pinning, rollback, user scope, or offline installation.

## 2. Current Baseline and Target

The implemented baseline supports:

- a catalog compiled into `a3s`;
- bundled components;
- verified GitHub release archives for selected macOS and Linux targets;
- Homebrew when `a3s` is itself Homebrew-managed;
- delegated Browser and Office lifecycle through `a3s-use`;
- explicit local ACL extension directories.

The target design adds:

- portable managed artifacts for macOS, Linux, and Windows;
- target-aware source selection rather than archive-name conditionals;
- a common backend contract for native package managers;
- install planning, dry runs, source migration, and honest privilege reporting;
- signed registry metadata for independently published components;
- recovery journals and richer ownership receipts;
- explicit support levels for each OS, architecture, and backend capability.

This document defines the target contract. Public documentation must continue
to describe only behavior that has shipped.

### 2.1 Platform delivery policy

- macOS and Linux are the current runtime, release, and install-conformance
  priorities.
- Windows keeps compile-time CLI, MCP, Skill, manifest, and package-schema
  compatibility, but managed artifacts and runtime lifecycle are not currently
  advertised as supported.
- Windows moves to supported status only after install, upgrade, recovery,
  uninstall, file-lock, and real Browser persistent-session tests pass on
  release artifacts.
- WSL is evaluated as Linux, not as a native Windows backend.

## 3. Product Scope

### 3.1 Goals

| ID | Outcome |
| --- | --- |
| XP-1 | The same component ID and lifecycle commands work across every explicitly supported target. |
| XP-2 | `a3s install` selects a deterministic trusted source and explains why it was selected. |
| XP-3 | `--dry-run` exposes downloads, commands, privileges, scripts, ownership, and rollback limits before mutation. |
| XP-4 | Direct artifacts are digest-verified, staged, health-checked, and atomically activated. |
| XP-5 | Native package-manager installations remain native-manager-owned. |
| XP-6 | Existing provenance is retained unless the user explicitly requests migration. |
| XP-7 | Automation can select exact sources, versions, scope, offline behavior, and non-interactive policy. |
| XP-8 | Independently released components use ACL manifests and signed registry metadata without shipping arbitrary installer code. |
| XP-9 | Uninstall removes only receipt-proven A3S files or delegates to the recorded native package manager. |
| XP-10 | Unsupported platforms fail before download with supported alternatives and a manual path forward. |

### 3.2 Non-Goals

The component manager does not:

- accept arbitrary package names such as `a3s install apt:curl`;
- replace Homebrew, Winget, APT, DNF, Pacman, Snap, Flatpak, or language package
  managers;
- execute installer commands supplied as free-form strings by a registry;
- silently invoke `sudo`, bypass UAC, or change machine-wide policy;
- promise atomic rollback for an external package manager that cannot provide
  it;
- claim ownership of files installed by another package manager;
- silently change source, scope, architecture, or release channel;
- treat HTTPS alone as artifact integrity;
- run an unverified remote install script.

## 4. Product Model

The product vocabulary is:

- **component**: a stable A3S identity such as `use` or `use/browser`;
- **catalog**: trusted component identities and bootstrap policy shipped with
  `a3s`;
- **registry**: signed version and source metadata that can evolve independently
  of the CLI;
- **source**: one declared way to satisfy a component on a target;
- **backend**: trusted A3S code that understands a source kind;
- **target**: normalized OS, architecture, runtime, and distribution facts;
- **scope**: `user` or `system` ownership boundary;
- **plan**: immutable description of intended actions and risks;
- **receipt**: machine-owned evidence of the result and mutation authority.

A source is data. A backend is executable code. Registry packages may declare
sources supported by built-in backends, but they cannot supply a new backend
executable. New backend code ships through the normal trusted A3S release
process.

## 5. Public Experience

### 5.1 Commands

The existing lifecycle commands remain canonical:

```text
a3s list [--installed|--available|--updates] [--json]
a3s info <component> [--versions] [--sources] [--json]

a3s install <component>...
    [--version <requirement>]
    [--channel stable|beta|nightly]
    [--source auto|<source-id>]
    [--scope user|system]
    [--from <local-package>]
    [--dry-run]
    [--offline]
    [--migrate]
    [--force]
    [--allow-unsigned]
    [--yes]
    [--json]

a3s upgrade <component>... [--dry-run] [--offline] [--yes] [--json]
a3s upgrade --all [--dry-run] [--offline] [--yes] [--json]

a3s uninstall <component>...
    [--cascade]
    [--purge]
    [--dry-run]
    [--yes]
    [--json]

a3s doctor [component] [--json]
```

`a3s info` and `a3s doctor` are proposed additions. Existing flags retain their
current meaning. An omitted `--source` or `--source auto` invokes deterministic
selection. Any other value is a validated source ID rather than a closed CLI
enum, so new built-in backends do not require another command grammar.

### 5.2 Interaction Rules

- An explicit `a3s install` authorizes ordinary download and user-scoped
  installation from an already trusted source.
- `--dry-run` may refresh signed metadata unless `--offline` is also present,
  but it never installs or removes anything.
- `--json` is non-interactive and writes exactly one versioned result to
  stdout. Any required decision must be expressed by a flag or policy.
- `--yes` accepts the displayed plan; it does not grant unsigned trust,
  privilege elevation, or source migration.
- `--force` repairs using the existing provenance. It does not migrate sources.
- `--migrate` is required when an installed component changes backend, package
  identity, ownership model, or scope.
- Multiple requested components are planned together but committed as
  independent component transactions. A result reports partial success rather
  than claiming cross-manager atomicity.

### 5.3 Example Plan

```text
$ a3s install use --dry-run

Component:   use 0.2.0
Target:      windows-x86_64
Source:      winget / A3S-Lab.Use
Scope:       user
Trust:       first-party, signed registry
Actions:     refresh metadata, install package, probe a3s-use --version
Privileges:  none expected; installer may request UAC if Winget changes scope
Ownership:   Winget-managed; A3S will not delete package files directly
Rollback:    backend does not guarantee rollback
```

Human output explains the selected source. JSON returns every candidate and a
machine-readable rejection reason for candidates that were not selected.

### 5.4 Command Exposure

The umbrella CLI always executes a component through the absolute path in its
receipt, so lifecycle correctness never depends on `PATH`. A native package
manager may expose its normal command. A managed-artifact backend may create an
A3S-owned shim only in the documented user bin directory and must record it in
the receipt. Installation never edits shell profiles or the machine `PATH`
silently.

## 6. Status and Capability Model

The existing presence, health, update, and trust dimensions remain. The target
status adds:

```text
scope:       user | system | mixed | unknown
ownership:   a3s | package-manager | parent | external | none
sourceId:    managed-release | homebrew | winget | ...
sourceKind:  managed-artifact | native-manager | delegated | local-package
```

Each backend advertises capabilities instead of relying on its name:

```text
install
uninstall
update
repair
version-select
user-scope
system-scope
offline
rollback
dependency-resolution
```

An unsupported operation fails during planning. For example, a backend that
cannot select historical versions rejects `--version` before invoking its
package manager.

## 7. Target Model and Platform Coverage

Target detection produces normalized facts:

```rust
struct Target {
    os: OperatingSystem,
    arch: Architecture,
    libc: Option<Libc>,
    distribution: Option<Distribution>,
    os_version: Option<Version>,
    environment: ExecutionEnvironment,
    available_managers: Vec<ManagerProbe>,
}
```

Required distinctions include:

- macOS deployment version and native architecture;
- Linux `glibc` versus `musl`, distribution family, and distribution version;
- Windows build, native architecture, and whether execution is native or WSL;
- optional Rosetta or Windows x64 emulation only when the component manifest
  permits it. Cross-architecture fallback is never silent.

WSL is a Linux target. It does not use the Windows package-manager backend for
Linux component binaries.

### 7.1 Support Levels

| Level | Contract |
| --- | --- |
| Tier 1 | Built, released, and tested in A3S CI; supported for automatic selection. |
| Tier 2 | Built-in adapter with conformance tests; enabled only for components declaring a compatible package. |
| Tier 3 | Architecture extension point; not advertised until an actual component and CI fixture require it. |

Current delivery and roadmap are:

| Platform ecosystem | Delivery status | Ownership |
| --- | --- | --- |
| macOS/Linux portable archive or binary | Current Tier 1 priority | A3S |
| Homebrew on macOS/Linux | Current ownership-preserving backend | Homebrew |
| Windows portable archive or binary | Roadmap; not currently advertised | A3S |
| Winget on Windows | Roadmap after portable Windows conformance | Winget |
| APT, DNF, Pacman, Zypper on Linux | Demand-driven Tier 2 | Native manager |
| MSI/MSIX/PKG/DEB/RPM package formats | Demand-driven Tier 2 through a typed native adapter | Native installer or manager |
| Snap, Flatpak, Scoop, Chocolatey | Tier 3 until a registered component needs them | Native manager |
| npm, pipx, Cargo, and other language managers | Tier 3 and opt-in only | Language manager |

Package formats and package managers are not interchangeable. For example,
`.deb` is normally applied through APT so dependency resolution and ownership
remain with APT; it is not unpacked as an A3S-owned archive.

## 8. Catalog, ACL Manifests, and Registry

The CLI ships a small bootstrap catalog containing official component IDs,
registry endpoints, root trust keys, and safe first-use policy. Version and
platform metadata may come from a signed registry snapshot.

Publishers author component metadata in A3S ACL and it is parsed with
`a3s-acl`. A simplified manifest is:

```acl
component "use" {
  schema_version = 1
  publisher = "a3s-lab"
  description = "Browser, Office, and external application capabilities"

  release "0.2.0" {
    channel = "stable"

    source "managed-release" {
      kind = "managed-artifact"
      priority = 100

      target "windows-x86_64" {
        url = "https://github.com/A3S-Lab/Use/releases/download/v0.2.0/a3s-use-0.2.0-windows-x86_64.zip"
        format = "zip"
        sha256 = "<publisher-sha256>"
        executable = "a3s-use.exe"
      }
    }

    source "homebrew" {
      kind = "homebrew"
      package = "a3s-lab/tap/a3s-use"
      platforms = ["macos", "linux"]
      priority = 80
    }

    source "winget" {
      kind = "winget"
      package = "A3S-Lab.Use"
      platforms = ["windows"]
      priority = 80
    }
  }
}
```

The manifest may provide typed package identity and target metadata only. It
cannot provide shell fragments, arbitrary manager arguments, environment
assignments, or post-install commands.

Registry freshness and rollback protection use The Update Framework (TUF)
rather than a new signature protocol. TUF's standard machine metadata remains
TUF JSON; it is signed transport metadata, not A3S product configuration. ACL
remains the human-authored component and policy format.

The official registry has a built-in trust root. A third-party registry must be
added explicitly in ACL configuration and accepted with its root identity.
Registry trust establishes metadata provenance, not permission to execute an
untrusted component without policy checks.

Only the built-in root catalog may introduce or transfer a top-level component
ID. Third-party registry entries must remain inside a namespace owned by a
trusted parent, such as `use/acme/slack`; a registry cannot claim `use`,
`search`, or another reserved product ID.

## 9. Deterministic Source Selection

Resolution follows this order:

1. Parse the component ID and load the trusted catalog and pinned registry
   snapshot.
2. Detect the target and available trusted backends without mutation.
3. If a healthy compatible installation already satisfies the request, return
   an idempotent no-op and report its ownership.
4. If a receipt exists, retain its source, scope, channel, and package identity
   unless `--migrate` explicitly permits a change.
5. Apply explicit request constraints: version, channel, source, scope,
   offline mode, architecture, and trust policy.
6. Filter sources by target match, backend availability, backend capability,
   artifact integrity, and publisher policy.
7. Prefer the installation family that manages `a3s` itself when the component
   declares a compatible source. Otherwise follow manifest priority, then a
   verified user-scoped managed artifact.
8. Produce a plan or fail with every candidate's rejection reason.

The mere presence of a package-manager executable does not authorize its use.
The component must declare that source, the adapter must validate the manager,
and policy must permit the resulting scope and risk.

A discovered compatible system executable may satisfy ordinary resolution but
remains externally owned. An explicit `--source managed-release` selects an
A3S-owned copy. Changing an existing managed receipt also requires
`--migrate`.

The implementation architecture, backend contract, transaction model, and
delivery sequence are defined in the
[Cross-Platform Install Technical Architecture](cross-platform-install-architecture.md).
