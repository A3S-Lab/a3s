# A3S CLI Product Design

- Status: Accepted; incremental migration in progress
- Date: 2026-07-15
- Scope: Umbrella `a3s` command surface
- Related: [Technical Architecture](cli-technical-architecture.md),
  [Migration Plan](cli-migration-plan.md),
  [Component Management](component-management-design.md), and
  [A3S Use and Component Platform](a3s-use-component-platform.md)

## 1. Decision

The `a3s` executable will become one coherent product CLI rather than a manual
router around independently evolved commands. The redesign uses these rules:

- keep frequent component lifecycle verbs at the top level;
- group secondary administration by noun, then verb;
- reserve explicit top-level namespaces for bundled products and trusted
  component proxies;
- use one global interaction and output contract for root-owned commands;
- keep aliases only for compatibility, never as competing documented forms;
- use A3S ACL for human-authored configuration and manifests;
- use ordinary CLI process contracts, standard MCP, and Skills instead of a
  custom JSON-RPC protocol.

The principal naming correction is:

```text
a3s self update       # update the a3s executable
a3s upgrade <id>      # upgrade an installed component
```

`update` will no longer have two meanings.

## 2. Scope

This design covers every command parsed or owned by the umbrella CLI,
including non-interactive A3S Code commands. It defines the proxy boundary for
Box, Bench, Search, and Use, but their internal command trees remain owned by
their respective executables. Interactive `/commands` inside the Code TUI are
not shell commands and are outside this design.

This document is the accepted target product contract. The implementation
baseline and remaining gaps are tracked in the migration plan; a target
contract must not be presented as implemented until its acceptance gate passes.

## 3. Product Principles

1. **Predictable grammar.** Commands use `noun verb` for administration and a
   small set of conventional top-level verbs for frequent package operations.
2. **One canonical spelling.** Full words are documented; abbreviations are
   compatibility aliases only.
3. **Safe reads by default.** Listing, inspection, planning, and diagnosis do
   not mutate state. Network access is explicit or evident from the command.
4. **Explicit mutation.** Destructive or provenance-changing work exposes a
   plan, confirmation policy, and dry-run behavior.
5. **Automation is a first-class client.** Machine output, exit status,
   non-interactive behavior, and stream formats are stable contracts.
6. **Human output may improve.** Scripts must request JSON or JSONL instead of
   scraping tables, colors, progress bars, or prose.
7. **No surprise execution.** Unknown commands do not discover and execute an
   arbitrary `a3s-*` binary from `PATH`.
8. **Secrets are not arguments.** Credentials never use positional CLI
   arguments and are redacted from output and diagnostics.
9. **Ownership controls deletion.** A3S mutates only receipt-owned files or
   delegates to the package manager or trusted parent that owns them.
10. **Cross-platform means declared support.** Unsupported targets fail before
    mutation; A3S is not a universal frontend for arbitrary operating-system
    packages.

These principles follow current patterns in the uv, GitHub, Docker, kubectl,
and Winget CLIs and the cross-vendor guidance at clig.dev.

## 4. Target Command Tree

```text
a3s
├── code                         interactive coding agent
│   ├── exec                     non-interactive coding task
│   ├── resume                   resume the newest or selected session
│   ├── research                 evidence gathering and report generation
│   ├── session                  list, show, export, or delete sessions
│   ├── agent                    Agent asset lifecycle
│   ├── mcp                      MCP asset lifecycle
│   ├── skill                    Skill asset lifecycle
│   ├── flow                     Flow asset lifecycle
│   ├── okf                      OKF asset lifecycle
│   ├── kb                       workspace knowledge base
│   ├── context                  durable context history
│   └── memory                   long-term memory
├── web                          start, stop, inspect, and open A3S Web
├── top                          interactive monitor or structured snapshots
├── box                          transparent a3s-box proxy
├── compose                      transparent a3s-box Compose namespace
├── up / down / ps / logs        frequent Compose workflow shortcuts
├── bench                        transparent a3s-bench proxy
├── search                       transparent a3s-search proxy
├── use                          transparent a3s-use proxy
├── auth                         account login, logout, and status
├── model                        model discovery and selection
├── config                       A3S ACL configuration
├── list                         component inventory
├── info                         component details and sources
├── install                      component installation or repair
├── upgrade                      component upgrade planning and execution
├── uninstall                    ownership-safe component removal
├── doctor                       read-only diagnostics
├── registry                     trusted component registries
├── cache                        download and derived-data caches
├── self update                  update the a3s executable
├── completion                   generate shell completion
├── version                      print version information
└── help                         command help
```

