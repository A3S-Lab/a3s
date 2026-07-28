# A3S Component Management Design

Status: Active implementation, pre-1.0

Parent: [A3S Use and Component Platform](a3s-use-component-platform.md)

Related: [Cross-Platform Install Product Design](cross-platform-install-product.md) and
[Technical Architecture](cross-platform-install-architecture.md). Public command naming follows
the [A3S CLI Product Design](cli-product-design.md).

## 1. Scope

This document defines component identity, discovery, ownership, command
behavior, installation transactions, receipts, and delegated capability
management for the umbrella `a3s` CLI.

It does not define Browser or Office actions. Those contracts live in the
[A3S Use Domain Design](a3s-use-domain-design.md).
Externally implemented Use domains are specified in the
[A3S Use Extension Design](a3s-use-extension-design.md).

## 2. Component Identity

Component IDs are stable lowercase path-like names. Each segment matches
`[a-z][a-z0-9-]*`; `/` denotes delegated ownership.

```text
code
box
bench
search
use
use/browser
use/office
use/acme/slack
```

The root catalog owns top-level IDs. The trusted parent owns the semantics and
lifecycle implementation of child IDs. Browser and Office are first-party
children. External Use extensions use `use/<publisher>/<name>` so package
identity is stable even when the shorter command route differs.

Component kinds are:

- `built-in`: functionality shipped in `a3s`;
- `product`: a separately distributed A3S executable;
- `capability`: a runtime delegated to a product component;
- `extension`: an externally implemented domain delegated to a trusted product
  component.

The Browser and Office domain code is part of the `use` product binary.
`use/browser` and `use/office` are virtual capability targets representing
runtime readiness and managed provider files, not separately installed domain
executables. Uninstalling one removes only its managed runtime; the built-in
command and doctor surface remain available. In contrast, `use/acme/slack`
owns an external extension package and executable.

## 3. Typed Trusted Catalog

The first catalog is compiled into the umbrella CLI:

```rust
struct ComponentSpec {
    id: ComponentId,
    kind: ComponentKind,
    command: Option<&'static str>,
    binary: Option<&'static str>,
    distribution: Distribution,
    auto_install_on_use: bool,
    removable: bool,
    static_children: &'static [ComponentId],
}

enum Distribution {
    Bundled,
    Release {
        homebrew_formula: Option<&'static str>,
        github: GitHubReleaseSpec,
    },
    Delegated {
        parent: ComponentId,
        capability: &'static str,
    },
}
```

Strings from CLI input are parsed into `ComponentId` and other enums at the
boundary. The first release does not load executable product definitions from
configuration or an unsigned remote catalog.

Only the top-level product catalog is compiled in. A trusted parent may return
dynamic child specifications through its delegated CLI contract. The root
validates the namespace and schema but does not parse or execute the child's
manifest.

## 4. Status and Provenance

Status uses independent dimensions:

```text
presence: bundled | managed | external | system | missing
health:   ready | broken | unknown
update:   current | available | unknown
trust:    first-party | verified-publisher | local-explicit | untrusted | n/a
```

Human labels such as `installed`, `ready (system)`, or `update available` are
derived. JSON returns all dimensions.

Provenance is one of:

```text
bundled
homebrew
github-release
external-path
system
delegated
local-package
```

Ownership follows provenance:

- bundled files belong to the parent installation;
- Homebrew files change only through Homebrew;
- GitHub release files change only through their A3S receipt;
- external and system files are never deleted by A3S;
- delegated files change only through the parent component CLI contract.

## 5. Resolution

Resolution for a registered executable component checks, in order:

1. a healthy path from an A3S receipt;
2. an existing known package-manager installation;
3. a compatible executable next to `a3s`;
4. a compatible executable on `PATH`;
5. catalog-supported first-use installation when allowed.

Every executable candidate must pass a bounded version/health probe. A found
PATH entry satisfies execution but remains externally owned.

Unknown top-level commands never automatically execute arbitrary
`a3s-<name>` binaries. Unregistered tools may appear under an external section
in `a3s list`.

## 6. Registered Product Proxy Contract

The root exposes explicit proxy namespaces for `box`, `bench`, `search`, and
`use`. For `a3s <registered-product> <args...>`, it:

1. resolves component `use`;
2. visibly bootstraps it if missing and allowed by that product's catalog
   policy;
3. refuses silent reinstall when the component is broken;
4. invokes the absolute resolved `a3s-use` path without a shell;
5. forwards domain arguments without parsing them;
6. preserves the child exit code and signal outcome as closely as the platform
   permits.

Arguments after the registered namespace remain child-owned. Box and Use may
opt into first-use installation; Bench and Search initially require explicit
installation. Unknown top-level words never enter this proxy path.

## 7. `a3s list`

