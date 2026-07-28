# A3S CLI Technical Architecture

- Status: Accepted; incremental migration in progress
- Date: 2026-07-15
- Parent: [A3S CLI Product Design](cli-product-design.md)
- Delivery: [Migration and Verification Plan](cli-migration-plan.md)
- Related: [Cross-Platform Install Architecture](cross-platform-install-architecture.md)

## 1. Architecture Decision

The umbrella CLI will use one typed parse-dispatch-render pipeline. Command
handlers return typed outcomes and never own process termination or output
format selection. Product proxies remain process boundaries and receive raw
arguments, streams, execution context, and status without a shell.

```text
argv / environment / terminal facts
                |
                v
        clap parser and aliases
                |
                v
       InvocationContext builder
  config | output | policy | paths | cancellation
                |
                v
       typed command dispatcher
     /          |           |          \
  Code      components     services    proxy
     \          |           |          /
                v
          CommandOutcome
                |
                v
      human / JSON / JSONL renderer
                |
                v
        stdout, stderr, exit code
```

This is an in-process application architecture. It does not introduce a
daemon, universal action API, or custom JSON-RPC transport.

## 2. Current Problems to Remove

The current root and Code routers manually match strings. Help is incomplete;
`update` is overloaded; unknown Code words and some typos fall through; output,
prompting, and exit behavior vary by handler; global administration is nested
under Code; Web lacks lifecycle commands; and proxy arguments are forced
through UTF-8 `String`. The migration characterizes public behavior with
integration tests but does not preserve accidental fuzzy dispatch or the
documented-but-unrouted `a3s code view` form.

## 3. Parser and Command Types

Use Clap 4 derive APIs as a direct CLI dependency. The parser is the single
source of truth for help, usage, aliases, conflicts, value enums, completion,
and spelling suggestions.

Conceptually:

```rust
#[derive(clap::Parser)]
struct Cli {
    #[command(flatten)]
    global: GlobalOptions,
    #[command(subcommand)]
    command: Option<RootCommand>,
}

#[derive(clap::Subcommand)]
enum RootCommand {
    Code(CodeArgs),
    Web(WebArgs),
    Top(TopArgs),
    Box(ProxyArgs),
    Compose(ProxyArgs),
    Up(ProxyArgs),
    Down(ProxyArgs),
    Ps(ProxyArgs),
    Logs(ProxyArgs),
    Bench(ProxyArgs),
    Search(ProxyArgs),
    Use(ProxyArgs),
    Auth(AuthArgs),
    Model(ModelArgs),
    Config(ConfigArgs),
    List(ComponentListArgs),
    Info(ComponentInfoArgs),
    Install(InstallArgs),
    Upgrade(UpgradeArgs),
    Uninstall(UninstallArgs),
    Doctor(DoctorArgs),
    Registry(RegistryArgs),
    Cache(CacheArgs),
    Self_(SelfArgs),
    Completion(CompletionArgs),
    Version(VersionArgs),
    Help(HelpArgs),
}
```

Command value types use validated newtypes such as `ComponentId`, `SourceId`,
`ModelRef`, `SessionId`, `OutputMode`, and `InstallScope`. Strings are converted
at the parser boundary, not repeatedly inside handlers.

Deprecated forms use hidden Clap aliases or one explicit compatibility
normalizer before canonical parsing. They do not create duplicate handler
paths. The normalizer returns a canonical invocation plus structured warnings.
Aliases that cannot be expressed without ambiguity, such as the old dual-use
`update`, have narrowly scoped pre-parser rewrites with dedicated tests.

No `allow_external_subcommands` behavior is enabled at the root. Registered
proxy variants use a trailing raw argument field. Unknown root commands remain
usage errors.

## 4. Process Entry and Exit

The binary entry point returns `ExitCode`, passes `std::env::args_os()` to the
application runner, and contains no business logic. The runner performs:

1. compatibility normalization;
2. canonical parsing;
3. invocation-context construction;
4. cancellation registration;
5. dispatch;
6. one render pass;
7. exit classification.

