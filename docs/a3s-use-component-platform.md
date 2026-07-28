# A3S Use and Component Platform

Status: Active implementation, pre-1.0

Date: 2026-07-14

Owners: A3S CLI, A3S Use, A3S Search, and A3S Updater

## 1. Decision

A3S will add `a3s-use` as its typed application-capability layer. Browser and
Office are first-party domains enabled in the default distribution. Additional
domains can be implemented and released independently as external Use
extensions. The primary user entry point is `a3s use`; the standalone
`a3s-use` binary remains available for direct use and debugging.

The umbrella CLI will own a trusted component catalog and these public
lifecycle commands:

```text
a3s list
a3s info <component>
a3s install <component>
a3s uninstall <component>
a3s upgrade [<component>...]
a3s doctor [component]
a3s self update
a3s use <args...>
```

The complete umbrella command taxonomy and compatibility policy are defined in
the [A3S CLI Product Design](cli-product-design.md).

The component hierarchy is:

```text
A3S product components
├── code                         bundled with a3s
├── box                          separately distributed
├── bench                        separately distributed
├── search                       separately distributed
└── use                          separately distributed as a3s-use
    ├── use/browser              built-in domain and delegated runtime
    ├── use/office               built-in domain and delegated runtime
    └── use/<publisher>/<name>   externally implemented domain extension
```

`a3s-search` will depend directly on the `a3s-use-browser` Rust crate. It will
not execute `a3s use`, depend on the `a3s-use` facade, or require an
`a3s-use` process or MCP service.

Detailed contracts live in:

- [Component Management Design](component-management-design.md)
- [Cross-Platform Install Product Design](cross-platform-install-product.md)
- [Cross-Platform Install Technical Architecture](cross-platform-install-architecture.md)
- [A3S Use Domain Design](a3s-use-domain-design.md)
- [A3S Use Extension Design](a3s-use-extension-design.md)

## 2. Why This Architecture

Before the component foundation, A3S had three overlapping distribution
patterns:

- `a3s list` discovers executable names by scanning `PATH`.
- `a3s box` contains a Box-specific first-use installer.
- `a3s update` and `a3s-updater` focus on replacing one current binary.

The component foundation now consolidates those paths behind the typed catalog,
receipts, and lifecycle commands. Use remains a registered component and
delegated parent rather than gaining a second installer, so Box and Use keep one
ownership record and one uninstall authority.

Browser and Office also require different runtime semantics:

- Browser rendering must be embeddable in Search without a CLI or MCP service.
- Interactive Browser commands benefit from persistent sessions.
- OfficeCLI exposes native CLI and standard MCP surfaces and has mutation
  operations whose outcomes can become unknown if a process result is lost.
  Its resident transport is an OfficeCLI implementation detail, not an A3S
  integration contract.

One component platform plus typed domain libraries addresses both concerns
without making Use a package manager or making Search depend on a CLI.

Browser and Office are not intended to be the permanent closed set. Extension
packages can expose native CLI, MCP, and Skill surfaces to add routes such as
`a3s use slack ...` without linking against Rust's unstable dynamic library ABI
or modifying the `a3s-use` binary.

## 3. Goals

### 3.1 Product Goals

| ID | Goal |
| --- | --- |
| CP-1 | `a3s use <args...>` resolves and runs the registered Use component, preserving arguments and child status. |
| CP-2 | `a3s list` reports registered components, delegated capabilities, and unregistered local `a3s-*` tools separately. |
| CP-3 | `a3s install`, `a3s upgrade`, and `a3s uninstall` provide idempotent, ownership-safe component lifecycle management. |
| CP-4 | Homebrew installations stay Homebrew-managed; direct installations use verified releases and receipts. |
| CP-5 | Product components may opt into visible first-use installation, which automation can disable. |
| CP-6 | Registered components may declare typed sources for supported targets while native package managers retain ownership; current delivery prioritizes macOS and Linux. |
| USE-1 | Use exposes separate typed Browser and Office APIs, not a generic JSON action API. |
| USE-2 | Browser supports an embedded renderer and optional persistent CLI/MCP sessions. |
| USE-3 | Office uses a pinned OfficeCLI provider through native commands and launches OfficeCLI's standard MCP server directly; A3S does not implement its resident transport. |
| USE-4 | Browser and Office runtimes appear as delegated child components. |
| EXT-1 | Browser and Office are reserved built-in routes in the default `a3s-use` distribution. |
| EXT-2 | An explicitly installed external extension can add a new `a3s use <route>` domain without rebuilding Use. |
| EXT-3 | External extensions reuse native CLI, MCP, and Skill contracts and cannot shadow built-in routes. |
| EXT-4 | Dynamic extension components are reported and managed through the trusted Use parent namespace. |
| SEARCH-1 | Search injects `Arc<dyn PageRenderer>` and retains search query, parsing, fallback, deduplication, and ranking logic. |