```text
a3s list
a3s list --installed
a3s list --available
a3s list --updates
a3s list --json
```

Default listing combines:

- catalog entries;
- bundled versions;
- A3S receipts;
- package-manager state;
- registered executable discovery;
- delegated child status when its parent is compatible;
- unregistered `a3s-*` executables in a separate external section.

It performs no network request unless `--updates` is present.

Example:

```text
COMPONENT     TYPE         STATUS       VERSION   SOURCE
code          built-in     ready        5.2.3     bundled
box           product      installed    3.0.5     homebrew
bench         product      missing      -         -
search        product      ready        1.4.1     external-path
use           product      installed    0.1.0     github-release
use/browser   capability   ready        138.x     system
use/office    capability   missing      -         -
use/acme/slack extension   installed    1.2.0     local-package

EXTERNAL TOOLS
foo           a3s-foo 1.4.1             /usr/local/bin/a3s-foo
```

## 8. `a3s install`

```text
a3s install <component>...
    [--version <semver>]
    [--source auto|homebrew|release]
    [--from <package>]
    [--force]
    [--allow-unsigned]
    [--yes]
    [--json]
```

Rules:

- No component argument prints available components and makes no changes.
- An already healthy matching version succeeds without mutation.
- `--force` repairs or reinstalls using existing provenance unless `--source`
  explicitly changes it.
- `a3s install code` verifies or repairs the bundled installation.
- A delegated child first ensures its parent, then uses the parent CLI
  contract.
- `--from` is valid for a delegated extension package and is passed to the
  trusted parent for manifest and native surface validation.
- Unsigned local development packages require `--allow-unsigned`; arbitrary
  remote packages are not activated implicitly.
- Explicit install authorizes the download but still reports source, version,
  and expected changes.
- Multiple components are independent operations, not one transaction.

## 9. `a3s uninstall`

```text
a3s uninstall <component>...
    [--cascade]
    [--purge]
    [--yes]
    [--json]
```

Rules:

- Stop owned background services before deleting executables.
- Remove only package-manager or receipt-owned files.
- Preserve configuration, sessions, profiles, documents, and artifacts.
- A parent with managed children requires `--cascade`.
- `--purge` removes component caches and owned runtime state after confirmation,
  but never user documents or output artifacts.
- `code` returns `component.not_removable` because it is bundled.
- External and system entries return `component.not_owned` and identify their
  path or package manager when known.
- Removing an extension also releases its route registration before deleting
  receipt-owned files.

## 10. Upgrade and Self-Update

- `a3s self update` updates the umbrella CLI without overloading a component
  operation.
- No-argument `a3s upgrade` lists available component upgrades without
  mutation.
- `a3s upgrade <component>...` upgrades selected managed components.
- `a3s upgrade --all` upgrades all eligible managed product components.
- The old `a3s update` forms remain temporary compatibility aliases according
  to the CLI deprecation policy.
- Missing optional components produce an install suggestion; upgrade does not
  silently become install.
- System-provided capability runtimes are not replaced by `--all`.
- A local extension package without a recorded update source requires an
  explicit new `--from` package; it is not queried on the network.

## 11. First-Use Policy

Only trusted registered product components can opt into first-use install.
The action is announced before download.

```text
A3S_NO_AUTO_INSTALL=1
```

disables first-use mutation while leaving explicit install available.

Capability runtimes use stricter policy:

- validated system runtimes may satisfy the capability;
- interactive use may request confirmation for managed downloads;
- non-interactive use requires prior installation unless explicitly enabled;
- third-party source, version, and license are shown before installation.

Unknown external Use routes never trigger first-use download or PATH lookup.
The user must explicitly install a package or enable an already registered
extension.

## 12. Delegated Component CLI Contract

The root CLI never scrapes child human output. A compatible product exposes
ordinary subcommands with versioned `--json` output. Use provides:

```text
a3s-use component list --json
a3s-use component status browser --json
a3s-use component install browser --json
a3s-use component uninstall office --json
a3s-use component install acme/slack --from <package> --json
a3s-use component uninstall acme/slack --json
a3s-use mcp start|status|stop browser --json
```

Every response includes `schemaVersion`, component ID, structured status, and
the common error envelope. Unsupported versions return
`component.cli_contract_incompatible` with an update suggestion.

This is a process contract: argv in, one JSON document out, diagnostics on
stderr, and an exit status. It is not JSON-RPC and it has no method envelope or
long-running connection. It manages built-in runtimes and dynamic extension
children. An extension then uses its declared CLI, MCP, and Skill contracts;
Browser and Office sessions remain separate domain contracts.

## 13. Installer Ownership

The CLI owns catalog and user policy. `a3s-updater` is extended with reusable
mechanics:

- platform and release resolution;
- Homebrew command adapter;
- direct archive download;
- mandatory SHA-256 verification for direct releases;
- safe extraction;
- staging and health probes;
- atomic activation and rollback;
- receipt reads and writes;
- owned-file uninstall;
- per-component locks.

`a3s-updater` does not own the product catalog.

The current Homebrew and direct-release implementations are the initial
backends. Target detection, source planning, native package-manager adapters,
signed registry metadata, and the macOS/Linux/Windows support contract are
defined in the
[Cross-Platform Install Product Design](cross-platform-install-product.md) and
[Technical Architecture](cross-platform-install-architecture.md).

## 14. Source Selection

Selection is deterministic:

1. retain existing managed provenance;
2. honor an explicit supported `--source`;
3. use Homebrew when `a3s` is Homebrew-managed and a formula exists;
4. otherwise use a verified GitHub release.

The mere presence of `brew` on `PATH` does not change a direct installation
into a Homebrew installation.

## 15. Direct Install Transaction

```text
resolve specification and target version
  → acquire component lock
  → download into cache
  → verify checksum before extraction
  → extract into a new staging directory
  → reject absolute paths, parent traversal, and escaping links
  → run bounded health and version probes
  → atomically switch the active version
  → atomically write the receipt
  → remove the previous version after successful activation
```

An interruption leaves either the previous healthy version or a removable
staging directory. It never partially overwrites the active binary.

## 16. Filesystem and Receipts

Logical XDG-style roots are:

```text
$XDG_DATA_HOME/a3s/components/<component>/<version>/
$XDG_STATE_HOME/a3s/components/<component>.json
$XDG_CACHE_HOME/a3s/downloads/
$XDG_RUNTIME_DIR/a3s/locks/
$XDG_RUNTIME_DIR/a3s/use/
```

Path helpers provide platform equivalents. The CLI executes the absolute path
from the receipt, so proxying does not depend on `PATH`. A user-visible shim may
be installed separately.

A receipt records at least:

- schema version;
- component ID and version;
- provenance and release identity;
- install and active roots;
- owned files or owned root set;
- artifact checksums;
- install timestamp;
- health-probe identity.

Receipts are versioned JSON machine state. Product configuration remains in
A3S ACL; receipts are not user configuration.

Serialization boundaries are explicit:

- human-authored A3S configuration and extension manifests use ACL;
- machine-owned installation receipts use versioned JSON;
- root-to-child component control uses versioned CLI JSON output, not JSON-RPC;
- external execution retains native CLI, MCP, and Skill contracts.

## 17. Uninstall Transaction

```text
resolve component and ownership
  → acquire component lock
  → stop owned background service or fail safely
  → validate child/cascade policy
  → deactivate user-visible shim
  → remove only owned paths
  → remove empty owned directories
  → remove receipt
```

Homebrew removal delegates to Homebrew and verifies the result. External or
system installations receive no filesystem mutation.

## 18. Output Contract

Human mode writes results to stdout and progress or diagnostics to stderr. JSON
mode writes exactly one versioned document to stdout.

```json
{
  "schemaVersion": 1,
  "ok": false,
  "error": {
    "code": "component.not_installed",
    "message": "Component 'use' is not installed.",
    "suggestion": "Run 'a3s install use'.",
    "details": {}
  }
}
```

Component-management exit codes are:

| Code | Meaning |
| --- | --- |
| `0` | Success |
| `1` | Operation failure |
| `2` | Invalid usage, preserving the current CLI convention |
| `3` | Partial failure in a multi-component operation |

Proxy commands preserve child results instead of remapping them.

## 19. Security Rules

- Only registered commands may trigger first-use installation.
- Direct checksums are mandatory and verified before extraction.
- Extraction rejects traversal, absolute targets, and escaping links.
- Install never executes a downloaded shell script.
- A3S never silently invokes `sudo` or writes to a system prefix.
- Receipts may own paths only under approved roots.
- External and system installations are read-only to the manager.
- Dynamic delegated IDs are accepted only under a trusted parent's namespace.
- External extension activation requires manifest validation, successful
  validation of every declared native surface, and an explicit trust decision.
- Uninstall never treats profiles, documents, downloads, or artifacts as
  component files.
- Background-service shutdown is bounded; failure prevents unsafe executable
  removal.

## 20. Verification

Focused tests cover:

- component ID and catalog validation;
- local list with no network request;
- bundled, managed, Homebrew, external, system, missing, and broken discovery;
- provenance retention and explicit source changes;
- checksum and unsafe archive rejection;
- atomic activation and injected rollback failures;
- receipt ownership and uninstall safety;
- concurrent operation locks;
- parent/child cascade behavior;
- delegated CLI contract compatibility;
- dynamic extension namespace, route-conflict, trust, and uninstall behavior;
- JSON golden output;
- proxy arguments, status, and first-use behavior;
- migration of Box bootstrap tests to the common manager.