Handlers return `Result<CommandOutcome, CliError>`. They do not call
`process::exit`, select a renderer, or print ad hoc JSON. Deep library code does
not know CLI exit codes.

Root-owned exit codes are deliberately small and stable:

| Code | Meaning |
| --- | --- |
| `0` | Requested operation completed successfully |
| `1` | Runtime, health, policy, authentication, or operation failure |
| `2` | Invalid command, argument, value, or usage |
| `3` | A multi-target operation completed with partial failure |
| `130` | Root-owned operation cancelled by Ctrl-C where the platform permits |

Machine-readable error codes carry detail such as `auth.required`,
`component.not_owned`, or `config.invalid`; adding such codes does not consume
new process exit codes. Proxy commands preserve the child exit code and signal
outcome as closely as the operating system permits.

## 5. Invocation Context

Each root-owned handler receives an immutable context:

```rust
struct InvocationContext {
    directory: CanonicalWorkspace,
    config: Arc<EffectiveConfig>,
    paths: Arc<A3sPaths>,
    output: OutputPolicy,
    interaction: InteractionPolicy,
    network: NetworkPolicy,
    terminal: TerminalCapabilities,
    cancellation: CancellationToken,
    diagnostics: Arc<dyn DiagnosticSink>,
}
```

The context is built once. Handlers do not independently rediscover config,
inspect TTY state, parse environment variables, or invent directory defaults.
Tests construct a context with isolated paths, deterministic terminal facts,
and in-memory output sinks.

The working directory is resolved before workspace configuration. Root-owned
commands pass paths explicitly rather than changing the global process current
directory. Proxies set the child current directory to the resolved directory.

The current migration checkpoint creates one token and one Ctrl-C listener at
the root. Code Exec, Top JSON/JSONL snapshot execution, and `web logs --follow`
consume that token. A cancelled machine stream writes its terminal error event
before the renderer returns exit `130`. Foreground Web shutdown and proxy
signal forwarding remain separate acceptance work and must not be inferred
from this checkpoint.

## 6. Configuration Architecture

### 6.1 One ACL Resolver

All human-authored product configuration and component or extension manifests
use A3S ACL and are parsed through `a3s-acl`. ACL is not HCL. The CLI must not
add an HCL parser, label ACL syntax as HCL, or use an HCL-specific intermediate
model.

The resolver implements this precedence:

```text
typed command flags
    > typed A3S environment overrides
    > explicit --config or A3S_CONFIG_FILE
    > workspace .a3s/config.acl
    > user config.acl
    > built-in defaults
```

An explicit config path selects a reproducible single file and disables the
normal workspace/user file merge. Otherwise, the workspace file is a typed
overlay on user configuration. Merge rules are defined per field; collections
are not accidentally concatenated or replaced through generic JSON merging.

The resolver returns provenance for each effective field so `config show`,
`config validate`, `model current`, and diagnostics can explain where a value
came from.

### 6.2 Writes

Configuration mutation goes through typed editors, never regex replacement.
Writes are validated before an atomic same-filesystem replacement and preserve
permissions. If the available ACL editor cannot preserve comments for a
section, the command must either use a section-aware core editor or refuse and
open `config edit`; it must not rewrite unrelated user content.

`config show` always redacts credentials and secret-derived values. Generated
installation receipts, journals, signed registry transport metadata, and CLI
JSON remain versioned machine JSON because they are not human product config.

## 7. Credentials and Sensitive Data

Credential ingestion is limited to:

- browser-based OAuth with a bounded callback;
- protected stdin selected by `--token-stdin`;
- an explicitly selected file whose permissions and type are validated;
- the platform credential store or a permission-restricted compatibility
  store.

Secrets are never accepted as positional values. The parser marks sensitive
options for redaction before diagnostics are created. Debug output records the
presence and source class of a credential, not its content, length, prefix, or
hash.

Credential material is not written to ACL, component receipts, transaction
journals, telemetry, shell history, URLs, or generated JSON. Child processes
receive the minimum scoped credential material needed for their operation.