### 3.2 Quality Goals

| ID | Goal |
| --- | --- |
| Q-1 | `a3s list` and local status checks require no network access unless updates are requested. |
| Q-2 | Install, upgrade, and uninstall are serialized per component and recover from interruption. |
| Q-3 | Public Rust services and provider traits are `Send + Sync` where applicable and use async I/O. |
| Q-4 | A library downloads software only when its caller explicitly selects a managed provider. |
| Q-5 | Human output is actionable; JSON output has a stable, versioned schema. |
| Q-6 | Install never pipes a remote script into a shell or silently escalates privileges. |
| Q-7 | Uninstall never deletes externally owned binaries, system browsers, documents, profiles, or artifacts. |
| Q-8 | Supported release targets are explicit; unsupported targets fail with a manual path forward. |
| Q-9 | External extensions are never auto-executed from `PATH`; activation requires an explicit manifest, native surface validation, and trust decision. |
| Q-10 | Cross-platform source selection is deterministic, privilege-transparent, and capability-aware. |

## 4. Non-Goals

The first version will not:

- treat arbitrary third-party executables as Use extensions without explicit
  installation, registration, and surface validation;
- load executable component definitions from an unsigned remote catalog;
- turn A3S Use into a workflow engine or agent planner;
- move search-provider logic into A3S Use;
- expose `execute(domain, action, serde_json::Value)` as the Rust SDK;
- reimplement Office document formats;
- require embedded Rust callers to use a background service;
- support multiple active versions of one product component;
- remove user data during normal uninstall;
- silently migrate between Homebrew and direct-release provenance.

## 4.1 Platform scope

macOS and Linux are the current supported runtime and managed-component
targets. Windows remains on the roadmap. Its CLI/MCP/Skill and manifest types
stay build-compatible, but native managed installation, file-lock lifecycle,
and real Browser persistent sessions are not completion criteria for the
current release. Windows support is promoted only after those release and
runtime gates pass; WSL follows the Linux contract.

## 5. System Architecture

```text
                         user / script / agent
                                  │
                                  ▼
                         a3s umbrella CLI
                    ┌─────────────┴─────────────┐
                    │ component manager         │ command proxy
                    ▼                           ▼
              a3s-updater engine             a3s-use host
           release / brew / receipts      ┌─────┼──────────┐
                                      browser  office  domain registry
                                         │       │           │
                                         ▼       ▼           ▼
                               Chrome / Lightpanda OfficeCLI external surfaces

              a3s-search ── Arc<dyn PageRenderer> ──► a3s-use-browser
```

| Owner | Owns | Does not own |
| --- | --- | --- |
| `a3s` CLI | Public commands, trusted catalog, first-use policy, proxy execution, human/JSON UX | Browser actions, Office operations, archive mechanics |
| `a3s-updater` | Release resolution, download, verification, staging, receipts, rollback, uninstall primitives | Product catalog, domain behavior |
| `a3s-use-core` | Shared IDs, artifacts, diagnostics, errors, risk classes, policy hooks | Browser or Office command models |
| `a3s-use-browser` | Providers, lifecycle, rendering, sessions, snapshots, interactions | Search queries, result parsing, ranking |
| `a3s-use-office` | Typed Office operations and OfficeCLI provider | Office file-format implementation |
| `a3s-use-extension` | ACL manifest, registry, and CLI/MCP/Skill adapters | Browser, Office, or extension-specific business logic |
| `a3s-search` | Search providers, parsing, ranking, fallback | Browser installation, CDP, process pools |
| `a3s-flow` | Cross-domain durable workflows | Low-level Browser or Office actions |
| `a3s-code` | Agent tools, approvals, task-level policy | Product component installation |

