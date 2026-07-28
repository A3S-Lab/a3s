# A3S Use Extension Design

Status: Explicit local packages, TUF-signed registries, and workbench
contributions implemented

Parent: [A3S Use and Component Platform](a3s-use-component-platform.md)

Related: [A3S Use Domain Design](a3s-use-domain-design.md)

## 1. Decision

The default `a3s-use` distribution includes Browser and Office as first-party
built-in domains. Additional domains are independently distributed extension
packages.

An extension package declares one or more native surfaces:

- CLI for deterministic human and script commands;
- MCP for structured tools, resources, and Agent integration;
- Skill for Agent instructions and workflows.

It may also declare non-callable workbench contributions whose package-owned
assets are rendered by a host such as A3S Web.

Use does not define another universal RPC protocol. It owns the ACL manifest,
package identity, registration, trust, route selection, policy handoff, and
lifecycle around those existing surfaces.

For example, a package may add:

```text
a3s use slack channels list
```

without rebuilding or dynamically linking into `a3s-use`.

## 2. Why Native Surfaces

The three surfaces solve different problems and should retain their native
semantics:

| Surface | Best for | Native contract |
| --- | --- | --- |
| CLI | Humans, shell scripts, deterministic commands, pipelines | argv, stdin, stdout, stderr, exit status |
| MCP | Agent tools, JSON Schema inputs, resources, progress, cancellation | Standard MCP client/server protocol |
| Skill | Instructions, domain guidance, repeatable Agent workflows | `SKILL.md` package conventions |
| Workbench Activity | Reviewed context preparation and package navigation | Integrity-bound HTML rendered by the host |

Forcing all three through a new protocol would duplicate MCP, weaken normal CLI
behavior, and turn Skill into something it is not. A3S Use therefore has no
extension JSON-RPC method namespace, request envelope, response envelope, or
protocol version.

JSON is only an encoding at two machine-owned boundaries: versioned CLI output
selected with `--json`, and installation receipts. JSON output is not JSON-RPC.
An MCP surface follows standard MCP as an opaque native contract; any wire
format used by MCP remains an MCP implementation detail and is never wrapped in
an A3S Use protocol.

These boundaries are normative:

- the ACL manifest describes identity, files, and native surfaces, not callable
  RPC methods;
- CLI dispatch forwards argv, streams, and process status;
- MCP clients connect directly through the existing standard MCP subsystem;
- Skill packages are loaded through the existing `SKILL.md` loader;
- workbench Activities remain non-callable and reference a same-package Skill;
- a package may provide any combination of the three callable surfaces, and Use never
  converts one surface into another implicitly.

## 3. Goals

| ID | Goal |
| --- | --- |
| EXT-1 | Browser and Office remain reserved built-in routes in the default binary. |
| EXT-2 | An external package adds a domain without changing or relinking Use. |
| EXT-3 | A package can expose CLI, MCP, Skill, or any useful combination. |
| EXT-4 | Each surface uses its established execution and lifecycle contract. |
| EXT-5 | Identity, route, trust, ownership, policy, and diagnostics are consistent across surfaces. |
| EXT-6 | Install, upgrade, disable, and uninstall are explicit and ownership-safe. |
| EXT-7 | A package can contribute an integrity-bound host view without creating another execution protocol. |

## 4. Non-Goals

The first extension version will not:

- load Rust, C, or platform dynamic libraries into the Use process;
- execute a binary merely because it is on `PATH`;
- let an extension replace Browser, Office, or a host-management route;
- download an unknown extension when a user enters an unknown route;
- provide an unsigned remote marketplace;
- reinterpret MCP as a custom Use protocol;
- treat a Skill as an executable runtime;
- synthesize a polished CLI automatically from arbitrary MCP tools;
- expose a new-domain extension as a replacement for a typed Browser or Office
  provider;
- grant requested filesystem, network, secret, or process permissions without
  host policy approval.

## 5. Built-In and External Domains