## 8. Output and Error Model

### 8.1 Typed Outcomes

Handlers return semantic data:

```rust
enum CommandOutcome {
    Value(serde_json::Value),
    Table(TableModel),
    Text(TextArtifact),
    Stream(Pin<Box<dyn Stream<Item = Result<CliEvent, CliError>> + Send>>),
    Proxy(ProxyOutcome),
}

struct CliError {
    code: ErrorCode,
    message: String,
    suggestion: Option<String>,
    details: serde_json::Value,
    class: ExitClass,
    source: Option<anyhow::Error>,
}
```

Concrete Rust result types are preferred inside command modules; conversion to
`CommandOutcome` occurs at the presentation boundary. The value enum above is
conceptual and should not become a generic JSON business API.

### 8.2 JSON

One-shot machine output uses one envelope:

```json
{
  "schemaVersion": 1,
  "command": "component.list",
  "ok": true,
  "data": {},
  "warnings": []
}
```

Failures use the same envelope and a nonzero exit status:

```json
{
  "schemaVersion": 1,
  "command": "component.install",
  "ok": false,
  "error": {
    "code": "component.not_owned",
    "message": "Component 'search' is externally managed.",
    "suggestion": "Upgrade it with the package manager that installed it.",
    "details": {}
  },
  "warnings": []
}
```

Each command owns a versioned data schema. The common envelope does not imply
that unrelated commands share one untyped payload. Additive optional fields are
allowed within a schema version; removals, meaning changes, and type changes
require a new version.

Asset path fields remain JSON strings when the native path is valid UTF-8. A
native path that cannot be represented as a JSON string uses an object with
`display`, `encoding`, and lossless hexadecimal `value` fields. Current Unix
paths use `unix-bytes-hex`; Windows paths use `windows-wide-hex`. Path
resolution and process invocation always retain `PathBuf`/`OsString`; the
human display value is never reparsed as the path.

### 8.3 JSONL and Human Output

JSONL events include `schemaVersion`, `command`, `type`, a monotonic sequence,
and command-specific data. Final success or error is an explicit terminal
event. Lines are flushed individually. Truncated streams remain detectably
incomplete because they lack the terminal event.

Human renderers may use tables, Unicode, color, and TTY progress. They consume
the same typed result and never become the source for JSON. Data goes to stdout;
progress and diagnostics go to stderr. Broken pipes terminate quietly with the
conventional successful pipeline behavior where appropriate.

## 9. Interaction, Terminal, and Network Policy

`InteractionPolicy` is calculated centrally from output mode, explicit flags,
and terminal capabilities:

- JSON and JSONL are always non-interactive;
- `--non-interactive` disables every prompt;
- `--yes` answers only the plan confirmation associated with that command;
- missing trust, migration, elevation, or destructive-data consent still
  fails unless separately authorized;
- a prompt is allowed only when both its input and diagnostic output are safe
  TTYs;
- non-TTY progress is disabled rather than rendered as escape sequences.

`NetworkPolicy::Offline` prevents registry refresh, update checks, downloads,
OAuth browser login, and first-use installation before a request is sent. It
still permits local receipts, cached signed metadata, local packages, system
probes, and already installed proxies. `A3S_NO_AUTO_INSTALL=1` remains a
compatibility input and maps to the stricter first-use policy.

Color resolution is explicit flag, then `NO_COLOR`, then TTY capability. Child
processes receive compatible color and offline context without secrets.

## 10. Command Module Boundaries

A target layout keeps the boundary explicit without creating a monolithic
`cli.rs`:

```text
src/
├── main.rs
├── cli/                       args, compatibility, context, errors, renderers
├── commands/                  one orchestration module per root concern
├── components/                catalog and lifecycle application layer
├── proxy/                     resolution and child execution
├── api/                       Web application implementation
├── top/                       monitor model and views
└── tui/                       interactive Code implementation
```

Parser types contain no business logic. Command modules orchestrate existing
domain modules, which return types and errors rather than formatted strings.
Files split by concern before reaching repository size limits.