### 5.1 Code Use Worker

Every TUI or Web Code session attached to an already-installed Use component
registers one stable worker named `use`. The parent model discovers that worker
through the live `task` and `parallel_task` definitions; the catalog includes
the current capability IDs and is rebuilt after capability snapshot changes.
Hidden implementation helpers never enter that catalog.

The worker is a capability boundary, not another package manager:

- it can see and call only `mcp__use_*` tools;
- it cannot use workspace, shell, unrelated MCP, or recursive task tools;
- packaged `SKILL.md` text supplies domain guidance but cannot expand those
  permissions or authorize installation; Code verifies its registry-projected
  SHA-256 before loading or replacing the live Skill;
- ordinary built-in Browser, native Office, and OCR tools are allowed only
  inside this worker, while bounded provider installers and newly projected
  extension tools remain `Ask` and surface through the parent confirmation
  stream;
- it returns the capability route, observable result, session/object
  references, and typed failures to the parent;
- it never retries application mutations automatically, and
  `use.office.outcome_unknown` is reported as potentially applied.

Live MCP additions refresh delegation before the next child run. A run already
in progress keeps its accepted execution boundary and settles through normal
MCP and session cancellation. A newly discovered route is reported as callable
only after its MCP projection connects; a removed or replaced route is withdrawn
from the worker catalog before its prior connection drains. Starting Code does
not install Use; installation remains an explicit umbrella component action.

TUI and Web derive presentation from the existing standard subagent progress
stream. The TUI renders ordered capability identities such as `Using Browser`
and `Used Browser`; Web renders `Use · Browser` and readable action evidence.
Only an exact worker identity of `use` plus an observed `mcp__use_<route>__*`
tool can produce those labels, and restored task snapshots replay the same
projection. Raw Use definitions stay hidden from the parent model. Delegated
confirmation-required, confirmation-received, and confirmation-timeout events
are forwarded to the parent runtime so an `Ask` decision cannot deadlock inside
the child. This presentation adds no transport or permission surface.

## 6. Core Architectural Decisions

### 6.1 Library First

Browser and Office expose typed libraries. Their public execution surfaces stay
explicit: CLI uses the native process contract, MCP uses standard MCP, and
Skill uses `SKILL.md`. Browser owns its standard MCP adapter; Office CLI and MCP
delegate to OfficeCLI's native surfaces; external packages own their declared
surfaces. Component lifecycle delegation is an ordinary versioned CLI `--json`
contract, not JSON-RPC. Search calls the Browser library directly.

### 6.2 Two Component Levels

The root CLI manages A3S product distributions such as `use`. A product may
manage delegated capability runtimes such as `use/browser`. The root delegates
through versioned child CLI commands instead of learning Chrome or OfficeCLI
internals.

### 6.3 Trusted Catalog and Explicit Ownership

The initial product catalog is typed policy compiled into `a3s`. Local
discovery can satisfy a registered component, but discovery alone never grants
delete ownership. Direct-install receipts and package-manager provenance are
the authority for mutation.

### 6.4 Structured Status

Component status is not one overloaded string:

```text
presence: bundled | managed | external | system | missing
health:   ready | broken | unknown
update:   current | available | unknown
trust:    first-party | verified-publisher | local-explicit | untrusted | n/a
```

Human labels are derived from these fields. JSON clients receive the structured
state.

### 6.5 Embedded and Persistent Runtime Forms

Use provides:

- an embedded runtime for Search and Rust applications;
- explicit standard MCP entry points for Browser, Office, and external package
  targets;
- an authenticated loopback standard MCP Streamable HTTP deployment for
  Browser state shared across short-lived CLI invocations.

