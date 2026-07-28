# A3S Use Domain Design

Status: Implemented, pre-1.0 stabilization
Parent: [A3S Use and Component Platform](a3s-use-component-platform.md)

## 1. Scope

A3S Use is the typed application-capability layer for operating applications.
Browser and Office are enabled as first-party built-in domains in the default
distribution. Additional domains can be installed as external extensions. Use
provides libraries, a standalone CLI, an extension host, and explicit standard
MCP entry points. These surfaces coexist; none is a transport wrapper around
another.

It does not manage A3S product distributions. The umbrella CLI owns product
installation through the
[Component Management Design](component-management-design.md). Use manages only
its delegated runtime capabilities.

External domain contracts are defined separately in the
[A3S Use Extension Design](a3s-use-extension-design.md).

### Reference Inputs

| Project | What A3S Use adopts | Boundary |
| --- | --- | --- |
| [agent-browser](https://github.com/vercel-labs/agent-browser) | Client/service separation, sessions, semantic snapshots, element references, profiles, doctor, and Agent-oriented commands | Design reference or optional future provider, not the core Rust library dependency or protocol |
| [OfficeCLI](https://github.com/iOfficeAI/OfficeCLI) | Native CLI vocabulary, standard MCP server, batch operations, and explicit document lifecycle | Version-pinned external provider with A3S-owned installation and mutation-outcome policy; its resident transport remains private to OfficeCLI |
| [ai-cli](https://github.com/vercel-labs/ai-cli) | Domain subcommands, stdin pipelines, predictable artifacts, JSON-only stdout, and progress on stderr | UX reference, not a universal untyped action API |

## 2. Repository

The project is a separate repository and monorepo submodule:

- GitHub: `A3S-Lab/Use`
- Submodule: `crates/use`
- Main package and binary: `a3s-use`
- Rust import: `a3s_use`

```text
Use/
├── Cargo.toml                   package plus workspace
├── README.md
├── LICENSE
├── src/
│   ├── lib.rs                   facade
│   └── main.rs                  CLI and standard MCP server adapter
├── crates/
│   ├── core/                    a3s-use-core
│   ├── browser/                 a3s-use-browser
│   ├── office/                  a3s-use-office
│   └── extension/               a3s-use-extension
├── docs/rfcs/
└── tests/
```

The extension crate is required because out-of-tree domains are a product
requirement. It contains ACL manifest types, the domain registry, and native
CLI/MCP/Skill descriptors. It reuses `a3s-acl`, standard MCP, and the existing
Skill loader instead of defining an extension RPC. A separate service crate is
deferred until an A3S-owned standard MCP server needs independent versioning.

## 3. Dependency Graph

```text
a3s-use ─────────────┬──► a3s-use-browser   ──► a3s-use-core
                     ├──► a3s-use-office    ──► a3s-use-core
                     └──► a3s-use-extension ──► a3s-use-core

a3s-search ─────────────► a3s-use-browser

external domain ────────► a3s-use-extension   optional Rust helper
```

The facade can expose optional Browser and Office features. Downstream crates
depend on the narrow domain crate they need.

The published `a3s-use` binary enables Browser, Office, and extension hosting
by default. Cargo features may support smaller custom library builds, but the
official binary cannot omit the two built-in command domains.

## 4. Shared Core

`a3s-use-core` contains only concepts with equivalent cross-domain semantics:

- `UseSessionId`;
- capability discovery and diagnostics;
- artifact path, media type, size, and content hash;
- stable structured errors with code, suggestion, and details;
- request IDs and operation timing;
- action risk classification;
- policy hooks.

It does not contain a universal action payload or domain command enum.

## 5. Provider Model

Public SDK callers use typed providers:

```rust
let browser = BrowserRuntime::builder()
    .provider(ChromeProvider::managed(options))
    .policy(policy)
    .build()
    .await?;
```

Normal scalar options such as timeouts and headless mode remain scalars.
Backend and extension choices are provider objects, not raw string fields.

A provider may be:

- system-backed, using an already installed compatible runtime;
- managed, explicitly permitted to install a pinned runtime;
- externally supplied by an application.

Constructing or calling a library does not implicitly download software. The
caller must select a managed provider to authorize that behavior.

### Built-In Registry and External Boundary

Browser and Office register in the host's domain registry without a subprocess
hop. Their routes are reserved and available in the default binary even when a
runtime dependency is missing. Missing runtime state is reported by doctor; it
does not remove the command surface.

Accordingly, delegated IDs `use/browser` and `use/office` describe operational
runtime readiness. Installing or uninstalling those IDs prepares or removes a
managed provider runtime without adding or removing the built-in domain code.

External domains register one or more CLI, MCP, and Skill surfaces through
validated manifests. Each uses its native contract. The host-facing registry
shares discovery, policy, availability, and ownership semantics, while Browser
and Office retain their richer typed Rust APIs. There is no universal extension
invocation envelope.

## 6. Browser Contract

### 6.1 Renderer

The narrow Search-facing API is object-safe and browser-specific:

```rust
#[async_trait]
pub trait PageRenderer: Send + Sync {
    async fn render(
        &self,
        request: RenderRequest,
    ) -> Result<RenderedPage, BrowserError>;
}
```

`RenderRequest` includes:

- typed URL;
- bounded timeout;
- typed wait condition;
- navigation headers where allowed;
- bounded output and artifact options.

`RenderedPage` includes:

- requested and final URL;
- navigation or response status;
- content type;
- HTML;
- timing information;
- optional screenshot or other artifacts.

It never exposes Chromiumoxide, CDP, Chrome process, or Lightpanda types.

### 6.2 Session API

The richer Browser API owns:

- launch and close;
- isolated profiles and sessions;
- open and navigate;
- compact semantic snapshots;
- stable element references within a snapshot generation;
- click, type, select, scroll, screenshot, and download;
- explicit high-risk script evaluation when policy permits it.

Snapshot references are convenience handles, not durable selectors. A new
snapshot generation invalidates references that no longer resolve safely.

### 6.3 Providers

Initial providers are:

- Chrome through the proven direct browser-control implementation extracted
  from Search;
- Lightpanda through the existing Search integration.

The Browser crate owns discovery, setup, doctor, lifecycle, pools, page guards,
wait conditions, and cleanup.

`agent-browser` is a design reference for client/service separation, sessions,
semantic snapshots, element references, profiles, doctor, and Agent-oriented
CLI output. It is not a required Cargo or subprocess dependency. An external
provider can be proposed later if its protocol becomes a concrete need.

## 7. Office Contract

### 7.1 Typed Operations

`a3s-use-office` exposes typed requests for operations supported by the pinned
OfficeCLI version:

- create and open;
- get and query;
- set, add, and remove;
- batch;
- validate;
- render;
- save and close.

Common lifecycle does not pretend that Word, Excel, and PowerPoint have one
identical document model. Format-specific requests remain typed within the
Office domain.

### 7.2 OfficeCLI Provider

The typed provider invokes OfficeCLI's documented native commands:

```text
open         → officecli open <document> --json
read         → officecli get <document> <selector> --json
mutate       → officecli batch <document> --input - --json
save/close   → officecli save|close <document> --json
```

`a3s use office <officecli-args...>` is transparent native CLI delegation: it
preserves argv, stdin, stdout, stderr, and process status. `a3s-use mcp serve
office` launches `officecli mcp` directly, so OfficeCLI continues to own its
standard MCP tool vocabulary and lifecycle.

OfficeCLI may use a resident process or private pipe internally. That transport
is not an A3S Use contract. A3S Use does not open the pipe, implement its
framing, or expose it to Rust callers.

The provider:

- pins the supported OfficeCLI version;
- disables OfficeCLI background auto-update;
- verifies its installed binary;
- reports provider and binary versions through doctor;
- retains Apache license, NOTICE, and third-party notices when redistributed.

### 7.3 Mutation Outcomes

Mutation retry policy is strict:

- Retry is allowed only when the provider proves the mutation process was not
  started.
- If the process started and the result is unavailable, return
  `use.office.outcome_unknown`.
- The caller must inspect or reopen the document before deciding whether to
  issue another mutation.

The client does not assume OfficeCLI deduplicates mutation request IDs.

## 8. Execution Surfaces and MCP Ownership

Each surface keeps its native contract:

| Surface | Contract | Owner |
| --- | --- | --- |
| Rust SDK | Typed traits, requests, responses, and errors | Browser or Office domain crate |
| CLI | argv, stdin, stdout, stderr, and exit status | Built-in domain or installed extension executable |
| MCP | Standard MCP initialization, capabilities, tools, resources, prompts, and transport | The selected Browser, OfficeCLI, or extension MCP server |
| Skill | Existing `SKILL.md` instructions and referenced assets | The installed extension package |

`a3s-use mcp serve browser` runs the A3S-owned Browser server.
`a3s-use mcp serve office` directly launches OfficeCLI's server. An external
package target directly launches the MCP executable declared in its ACL
manifest. A3S Use does not aggregate these servers, rename their tools, or
translate CLI and Skill surfaces into MCP.

The JSON-RPC framing used by MCP is an implementation detail of standard MCP;
A3S Use does not define an A3S-specific JSON-RPC dialect. Likewise, CLI
`--json` output is one versioned command result, not a request/response
protocol.

Browser session commands deploy the same Browser MCP server over authenticated
loopback Streamable HTTP. It binds to `127.0.0.1`, stores its random bearer
token only in a private generated receipt, shares typed Browser state across
MCP client sessions, and has bounded idle and maximum lifetimes. Service stop
is a standard MCP tool. No A3S request envelope or control protocol is added.

Search and stateless `browser render` always use the embedded Rust renderer.
Persistent deployment is an implementation choice for separate CLI
invocations, not a prerequisite for SDK callers or Search.

## 9. CLI Contract

Primary user commands are routed through `a3s use`:

```text
a3s use capabilities --json
a3s use doctor [browser|office] --json

a3s use browser render <url> --output page.html
a3s use browser open <url> --session <id>
a3s use browser list --json
a3s use browser navigate <url> --session <id>
a3s use browser snapshot --session <id> --json
a3s use browser click @e3 --session <id>
a3s use browser type @e2 "query" --session <id>
a3s use browser screenshot page.png --session <id>
a3s use browser close --session <id>

a3s use office open report.docx
a3s use office get report.docx /body --json
a3s use office batch report.xlsx --input -
a3s use office validate report.docx
a3s use office close report.docx

a3s use extension list --json
a3s use extension inspect acme/slack --json
a3s use slack channels list

a3s-use mcp serve browser
a3s-use mcp serve office
a3s-use mcp serve acme/slack
a3s-use mcp start browser --json
a3s-use mcp status browser --json
a3s-use mcp stop browser --json
```

The standalone forms use `a3s-use` instead of `a3s use` and keep identical
domain arguments.

CLI rules:

- `--json` writes machine data only to stdout;
- progress and diagnostics go to stderr;
- `--input -` reads a pipeline from stdin;
- artifacts have explicit paths and metadata;
- bounded operations accept explicit timeouts;
- partial batch outcomes are structured rather than hidden in logs.

## 10. Delegated Runtime Management

Use implements the machine interface consumed by the root component manager:

```text
a3s-use component list --json
a3s-use component status browser --json
a3s-use component install browser --json
a3s-use component uninstall office --json
a3s-use mcp start|status|stop browser --json
```

`use/browser` may be satisfied by a validated system browser or a managed
runtime. Uninstall removes only a Use-managed runtime and never removes the
system browser.

`use/office` is satisfied by the supported pinned OfficeCLI provider. Managed
uninstall removes only provider-owned runtime files recorded by A3S Use. It
does not reach into OfficeCLI's private resident-process transport. Documents
and rendered outputs are always user data.

Provider-specific receipt details remain owned by Use. The root CLI consumes
only the versioned delegated component schema.

Externally implemented domains use IDs such as `use/acme/slack`. The root CLI
delegates their install, update, and uninstall to Use. Use validates the
manifest, declared CLI/MCP/Skill surfaces, reserved routes, trust decision, and
bounded health checks before activation. It never discovers and executes an
extension from `PATH` alone.

## 11. Policy and Configuration

Actions are classified at least as:

```text
read
navigate
mutate
submit
download
execute
```

The library evaluates policy hooks but never owns an interactive approval
prompt. A3S Code or the invoking CLI owns approval UX. Script execution, form
submission, downloads, and Office mutation have higher risk than reads and
snapshots.

External extensions declare requested operations and risk classes, but those
declarations do not grant permissions. The host supplies only policy-approved
working directories, environment values, artifacts, and secret handles.

CLI configuration uses an optional typed `use` section in the existing A3S ACL
configuration. There is no separate TOML backend selector. Rust SDK callers
construct provider objects directly.

Human-authored product configuration and extension manifests use A3S ACL and
are parsed with `a3s-acl`; ACL is not HCL. JSON is used only for generated
machine state such as ownership receipts and for versioned CLI/MCP payloads.

## 12. Search Migration

Responsibilities move as follows:

| Concern | Target owner |
| --- | --- |
| Chrome and Lightpanda setup | `a3s-use-browser` |
| Install, status, and doctor | `a3s-use-browser` |
| Process lifecycle and pool | `a3s-use-browser` |
| Page guard and cleanup | `a3s-use-browser` |
| Navigation wait conditions | `a3s-use-browser` |
| `PageFetcher` | `a3s-search` |
| Search-specific `BrowserFetcher` | `a3s-search` |
| Google/Baidu query construction | `a3s-search` |
| Parsing, deduplication, and ranking | `a3s-search` |

`BrowserFetcher` receives `Arc<dyn PageRenderer>`. Unit tests inject a fake
renderer without starting a browser.

Feature intent remains:

```toml
[features]
headless = [
  "dep:a3s-use-browser",
  "a3s-use-browser/chrome",
]
lightpanda = [
  "headless",
  "a3s-use-browser/lightpanda",
]
```

Search removes direct dependencies used only for browser control, executable
discovery, and runtime archive installation. It depends on a released crate,
not a committed sibling path. Existing public management types receive one
minor release of deprecated re-exports; Search-specific fetchers stay in
Search.

## 13. Release Model

Use releases provide:

- `a3s-use` archives for every explicitly supported target;
- checksum manifest;
- license and third-party notices;
- crates.io releases for core, Browser, Office, and the facade as appropriate;
- the extension manifest/adapter crate used by the host;
- `a3s-lab/tap/a3s-use` for supported Homebrew targets.

Internal crates use one coordinated pre-1.0 release train but normal semantic
version dependencies. The root `a3s` formula does not depend on Use; Use stays
optional.

## 14. Diagnostics and Events

```text
a3s use doctor --json
a3s use doctor browser --json
a3s use doctor office --json
```

Doctor reports component version, provider version, resolved path, provenance,
platform support, surface compatibility, and repair suggestions without
exposing credentials or document contents.

Events follow lowercase dot-separated names:

```text
use.browser.session.started
use.browser.render.completed
use.office.document.closed
```

## 15. Verification

Browser tests cover:

- provider contracts;
- local HTTP navigation, redirects, timeouts, waits, and cleanup;
- bounded concurrent rendering and pool shutdown;
- snapshot reference generations;
- no leaked processes, sockets, or temporary profiles;
- Search parity using fixtures and injected renderers.

Office tests cover:

- native OfficeCLI argv, stdin, stdout, stderr, and process-status delegation;
- standard MCP launch delegation;
- launch, timeout, transport, and invalid-output failures;
- `outcome_unknown` mutation behavior;
- batch partial outcomes;
- save/close persistence;
- opt-in tests against the pinned OfficeCLI binary;
- no leaked processes, handles, or temporary files.

Standard MCP integration tests prove initialization and tool discovery without
adding an A3S-specific protocol. Streamable HTTP tests prove bearer-protected
loopback access, shared state across separate MCP clients, lifecycle cleanup,
and separate CLI invocation persistence. Real-Chrome session tests use a local
HTTP fixture and skip only when no compatible system provider is installed.

Extension conformance tests prove manifest validation, route conflict
rejection, native CLI/MCP/Skill behavior, policy, artifact boundaries, and
cleanup with implementations built outside the Use repository.

## 16. Domain Acceptance Criteria

- Search renders through `PageRenderer` without a CLI or MCP service.
- Browser implementation types do not escape the Browser crate.
- System and managed providers have explicit, testable install policy.
- Stateful Browser sessions work across separate CLI invocations.
- An explicitly installed external extension adds a new route without
  rebuilding Use and cannot override Browser, Office, or host commands.
- Artifact payloads are bounded and represented consistently.
- Office typed operations use native OfficeCLI commands, and Office MCP remains
  OfficeCLI-owned standard MCP.
- Ambiguous Office mutations return `use.office.outcome_unknown` and are never
  retried automatically.
- Uninstalling capability runtimes preserves profiles, documents, downloads,
  and rendered outputs.