No-argument `a3s` prints concise help. It does not implicitly launch Code or
perform an update.

Command groups print help when their verb is omitted. The documented
exceptions are action commands with an intentional no-argument behavior:
`code` launches the TUI, `web` starts in the foreground, `top` opens the
monitor, `list` lists components, `install` lists available components, and
`upgrade` lists available upgrades. No other group guesses a default verb.

## 5. Global Contract

Root-owned commands share these options:

```text
-h, --help
-V, --version
-C, --directory <path>
    --config <path>
    --output human|json|jsonl
    --json
-q, --quiet
-v, --verbose
    --color auto|always|never
    --no-progress
    --offline
    --non-interactive
```

`--json` is a stable shorthand for `--output json`. JSONL is accepted only by
streaming commands such as `top`, `web logs`, and event-producing Code tasks.
An unsupported output mode is a usage error rather than a silent fallback.

Mutation-specific options are not global:

```text
--dry-run        resolve and display a plan without applying it
--yes            accept the displayed plan, but not extra trust or privilege
--force          repair within current ownership and provenance
```

`--force` never implies `--yes`, unsigned trust, source migration, privilege
elevation, or deletion of user data.

Human results go to stdout. Progress, warnings, and diagnostics go to stderr.
JSON mode writes exactly one versioned document to stdout. JSONL writes one
versioned event per line. Prompts, spinners, colors, and decorations never
appear in machine output. `NO_COLOR` and non-TTY output are honored.

JSON and JSONL imply non-interactive behavior. Human mode prompts only when the
required terminal streams are TTYs; otherwise a missing decision fails with an
actionable error and the flag needed to continue.

## 6. A3S Code

### 6.1 Interactive, Execution, and Sessions

```text
a3s code
a3s code exec [<prompt>] [--prompt-file <path>] [--mode plan|default|auto]
a3s code resume [session-id]
a3s code session list
a3s code session show <session-id>
a3s code session export <session-id> [--output-file <path>]
a3s code session delete <session-id> [--yes]
```

`code` with no subcommand launches the TUI in the effective directory.
`code exec` is the explicit automation surface; it accepts one prompt argument,
a prompt file, or piped stdin. Arbitrary trailing text after `a3s code` is never
guessed to be a prompt. It emits a final result in JSON or an event stream in
JSONL. Any approval that cannot be resolved in non-interactive mode fails
instead of blocking on hidden input. `auto` uses the shared risk classifier to
approve bounded workspace operations; high-risk or unknown operations still
fail with `approval.required`. A successful result requires a terminal Code
completion event rather than merely a closed event stream.

`code resume` remains canonical because it is a frequent user action. The
`session` group owns less frequent inspection and data lifecycle operations.
Deleting a session never deletes workspace files or memory.

Inside the TUI, `/relay` is the interactive workspace-session picker. Its A3S
Code tab resumes a native session together with its per-session model, effort,
execution mode, theme, and paused-goal state. The Claude Code, Codex, and
WorkBuddy tabs read only project transcript files and submit the selected
session's latest user task to the active A3S Code session; they do not import
external credentials or mutate the external transcript.

The TUI `/ide` surface also exposes Code Intelligence commands for status,
document or workspace symbols, definitions, declarations, references,
implementations, and document or workspace diagnostics. Results are modal,
navigable lists that open through the existing editor file-selection path.
They always describe saved files; a dirty editor must label that fact and is
never overwritten by a navigation result. Code Intelligence returns semantic
metadata and locations only. Existing workspace tools remain responsible for
source reads, text search, and all mutations.

### 6.2 Research

```text
a3s code research <query>
    [--local-only|--web]
    [--report-dir <path>]
```

The command replaces `deepresearch` and `deep-research`. It always reports the
Markdown and HTML artifacts it created. DeepResearch has one host-managed
runtime; callers choose only the evidence scope.

New runs use an evidence-first `quota.mode = bounded`,
`execution.mode = progressively_publishable` contract. Web acquisition starts
from the exact user query and one Host-owned, date-aware outcome-and-news
companion query. No model may expand, rewrite, or retry another provider query.
One closed semantic candidate-admission decision runs before fetch; a valid
non-empty selection may be filled only with bounded distinct-host institutional
or accountable-publisher candidates. A failed admission selects a bounded
deterministic acquisition fallback, while an explicit empty selection remains
empty. Provider titles, snippets, ranks, dates, and engine metadata are never
report evidence.