Each target keeps its own MCP vocabulary and lifecycle; Use does not aggregate
or translate servers. The Browser deployment uses the Browser server's normal
MCP tools, a private bearer receipt, and bounded lifecycle; it is not a second
RPC protocol. MCP is never a prerequisite for `PageRenderer`, stateless Browser
rendering, or native Office and extension CLI commands.

### 6.6 Built-In Domains and External Extensions

Browser and Office register directly in the Use domain registry and are always
present in the default binary, although their runtime dependencies may be
missing. External domains register native surfaces from explicitly installed
manifests. CLI and MCP retain their existing process contracts; Skill uses the
existing Skill loader. The shared registry provides discovery and dispatch; it
does not replace the typed Browser and Office Rust APIs with a generic SDK.

Built-in routes and host-management routes are reserved. External route
conflicts fail activation instead of using PATH or installation order as an
implicit precedence rule.

## 7. Public Command Surface

| Command | Contract |
| --- | --- |
| `a3s use <args...>` | Resolve or visibly bootstrap `a3s-use`, then forward all domain arguments. |
| `a3s list [--updates] [--json]` | Report catalog, ownership, health, version, source, capabilities, and external tools. No network by default. |
| `a3s info <component>` | Explain versions, compatible sources, provenance, ownership, and backend capabilities. |
| `a3s install <component>...` | Install or repair registered components. No arguments lists available components. |
| `a3s uninstall <component>...` | Remove only owned files. Children require `--cascade`; user data requires explicit `--purge`. |
| `a3s self update` | Update the umbrella CLI while preserving installation provenance. |
| `a3s upgrade [<component>...]` | List or upgrade managed components without turning missing components into installs. |
| `a3s doctor [component]` | Run read-only health and ownership diagnostics. |

Canonical Use examples are:

```text
a3s install use
a3s install use/browser
a3s install use/office
a3s install use/acme/slack --from ./slack-extension

a3s use browser render https://example.com
a3s use browser snapshot --session research --json
a3s use office validate report.docx
a3s use slack channels list

a3s uninstall use/office
a3s uninstall use/acme/slack
a3s uninstall use --cascade
```

Before terminal takeover, Code TUI may install verified Use and WebView product
releases when networking and first-use setup are allowed. Code Web consumes an
already-ready Use installation without mutating product lifecycle.
`A3S_NO_AUTO_INSTALL=1` and offline mode disable first-use product installation
for CI and hermetic environments. Third-party capability runtimes require an
explicit install or interactive confirmation; non-interactive library calls do
not download them implicitly.

## 8. A3S Use Repository

The new repository follows the monorepo submodule rules:

- GitHub: `A3S-Lab/Use`
- Submodule: `crates/use`
- Main Cargo package and binary: `a3s-use`
- Rust import: `a3s_use`

```text
Use/
├── Cargo.toml                   package plus workspace
├── README.md
├── LICENSE
├── src/
│   ├── lib.rs                   facade
│   └── main.rs                  a3s-use CLI
├── crates/
│   ├── core/                    a3s-use-core
│   ├── browser/                 a3s-use-browser
│   ├── office/                  a3s-use-office
│   └── extension/               a3s-use-extension
├── docs/rfcs/
└── tests/
```

The extension crate is required from the first stable contract because
out-of-tree implementations are an explicit product requirement. A separate
service crate is extracted only if the standard MCP server later requires
independent versioning.

## 9. Search Boundary

After migration:

```text
a3s-search owns
  query construction
  search provider selection
  result parsing
  deduplication and ranking
  fallback strategy

a3s-use-browser owns
  browser/runtime discovery
  install/status/doctor
  process lifecycle and pool
  page guards and cleanup
  navigation waits
  rendered page output
```

`BrowserFetcher` accepts `Arc<dyn PageRenderer>`. Search removes direct
browser-control and installation dependencies. A released
`a3s-use-browser` version is used instead of a committed sibling-only path
dependency. Existing public management types receive one compatible minor
release of deprecated re-exports.

## 10. Delivery Plan

### Phase 0: Approve Contracts