```text
DomainRegistry
├── BuiltInDomain
│   ├── browser          typed Rust + CLI + MCP adapter
│   └── office           typed Rust + CLI + MCP adapter
└── ExternalDomain
    ├── acme/slack       CLI + MCP + optional Skill
    └── example/review   Skill + MCP
```

Built-ins call typed Rust services in process. External packages register
surface descriptors. The registry resolves identity, route, availability, and
policy, then hands execution to the native surface adapter.

The registry is not a universal operation API. Browser callers still use
`PageRenderer` and Browser session types. Office callers still use typed Office
requests.

## 6. Identity and Routing

An extension has:

- package ID: `<publisher>/<name>`, for example `acme/slack`;
- component ID: `use/<publisher>/<name>`, for example `use/acme/slack`;
- route: a short CLI segment, for example `slack`;
- version: semantic package version;
- one or more surface descriptors.

Package ID and route segments use lowercase ASCII names matching
`[a-z][a-z0-9-]*`.

Reserved routes include:

```text
browser
office
capabilities
component
extension
doctor
mcp
help
```

An extension cannot claim a reserved route. Two enabled extensions cannot claim
the same route. Activation returns `extension.route_conflict`; install order
and PATH order never choose the winner.

## 7. ACL Manifest

Each package contains `a3s-use-extension.acl`, parsed by `a3s-acl`.

```acl
extension "acme/slack" {
  schema_version = 1
  version        = "1.2.0"
  route          = "slack"
  actions        = ["read", "mutate", "submit"]

  cli {
    executable  = "bin/a3s-use-acme-slack"
    json_output = true
  }

  mcp {
    executable = "bin/a3s-use-acme-slack"
    args       = ["serve", "--mcp"]
    transport  = "stdio"
  }

  skill {
    path = "skills/slack/SKILL.md"
  }

  contributes {
    activity_bar "channels" {
      title       = "Slack"
      description = "Prepare a reviewed Slack context."
      icon        = "messages-square"
      entry       = "web/activity.html"
      skill       = "slack"
      order       = 140
    }
  }
}
```

ACL is the A3S Agent Configuration Language. The implementation uses
`a3s-acl`, not an HCL parser.

The final manifest schema includes:

- schema version, package ID, package version, and route;
- supported platform and architecture constraints;
- at least one CLI, MCP, or Skill surface;
- surface entrypoints relative to the package root;
- optional workbench contribution assets and same-package Skill bindings;
- requested action classes and resource requirements;
- publisher and package-integrity metadata when signed;
- license and notice locations.

Entrypoints cannot be absolute, contain parent traversal, or resolve outside
the package. Manifest declarations are requests and descriptions, not grants.

## 8. CLI Surface

When a route has a CLI surface:

```text
a3s use <route> <args...>
```

executes the receipt-owned binary directly without a shell.

The adapter:

- forwards argv without interpreting domain flags;
- connects user stdin directly so normal pipelines work;
- forwards human stdout and stderr in human mode;
- preserves exit status and signal outcome;
- passes only an approved environment and working directory;
- uses an explicit JSON flag only when machine output is requested and the
  surface declares support.

CLI extensions implement these bounded conventions:

```text
<binary> --version
<binary> doctor --json
<binary> capabilities --json      optional
```

The human command surface remains extension-owned. A CLI-only extension does
not automatically become a structured Agent tool; it should add MCP or a Skill
when that experience is required.

## 9. MCP Surface

An MCP surface declares the server executable, arguments, and supported
standard transport. Use and A3S Code reuse their MCP client implementation for:

- initialize and capability negotiation;
- tools, resources, and prompts;
- JSON Schema operation input;
- progress and cancellation;
- standard MCP errors and lifecycle.

Use does not wrap MCP messages in another protocol. A3S Code may connect to the
server directly through its existing MCP subsystem. Use may expose bounded MCP
inspection and call commands for scripts, but it does not invent extension-
specific request framing.

An MCP-only package is a valid extension. Direct `a3s use <route>` invocation
shows its available MCP surface and actionable tool-discovery command rather
than pretending it has an extension-defined CLI.