Typed asset execution lives under `commands/code`. DeepResearch itself lives
in the independent `a3s-deep-research` crate. That crate owns the engine stage
machine, workflow assets, source admission, report admission, and
Markdown/HTML construction. Headless CLI, TUI, and Code Web call the shared
`src/research/CodeDeepResearchRunner` product adapter. It provides an isolated
read-only `AgentSession`, explicit evidence scope, workspace publication,
progress events, cancellation settlement, and a typed run journal. No surface
owns a planner or report implementation.

Every new CLI, TUI, or Web run compiles a transient evidence-first contract with
`quota.mode = bounded` and `execution.mode = progressively_publishable`. The
contract travels with durable runtime input but never creates a user-facing
`.a3s/loops/` asset. One Host-owned wall-clock origin bounds acquisition,
optional report proposal, and finalization. Stable Flow effects retain their
own replay semantics; the version-2 Code run journal is a surface-refresh
projection and does not claim to resume an interrupted root process.

`CodeDeepResearchRunner::start` is the new-run entry point. It validates a
`DeepResearchRequest`, creates an empty Skill registry, removes Web tools for
local-only evidence, and delegates one cancellable transaction to
`DeepResearchEngine::execute_request` through `CodeDeepResearchRuntime`. The
engine runs durable bootstrap acquisition, combines the exact query with only
the planner's closed supplemental-query contract, stages a source-backed or
no-evidence report, optionally upgrades it with an admitted proposal, and
returns one typed result. There is no string router or second report
implementation.

Every result is run-scoped under
`.a3s/research/artifacts/<run-id>/report.md` and `index.html`. Engine events are
persisted at `.a3s/research/runs/<run-id>/journal-v2.jsonl` with a strict
sequence and matching run identity. The durable projection retains lifecycle,
stage, publication, and quality while deliberately omitting absolute artifact
paths. Code Web uses it to restore progress after a browser refresh. Artifacts
are served only by validated run ID and `html` or `markdown` kind.

Web context files become bounded relative `WorkspaceSourceHint` values and are
preflighted as existing, non-empty, non-symlink files before the isolated
session starts. Non-empty Skill selections fail explicitly until the typed
runner defines a supported Skill contract. TUI escape, Agent Island stop, Web
cancel, and handle drop all cancel the root run; explicit cancellation waits
for settlement and closes the isolated session.

Web bootstrap always searches the exact query. One optional bounded semantic
outline may supply zero to three supplemental plain-text queries. The Host
validates only their shape and exact identity; it does not derive query text
from topic words, dates, language, publishers, domains, or URL vocabulary.
If the outline is unavailable or invalid, the Host keeps only the exact query,
uses one generic track, and selects the conservative `comprehensive` plus
`freshness_required = true` contract. Planner failure therefore cannot
authorize an undated synthesized answer.
Candidate and chunk selectors may return only IDs from the closed packets.
Transport fallback preserves bounded acquisition opportunities but cannot
promote bytes into evidence. Provider rank, snippet, date, engine, title,
hostname, and publisher remain discovery metadata.
Search output must decode as the declared JSON result shape. Web and workspace
text is restored only from typed range offsets, returned character or line
counts, exact source anchors, and artifact truncation metadata. Provider error,
continuation, or truncation prose is never matched inside tool output.

Search, fetch, and structured-generation effects use stable A3S Flow identities.
Completed effects replay from their journals. A running effect whose completion
was not durably acknowledged is redelivered with the same attempt identity and
therefore has explicit at-least-once semantics. The local Flow JSONL store
preserves a complete final envelope missing only its delimiter and discards only
an unterminated torn tail before the next append; terminated or interior
corruption still fails closed.