- Review these three design documents.
- Confirm IDs, command behavior, provenance, receipts, provider boundaries, and
  JSON schemas.
- Create `A3S-Lab/Use` only after the boundaries are accepted.

### Phase 1: Component Foundation

- Extend `a3s-updater` with generic transaction and receipt primitives.
- Add the trusted catalog and `list/install/upgrade/uninstall` commands.
- Migrate Box bootstrap to the component engine without changing `a3s box` UX.

### Phase 2: Use Browser MVP

- Create `a3s-use-core`, `a3s-use-browser`, and the domain registry.
- Extract proven Chrome and Lightpanda lifecycle and rendering behavior from
  Search.
- Publish crates, binaries, checksums, notices, and Homebrew formula.

### Phase 3: Root Integration and Search Migration

- Register `use`, `use/browser`, and `use/office`.
- Implement `a3s use` and first-use product bootstrap.
- Migrate Search to the released Browser crate with compatibility tests.

### Phase 4: Stateful Browser

- Add the Browser-owned standard MCP server, isolated sessions, semantic
  snapshots, stable references, interactions, and artifact handles.
- Stabilize the external extension manifest and CLI/MCP/Skill adapters, then
  validate them with out-of-tree conformance packages.
- Deploy the same Browser server over authenticated loopback Streamable HTTP
  after CLI and stdio MCP compatibility tests pass; verify state across
  separate CLI processes and ownership-safe shutdown.

### Phase 5: Office

- Add pinned OfficeCLI installation and a typed native-command provider.
- Preserve native CLI behavior and launch OfficeCLI's standard MCP server
  directly; do not integrate with its private resident pipe.
- Implement typed common commands, validation, render, batch, and safe close.

### Phase 6: Stabilization

- Add A3S Code tools and approval UX.
- Complete shell completion, recovery, Homebrew, migration, and public docs.
- Stabilize component CLI/receipt schemas and standard MCP compatibility before
  1.0.

## 11. Acceptance Criteria

The first stable milestone requires all of the following:

- `a3s use` resolves or installs Use through the common component manager and
  forwards command results correctly;
- `a3s list` reports registered, delegated, and external state without network
  access by default;
- failed install or upgrade leaves the prior healthy version active;
- uninstall proves ownership and preserves user data;
- Box uses the common engine instead of its private download pipeline;
- Search renders through `a3s-use-browser` without requiring `a3s`,
  `a3s-use`, or an MCP service;
- Browser CLI sessions persist across invocations when requested;
- TUI and Web sessions discover one live `use` worker whose MCP and Skill
  surfaces converge after install, upgrade, disable, re-enable, and session
  replacement without exposing shell or workspace fallbacks;
- a Skill content change updates the capability revision and is verified by
  digest before Code replaces the live Skill;
- an explicitly installed out-of-tree extension adds a new route without
  rebuilding Use and cannot shadow Browser or Office;
- Office mutations never retry after an ambiguous write;
- human and JSON contracts have integration tests;
- tests leave no processes, sockets, handles, or temporary files behind.

## 12. Documentation Plan

These design documents are authoritative while implementation is incomplete.
Public user docs must describe only released behavior.

Documentation lands with implementation:

1. The component foundation updates the root README and replaces PATH-only
   language in the CLI command and Box/tool docs.
2. Browser MVP adds the Use README, architecture, Browser reference, and Search
   migration guide.
3. Root integration documents `a3s use`, component JSON, first-use control,
   install, and uninstall.
4. External extension hosting adds a manifest/surface reference, packaging
   guide, and out-of-tree conformance example.
5. Stateful Browser and Office references land only with their releases.
6. English and Chinese public command tables change together.

Existing README examples that mention unavailable component commands must be
marked as planned or updated with the CLI release that implements them.

## 13. Deferred Decisions

The following require separate evidence and proposals:

- third-party registry governance and publisher admission policy;
- multiple active product versions;
- a component-manager crate separate from `a3s-updater`;
- an external `AgentBrowserProvider`;
- non-OfficeCLI Office providers;
- a public background-service API beyond the managed Browser standard-MCP
  deployment;
- automatic package-manager provenance migration.