The Host canonicalizes and sanitizes fetched content into a closed source
catalog. Safe siblings survive search or fetch failures. As soon as a non-empty
catalog exists, the Host materializes a source-backed Markdown and HTML pair
that preserves bounded excerpts, exact links, provenance, and an explicit
evidence limitation. With no safe source, it materializes an honest no-evidence
pair. A later model call is therefore never the sole authority for artifact
availability.

When at least one source is eligible to support a conclusion, one optional
closed report proposal may run with one transient retry. It receives source
aliases and bounded excerpts, no tools, and no authority to introduce a URL.
The Host admits report blocks independently, resolves every citation against
the catalog, enforces query-language, date, number, direct-answer, Findings,
atomic-claim, and strong-source gates, and rebuilds the source ledger from the
accepted blocks. An invalid block cannot erase a valid sibling, and an invalid
or unavailable proposal leaves the source-backed report in place.

Publication status is operational: `synthesized` passed complete Host admission,
`qualified` preserves useful admitted claims with an explicit typed material
gap, `source_backed` preserves fetched evidence without claiming a completed
synthesis, and `no_evidence` records the acquisition boundary. The Host renders
and atomically replaces both artifacts from the admitted report document.
Headless CLI, the TUI `?` path, and Code Web call the same typed runner. Stable
Flow identities retain their effect-level replay semantics; bootstrap workflow
metadata can never replace the Host-owned terminal publication result.

Artifacts are keyed by run ID under `.a3s/research/artifacts/`. The version-2
run journal stores a strict typed lifecycle/stage/publication projection
without absolute artifact paths. Code Web restores that projection on page
refresh and serves artifacts only by validated run ID and format kind.

### 6.3 Asset Families

The canonical discovery grammar is the same for Agent, MCP, Skill, Flow, and
OKF:

```text
a3s code <family> list --location local|os|all [query]
a3s code <family> clone <git-url>
a3s code <family> review [path]
a3s code <family> activity [query]
```

`--location` is required because local development sources and OS digital
assets have different availability, authentication, latency, and ownership.
Commands do not choose a location based on whether the user happens to be
logged in.

Lifecycle verbs remain family-specific:

| Family | Canonical lifecycle commands |
| --- | --- |
| `agent` | `publish`, `run`, `deploy`, `open`, `logs`, `status` |
| `mcp` | `publish`, `run`, `test`, `deploy`, `open`, `logs`, `status` |
| `skill` | `publish`, `deploy`, `open`, `status` |
| `flow` | `publish`, `run`, `deploy`, `open`, `logs`, `status` |
| `okf` | `publish`, `deploy`, `status` |

For Agent actions, the asset path is the operand and kind is an option:

```text
a3s code agent publish [path] --kind agentic|application|tool
a3s code agent run [path] [--kind agentic|application|tool]
```

This removes the current ambiguity between an optional kind and an optional
path. Other family actions retain a single optional path where supported.
Unsupported family/verb combinations are parser errors and never fall through
to another behavior.

### 6.4 Knowledge, Context, and Memory

```text
a3s code kb stats
a3s code kb add <text>
a3s code kb import <file-or-directory>
a3s code kb search <query>
a3s code kb path

a3s code context search <query>
a3s code context show event <event-id> [--window <count>]
a3s code context show session <session-id>

a3s code memory list [query]
a3s code memory stats
a3s code memory path
```

Canonical commands do not treat unknown words as search queries. This ensures
that typos fail early and completion remains trustworthy. TUI-only attachment
or promotion operations remain interactive-only.

## 7. Web and Monitor

### 7.1 A3S Web

```text
a3s web start [--detach] [--replace] [--host <host>] [--port <port>]
    [--directory <path>] [--web-dir <path>] [--api-only]
a3s web stop [--directory <path>]
a3s web status [--directory <path>]
a3s web logs [--directory <path>] [--follow]
a3s web open [--directory <path>]
```

`a3s web` remains a documented human shortcut for `a3s web start`. Scripts
should use the explicit verb. Foreground is the default; `--detach` creates a
managed instance associated with the canonical workspace. Stop and status
validate process identity and never signal an unrelated process from a stale
PID file. Repeated detached starts reuse a healthy same-workspace instance,
concurrent starts converge under a workspace lock, and stale records are
quarantined. `--replace` gracefully replaces a CLI-managed instance or a
same-workspace foreground A3S Web process verified by health PID, executable,
command, and explicit port. It does not kill an unknown or ambiguous port
listener.