The Host builds a source-backed Markdown/HTML pair before report generation can
become a terminal risk. An empty catalog instead produces the no-evidence pair.
When claim-eligible sources exist, one typed claim-graph proposal may run with
at most one transient retry. The proposal receives bounded source and chunk
IDs, no tools, and no publication authority. Rust independently admits facts,
inferences, recommendations, relations, and gaps through the evidence
compiler. It validates exact dimension/source/chunk IDs, basis edges,
derivation inputs, contradiction endpoints, provenance, and graph bounds. It
then derives coverage, citations, the source ledger, and both renderings from
one `ReportDocument`. Invalid graph items cannot erase valid siblings.

Each attempt uses a schema compiled from the current packet: dimension, source,
and chunk enums contain only the validated run, and audit-only sources cannot
be referenced. Opaque IDs are control data only, not reader prose. Reader
language is request-owned: the Host pins one inferred or explicit BCP 47 tag
through planning, generation, admission, and publication, and rejects an
altered tag or an obvious aggregate prose-language mismatch. Semantic
entailment remains a model/evaluator concern; the Host does not match claim
prose to source prose. Larger fetched catalogs are divided only by source
identity and a 32 KiB UTF-8 packet budget; a closed exact-ID reduction retains
at most four excerpts per source. Complete material coverage publishes
`Synthesized`; useful admitted claims with a material typed gap publish
`Qualified` only after the resolved dimensions pass the per-dimension depth
gate. If every dimension remains bounded, the Host admits exactly one qualified
partial conclusion only when it independently supplies the full two-source
comparison, explanation, implication, boundary, and substantive-length chain
plus a typed gap. A focused report may be structurally sufficient with one
cited direct-answer claim.

Markdown and HTML publication pairs carry the same versioned artifact marker.
The Host never infers synthesized, source-backed, no-evidence, recovery, or
fallback status from titles or body vocabulary.

The fixed HTML renderer follows the A3S Web design tokens. Desktop output uses
a sticky left action menu, centered report surface, and sticky right table of
contents; narrow screens stack both navigation regions without horizontal
overflow. One Host-owned script supports edit mode, title and table-of-contents
synchronization, saving a self-contained HTML copy, and printing. Arbitrary
reader-authored scripts remain rejected.

The returned status distinguishes `synthesized`, `qualified`, `source_backed`,
and `no_evidence`; it never equates artifact availability with semantic truth.
Bootstrap metadata is deliberately withheld from the Host-owned terminal tool
result so workflow canonicalization cannot replace publication output with
acquisition output. Historical Inquiry events remain readable through the
generic event journal, but the former sectioned-report executor and report
resume transaction have been removed. They are not selectable runtime paths.

## 11. Component Application Layer

The existing component catalog, discovery, lifecycle, and updater mechanics
remain separate concerns:

```text
CLI request
  -> catalog resolution
  -> target and installed-state probe
  -> source resolver
  -> immutable plan
  -> confirmation policy
  -> transaction executor
  -> verification
  -> receipt and typed result
```

`install`, `upgrade`, and `uninstall` share this pipeline. They do not each
reimplement source selection or output. Multi-component operations plan all
targets, serialize conflicting component work, execute independent components,
and return every result. One failure does not discard prior results or falsely
claim cross-manager rollback; mixed results exit with code 3.

No-argument `upgrade` resolves and reports candidates but does not apply them.
`--all` is represented in the request type and cannot be inferred after
planning. `--dry-run` ends after producing the same immutable plan that a real
operation would require. Plan digests protect approval from source, scope, or
privilege changes between review and execution.

The component manager supports registered A3S identities, not arbitrary native
package names. Backends construct typed argv for trusted source kinds. They do
not execute registry-provided command strings, remote shell scripts, or
installer command definitions embedded in ACL.

### 11.1 Code host command execution

Code does not provision or attach a local process sandbox. Bash routes through
the active workspace backend's host command runner with the workspace as its
current directory. TUI, Web, and headless execution share one Rust policy
adapter over Core's `InteractiveToolGuardrail`. The classifier fails closed on
critical commands and silently admits only a narrow subset proven read-only by
its command structure, options, paths, pipeline stages, expansion, redirection,
and workspace-symlink checks. Credential and control paths remain a hard floor
that remembered grants cannot override. Default asks for unproven commands,
Plan denies Bash, and Auto allows only the proven read-only subset. The
permission checker, confirmation provider, command timeout, process-group
cancellation, output bounds, and streaming observer are snapshotted with each
admitted run and inherited by delegated work.