## 10. Skill Surface

A Skill surface points to a package-owned `SKILL.md`. A3S Code's existing Skill
loader owns parsing, trigger behavior, instruction loading, and Agent UX. Use
only validates package ownership and exposes the installed Skill location.

A Skill may teach an Agent how to use the package's CLI or MCP surface. A
Skill-only package is also valid, but it is an Agent capability rather than a
deterministic executable domain. Direct `a3s use <route>` reports that boundary
and points to the A3S Code Skill workflow.

Skill text never grants permissions and cannot bypass CLI/MCP policy or
approval.

## 10A. Workbench Contributions

`contributes.activity_bar` follows the VS Code contribution-point model without
making HTML an executable backend surface. A contribution declares a stable ID,
bounded display text, icon identifier, ordering hint, package-relative HTML
entry, and same-package Skill name.

A3S Use accepts only a regular UTF-8 `.html` file inside the immutable package
root, no larger than 2 MiB, and publishes its `text/html` media type and
lowercase SHA-256 in the capability snapshot. The consuming host independently
checks those fields, owns iframe/CSP isolation, and requires user review before
plugin-proposed context reaches Code. There is no generic execute message.

## 11. Surface Selection

A package may include multiple surfaces because callers have different needs:

- direct terminal invocation selects CLI when present;
- Agent structured operations select MCP when present;
- Agent guidance loads Skill when relevant;
- a Skill may reference the same package's CLI or MCP entrypoint;
- no adapter silently converts one surface into another.

If the requested caller surface is absent, Use returns
`extension.surface_unavailable` with the available surfaces and an actionable
alternative.

## 12. Public Management Commands

The umbrella component manager is canonical:

```text
a3s install use/acme/slack --from ./slack-extension
a3s upgrade use/acme/slack                        # when an update source exists
a3s install use/acme/slack --from ./v1.3 --force # explicit local upgrade
a3s uninstall use/acme/slack
a3s list --json
```

Use provides inspection and diagnostics:

```text
a3s use extension list
a3s use extension inspect acme/slack --json
a3s use extension doctor acme/slack --json
```

The root CLI delegates `use/` component requests to the trusted Use parent. Use
owns ACL manifest parsing, surface validation, route registration, and extension
receipts.

`--from` accepts an explicit local package directory or archive. An unsigned
local package requires `--allow-unsigned`. Remote packages come only from an
explicitly enrolled TUF registry and are applied through an immutable,
digest-reviewed umbrella CLI plan.

## 13. Discovery and Activation

Use discovers active extensions only from managed receipts and explicitly
registered development roots. It does not scan PATH for candidates.

Activation performs:

1. ACL parsing and schema validation;
2. component ID, route, and platform validation;
3. package integrity and trust validation;
4. reserved and duplicate route checks;
5. entrypoint containment and executable checks;
6. native surface validation:
   - CLI bounded version and doctor probes;
   - MCP initialize/capability negotiation;
   - Skill package validation through the Skill loader contract;
   - Activity package containment, regular-file, UTF-8, size, media-type, and
     content-digest validation;
7. atomic route activation and receipt write.

Failure before activation leaves the previous version unchanged. A package may
remain installed but disabled for inspection.

## 14. Lifecycle Ownership

Each surface retains its native lifecycle:

- CLI is normally one process per command. A package-owned background service
  remains an internal implementation detail and must be reachable only through
  a declared CLI or standard MCP surface.
- MCP follows standard server transport startup, negotiation, cancellation, and
  shutdown.
- Skill has no process lifecycle.
- Workbench contributions follow the package activation generation and become
  unavailable before disable or uninstall drains the callable surfaces.

Update validates all declared surfaces before switching the active route.
Existing operations finish or are cancelled explicitly; they are not silently
moved between package versions.

Uninstall:

1. disables new route resolution;
2. drains or cancels active CLI/MCP work according to policy;
3. stops package-owned processes where declared;
4. unregisters MCP and Skill surfaces;
5. removes only receipt-owned package files;
6. preserves user data.

## 15. Policy and Isolation

The extension declares requested actions and resources, but host policy is
authoritative.

The host controls:

- whether the extension or a surface may run;
- approved working directory and environment;
- filesystem roots;
- network policy where the selected runner can enforce it;
- secret handles or redacted values;
- artifact roots and size limits;
- process, time, and concurrency limits;
- approval for mutate, submit, download, and execute actions.

CLI and MCP processes do not receive the complete parent environment by
default. Skills receive no direct execution permission. A local process runner
and an isolated A3S Runtime/Box runner implement the same policy boundary; the
host must not claim isolation that its runner cannot enforce.

## 16. Output and Artifacts

CLI output follows its native stream contract. MCP content follows MCP types.
Skills produce no runtime output.

When an extension declares artifact production, the host allocates approved
roots and validates returned or discovered paths. Artifact metadata includes
path, media type, size, and content hash.

Artifacts, documents, profiles, downloads, and extension-created content are
user data. Normal uninstall removes package files and receipts, not outputs.
`--purge` remains limited to explicitly identified cache and runtime state.

## 17. Trust Model

Trust is separate from presence:

```text
first-party
verified-publisher
local-explicit
untrusted
```

- Built-in Browser and Office are first-party.
- A configured TUF registry with a pinned bootstrap root establishes signed
  registry provenance and fails on expiry, rollback, length, or hash mismatch.
- An explicitly accepted unsigned local package is local-explicit.
- Untrusted packages remain inspectable but disabled.

Unknown routes never trigger automatic install. Explicit installation and a
trust decision are always required.

## 18. Versioning

The ACL manifest schema and each surface contract version independently:

- unsupported manifest major versions prevent installation;
- CLI JSON output declares its schema version when used;
- MCP negotiates through the standard MCP lifecycle;
- Skill compatibility follows the installed Skill loader contract;
- receipts record manifest and surface versions;
- changing an existing manifest field incompatibly requires a new major.

There is no A3S Use extension-RPC version because no such protocol exists.

## 19. Implementation Support

`a3s-use-extension` provides Rust types and host adapters for:

- ACL manifest parsing and validation through `a3s-acl`;
- component identity, routes, surfaces, and trust state;
- safe CLI process invocation;
- reuse of the existing MCP client;
- reuse of the existing Skill loader;
- workbench contribution parsing and integrity projection;
- doctor aggregation and component status;
- conformance fixtures.

External implementers do not need this Rust crate. They implement a normal CLI,
standard MCP server, Skill package, or combination.

## 20. Verification

The conformance suite covers:

- valid and invalid ACL manifests;
- reserved and duplicate routes;
- missing, escaping, and non-executable entrypoints;
- PATH-only candidates never executing;
- CLI argv, stdin, stdout, stderr, status, doctor, and JSON behavior;
- MCP initialize, tool schemas, progress, cancellation, and shutdown;
- Skill path, ownership, and loader validation;
- Activity path, type, size, UTF-8, digest, and same-package Skill validation;
- TUF metadata verification and digest-reviewed remote plans;
- multi-surface selection without implicit conversion;
- trust and policy denial;
- atomic upgrade and ownership-safe uninstall;
- dynamic appearance in `a3s list --json`;
- at least one out-of-tree package for each supported surface type.

## 21. Acceptance Criteria

- The default binary exposes Browser and Office without extension installation.
- A CLI extension adds a deterministic route without rebuilding Use.
- An MCP extension exposes standard tools without a new wrapper protocol.
- A Skill extension is discoverable by A3S Code without being treated as an
  executable.
- A workbench contribution is content-bound, sandboxed by its host, and cannot
  name or invoke a foreign Skill.
- A multi-surface package uses the right native contract for each caller.
- Built-in and management routes cannot be shadowed.
- A PATH-only executable is never activated.
- Uninstall removes the package without touching Use or user data.