GitHub archives and Homebrew installations carry the matching Web workspace.
When Cargo cannot install those data files, the first online Web start resolves
only the CLI's exact release version, verifies the published SHA-256, safely
extracts the archive, and atomically activates a versioned user-data cache
behind a cross-process lock. Offline mode and `A3S_NO_AUTO_INSTALL=1` never
contact the release service. A fixed detached port is validated before the
first-use download begins.

Start is idempotent: it reuses a healthy workspace instance instead of treating
repeat invocation as a failure. It may discover a healthy foreground or legacy
A3S instance through the versioned health contract, but that observation does
not grant general lifecycle ownership. `--replace` uses authenticated graceful
shutdown for a managed instance. It may interrupt an observed foreground
instance only after its workspace, health PID, executable, `web` command, and
explicit requested port are verified twice around process inspection. Foreign
and ambiguous port owners are refused. Port ownership is checked before assets,
configuration, or persisted sessions are loaded.

Web sessions for the same canonical workspace share one Code Intelligence
runtime. Monaco consumes typed status, outline, navigation, and diagnostics
routes for document symbols, markers, and editor actions. Workspace symbol
search is available through the typed API but does not replace or duplicate the
existing text-search panel. Dirty buffers stay browser-local and semantic
status explicitly says that results use the saved version.

### 7.2 A3S Top

```text
a3s top [--container <container>]
    [--view agents|sessions|containers|processes|events]
    [--connector a3s-box|docker|runc]
    [--active|--all]
    [--filter <text>]
    [--sort <field>] [--reverse]
    [--risk all|medium|high]
    [--kind all|tool|security|file|egress|llm|other]
    [--watch] [--interval <duration>] [--count <count>]
    [--compact] [--no-header] [--invert]
```

Human TTY output opens the monitor. JSON returns exactly one snapshot envelope
and rejects stream-only flags. JSONL requires `--watch` and emits sequenced
`snapshot` events followed by exactly one `result` or `error` terminal event.
`--count` creates a bounded stream; otherwise Ctrl-C terminates the stream with
the cancellation contract. A single `--view` enum replaces the current set of
competing tab flags.

## 8. Authentication, Models, and Configuration

### 8.1 Authentication

```text
a3s auth list
a3s auth status [provider]
a3s auth login [provider] [--token-stdin|--token-file <path>]
a3s auth logout [provider]
```

The initial managed provider is `os`; discovery may also report compatible
Claude and Codex account state without claiming ownership of their credentials.
Browser OAuth is the default OS login. Tokens are accepted only from protected
stdin, an explicitly selected credential file, or the platform credential
store. A token is never accepted as a positional argument.

### 8.2 Models

```text
a3s model list
a3s model current
a3s model use <provider/model> [--scope workspace|user]
a3s model reset [--scope workspace|user]
```

Model discovery distinguishes runtime-callable models from digital assets
whose category is `model`. `model use` validates the target before atomically
updating the selected ACL configuration layer. TUI `/model` selection remains
session state and does not silently rewrite product configuration.

### 8.3 Configuration

```text
a3s config path
a3s config paths
a3s config show
a3s config init [--scope workspace|user] [--force]
a3s config edit [--scope workspace|user]
a3s config validate [path]
```

`config show` prints the effective, redacted configuration; it is not a raw
secret dump. `config validate` parses with `a3s-acl` and reports source
locations. `config paths` reports configuration, data, state, cache, asset,
memory, KB, and OKF roots.

Human-authored configuration is A3S Agent Configuration Language in `.acl`
files. ACL is not HCL, and no HCL parser or HCL terminology is used.

The effective precedence is:

1. command flags;
2. typed `A3S_*` environment overrides;
3. an explicit `--config` or `A3S_CONFIG_FILE`, when present;
4. workspace `.a3s/config.acl` over the user ACL configuration;
5. built-in defaults.

When an explicit config path is present, it replaces the normal workspace/user
file stack so execution is reproducible.

## 9. Component Lifecycle

The high-frequency lifecycle commands remain top-level:

```text
a3s list [--installed|--available|--updates] [--kind <kind>]
a3s info <component> [--versions] [--sources]

a3s install <component>...
    [--version <requirement>]
    [--channel stable|beta|nightly]
    [--source auto|<source-id>]
    [--scope user|system]
    [--from <local-package>]
    [--dry-run] [--offline] [--migrate] [--force]
    [--allow-unsigned] [--yes]

a3s upgrade
a3s upgrade <component>... [--dry-run] [--offline] [--yes]
a3s upgrade --all [--dry-run] [--offline] [--yes]

a3s uninstall <component>...
    [--cascade] [--purge] [--dry-run] [--yes]

a3s doctor [component]
```

No-argument `install` lists available components without mutation.
No-argument `upgrade` lists available upgrades without mutation. `upgrade
--all` is required to mutate every eligible component. Missing components are
not silently installed by `upgrade`.

`doctor` is read-only and returns a failing exit status when required health
checks fail. It may suggest an exact install, repair, authentication, or config
command, but it does not accept a hidden `--fix` mutation mode.

`a3s install` manages registered A3S components and delegated capabilities. It
does not accept arbitrary Homebrew, Winget, APT, DNF, Pacman, language-package,
or operating-system package names. Supported native managers are trusted
backends for declared component sources; they retain ownership of their files.

Source selection uses trusted catalog and signed registry metadata. It retains
existing provenance first, then honors an explicit source, then applies typed
ACL policy and target-compatible source priority. Candidate sources may be a
managed signed artifact, a declared native package-manager identity, a trusted
parent such as Use, or an explicit local package. Registry data cannot inject
installer commands or scripts.

## 10. Registries, Cache, and Self Management

```text
a3s registry list
a3s registry show <name>
a3s registry add <url> --trust-root <file-or-digest> [--yes]
a3s registry remove <name> [--yes]
a3s registry refresh [name]

a3s cache path
a3s cache status
a3s cache prune [--dry-run]
a3s cache clean [--dry-run] [--yes]

a3s self update [--check] [--dry-run] [--yes]
a3s version [--verbose]
a3s completion bash|zsh|fish|powershell|elvish
a3s help [command...]
```

The official registry trust root ships with A3S. Adding a third-party registry
is an explicit trust operation; HTTPS alone is not a trust root. Registry
configuration is ACL. Signed transport metadata and machine receipts may use
versioned JSON because they are generated machine state, not product config.

`cache prune` removes only unreferenced or expired entries. `cache clean`
removes all recreatable cache content, never configuration, receipts, sessions,
documents, browser profiles, or report artifacts.

Self-update preserves installation provenance. A Homebrew-owned CLI delegates
to Homebrew; a managed release updates through verified artifacts and atomic
activation. It never silently migrates provenance.

## 11. Product Proxies

These are explicit registered namespaces:

```text
a3s box <args...>       -> a3s-box
a3s compose <args...>   -> a3s-box compose
a3s up <args...>        -> a3s-box compose up
a3s down <args...>      -> a3s-box compose down
a3s ps <args...>        -> a3s-box compose ps
a3s logs <args...>      -> a3s-box compose logs
a3s bench <args...>     -> a3s-bench
a3s search <args...>    -> a3s-search
a3s use <args...>       -> a3s-use
```

The root resolves an absolute executable, forwards arguments and streams
without a shell, and preserves child status. Box and Use may retain visible,
catalog-authorized first-use installation. Bench and Search require explicit
installation unless their catalog policy changes. `--offline` always disables
first-use network mutation.

Compose and its shortcuts resolve the same registered Box component; they do
not create another executable identity or orchestration implementation. Only
these registered routes may proxy. An unregistered `a3s-foo` on
`PATH` can appear in the external section of `a3s list`, but `a3s foo` never
executes it.

Use owns the Browser and Office domains and externally installed domain
routes. Their native CLI, standard MCP, and Skill surfaces stay native. The
umbrella CLI does not translate them through a universal JSON action API or a
custom JSON-RPC service.

Root `--output` is not silently translated into a child-specific flag. Until a
compatible first-party child explicitly negotiates the versioned CLI context,
non-human root output is rejected for proxies. A child-native output flag placed
after `box`, `bench`, `search`, or `use` is forwarded unchanged.

The Search proxy is only a user entry point. `a3s-search` embeds the typed
`a3s-use-browser` library for rendering; it does not shell out to `a3s use` or
require an A3S Use service.

## 12. Migration and Acceptance

The complete current-to-target command map, security exceptions, release
milestones, verification matrix, and final acceptance criteria live in the
[A3S CLI Migration and Verification Plan](cli-migration-plan.md). Canonical
forms must land before compatible aliases are removed, and ordinary aliases
remain for at least two minor releases.