Release archives, Homebrew, and standalone self-update therefore carry no
JavaScript sandbox support tree and install no Node.js or native sandbox
prerequisites. Runtime owns durable Task and Service placement, while Box owns
OCI and stronger-isolation workloads. Commands that require an isolation
boundary must use those products instead of the local host runner.
The Rust classifier is a policy guardrail, not OS isolation: it cannot prevent
PATH substitution, arbitrary build scripts or Git hooks, filesystem races,
host-file access, or network access after a process is authorized.

Executable discovery and version probes use bounded output files and an
explicit portable timeout. They must not install process-global signal
handlers: the root invocation owns signal registration, and component probes
may run while that listener is active.

## 12. Proxy Architecture

Proxy argument storage uses `Vec<OsString>`, not `Vec<String>`. The runner:

1. resolves a registered `ComponentId`;
2. applies offline and catalog-authorized first-use policy;
3. verifies health and compatibility;
4. selects the absolute executable from trusted state;
5. sets the resolved child working directory;
6. forwards raw argv and inherited stdin/stdout/stderr without a shell;
7. waits with signal forwarding;
8. preserves the child outcome.

Arguments after `box`, `bench`, `search`, or `use` belong to the child and are
not parsed by the root. Universal root options are parsed before the proxy
namespace. A versioned `A3S_CLI_*` child context conveys directory, output,
color, progress, offline, and non-interactive policy to compatible first-party
children. During migration, an incompatible child receives only safe process
context and the root reports which global behavior it cannot guarantee.

There is no generic fallback from an unknown root word to `a3s-<word>`. Dynamic
Use domains remain inside the trusted `use` namespace, where A3S Use validates
their ACL package and declared CLI, MCP, and Skill surfaces.

`a3s use box ...` is the one composed proxy route. The root resolves both
registered components and remains the sole Box lifecycle owner. It passes the
canonical Box executable to Use in the child environment; Use validates that
explicit path and delegates to it. No PATH rediscovery, wrapper package,
copied binary, or second receipt is allowed. Non-Box Use calls may receive an
already-ready Box path for diagnostics, but they never trigger Box
installation.

Proxying `a3s search` is an umbrella UX boundary only. Search continues to link
the typed `a3s-use-browser` renderer library directly; it does not call the Use
CLI or depend on a resident Use process.

Root-to-child component lifecycle uses argv, one versioned JSON document,
stderr diagnostics, and an exit status. Long-running domain tools use their
native CLI stream or standard MCP. None of these contracts is JSON-RPC.

## 13. Web Lifecycle Architecture

Detached Web instances use a cross-platform child-process supervisor contract,
not Unix-only daemonization. One managed instance is identified by each
canonical workspace. State records include:

- schema version and instance ID;
- canonical workspace and bound address;
- PID plus the recorded launch time;
- executable path and version;
- a random launch nonce known to the worker;
- log path, start time, and readiness state.

`web start --detach` launches a hidden internal worker mode and waits on a
bounded readiness handshake. It writes state atomically only after the server
binds. A workspace-keyed lifecycle lock makes concurrent starts converge on the
same worker. Failure returns the child diagnostic and cleans incomplete state.

`stop`, `status`, `logs`, and `open` resolve the same instance. Before requesting
shutdown, stop verifies the recorded PID and random launch nonce against the
private control route. A stale or ambiguous record is reported and quarantined;
it never causes a blind kill.
Graceful shutdown has a bounded timeout and does not fall back to force
termination.

Before configuration or session restoration, foreground startup reserves the
requested listener and detached startup probes any occupied address. A
versioned A3S health response identifies healthy foreground and legacy
instances for reuse and diagnostics, but it does not expose the control nonce
or confer general stop authority. `--replace` uses the authenticated control
route for a managed record. For an observed foreground instance, replacement
additionally requires the same canonical workspace, a health-reported PID, the
current A3S executable, a `web` command with the requested explicit port, and a
second health probe immediately before signaling. The CLI sends an interrupt
and waits for the listener to be released. A foreign or ambiguous listener is
never signaled.

Foreground and detached modes use the same server configuration and startup
path. Logs rotate under the shared state/log path policy and never contain
credentials or authorization headers.

## 14. Code Command Integration

The Code TUI, `code exec`, Web sessions, and research should reuse one session
application layer rather than parse or construct model/config state separately.
That layer owns:

- effective workspace and ACL configuration;
- session creation, resume, list, export, and deletion;
- model resolution and credential handles;
- permission and confirmation policy;
- event streaming and final results;
- cancellation and artifact reporting.

The TUI renders session events interactively. `code exec` renders the same
events as human output or JSONL. Web adapts them to its HTTP/SSE contract.
Research composes a bounded workflow and report artifact policy on top. The
shared layer does not force these surfaces through JSON-RPC.

Code Intelligence is a separate read-only workspace capability inside that
shared application layer. A local host builds one `ManifestWorkspaceBackend`,
then attaches the native provider to the resulting `WorkspaceServices`. The
provider subscribes to the existing manifest change stream and uses the same
workspace filesystem and path resolver; it must not start a second watcher,
file index, text-search service, mutation path, or memory store.

The Rust runtime owns framed stdio language protocol requests and child-process
lifecycle directly. TUI and Web never spawn language processes themselves.
They call the typed `WorkspaceCodeIntelligence` service asynchronously and
reuse their existing file-selection flows for returned locations. Web caches
the service bundle by canonical workspace and resolves an optional session ID
only to an already loaded workspace. Cache and process shutdown are explicit.

Semantic positions are zero-based UTF-16 throughout Core and HTTP contracts.
All queries use saved files and include bounded result metadata; dirty editors
must display saved-version behavior. Absolute paths, traversal, symlink
escapes, unknown sessions, malformed protocol locations, and unsupported
capabilities fail through typed errors before a file is exposed.

Interactive launch resolves an immutable `CodeRuntimeConfiguration` before
building a session. It contains the effective A3S ACL, primary ACL path, Code
asset roots, and memory root, all resolved from the invocation directory. TUI
panels receive these paths explicitly and never change the process current
directory to emulate `-C`.

Asset-family commands use a common typed discovery request with an explicit
`AssetLocation`. Each family registers only supported lifecycle operations.
Agent kind is a value enum option, removing positional inference.

## 15. Help and Completion

Clap generates parsing, root/nested help, and completion from the same types.
Help shows canonical forms, precedence, network/mutation/privilege behavior,
examples, and related commands. `a3s help <path...>` and `<path...> --help` are
equivalent. Docs tables are generated or checked against parser metadata, and
deterministic snapshots verify wrapping and errors. Suggestions never execute.

## 16. Migration and Verification

Compatibility normalization, release milestones, parser/output/security/proxy
test matrices, and acceptance gates are defined in the
[Migration and Verification Plan](cli-migration-plan.md). Built-binary tests
must use isolated roots and disable automatic installation by default.

## 17. Architectural Invariants

- There is one canonical parser and one render boundary.
- Unknown root commands never execute discovered binaries.
- Handlers do not call `process::exit` or print ad hoc machine JSON.
- Human-authored product data uses A3S ACL through `a3s-acl`, never HCL.
- CLI JSON is a versioned process result, not JSON-RPC.
- Standard MCP stays MCP; Skills stay `SKILL.md`; native CLI stays argv and
  streams.
- Secrets never enter argv or generated machine state.
- Offline is enforced before network I/O.
- Dry-run and apply share the same resolved plan.
- Files are deleted only with proven ownership.
- Proxies use absolute executables, no shell, raw native arguments, and child
  status preservation.
- Web lifecycle validates process identity before signaling.
- Public behavior is covered by built-binary integration tests on every
  supported operating-system family.
