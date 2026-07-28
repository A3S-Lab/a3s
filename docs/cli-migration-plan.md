# A3S CLI Migration and Verification Plan

- Status: In progress
- Date: 2026-07-15
- Parents: [Product Design](cli-product-design.md) and
  [Technical Architecture](cli-technical-architecture.md)

## 0. Implementation Baseline

The migration is deliberately incremental. The following table distinguishes
landed contracts from target architecture so documentation does not overstate
platform or security coverage.

| Area | Current state | Remaining acceptance work |
| --- | --- | --- |
| Parser and taxonomy | Typed Clap root and Code trees, generated help, command-path help, and five shell completions are implemented. Unknown root commands do not execute arbitrary binaries. | Finish the complete compatibility disposition and deterministic help snapshots. |
| Output and exits | Common JSON success/error envelopes, structured parser/context errors, exit classes `1/2/3/130`, and leaf command identifiers are implemented. Code asset, KB, Context, and Memory reads now return typed JSON; Code Exec, Research, Web logs, and Top provide terminal JSONL streams where applicable. | Collect structured deprecation warnings and finish broken-pipe and signal conformance on every platform. |
| Invocation context | The root boundary captures environment and terminal facts once; canonicalizes `-C`; resolves relative config, storage, memory, and Code asset paths from that directory; derives output, interaction, network, and progress policy; owns one invocation cancellation token and Ctrl-C listener; and passes explicit policy to modern handlers. Root dispatch, canonical non-interactive handlers, and the interactive Code TUI no longer change the process directory. One `CodeRuntimeConfiguration` now resolves the effective ACL plus TUI asset and memory paths once per launch. | Move remaining Top internals off legacy global helpers; add a diagnostic sink; inject the fully resolved config and platform-path provider directly into the context. |
| ACL configuration | Explicit-file resolution, user/workspace overlays, collection merge-by-identity, provenance, redaction, and typed environment overrides are implemented through `a3s-acl`. Relative `--config` and `A3S_CONFIG_FILE` paths resolve from the effective `-C` directory. | Finish comment-preserving mutation coverage and stop the remaining legacy TUI panels from rediscovering configuration through process globals. |
| Web and Top | Managed Web lifecycle and structured Top snapshots are implemented with contract tests. Code Exec, Top machine streams, and followed Web logs consume the root cancellation token; cancelled machine streams emit a terminal event and exit `130`. | Complete foreground Web and proxy signal convergence, cross-platform cancellation, stale-process, rotation, and long-running stream tests. |
| Components | Typed catalog discovery, receipts, list/info/install/upgrade/uninstall/doctor, dry-run, offline preflight, partial results, registry configuration, and cache ownership boundaries are implemented as an MVP. | Add immutable plan digests, transaction journal/recovery, signed TUF metadata, open validated source IDs, real migration, native-manager adapters, and declared Windows artifacts. |
| Code application layer | Exec, sessions, research, assets, knowledge, context, memory, and TUI launch receive the effective workspace/config explicitly. Clap asset commands map directly to family-specific typed requests that preserve native paths. Application-owned asset and Research runtimes now own typed asset outcomes plus DeepResearch workflow source, budgets, prompts, evidence normalization, permissions, report validation, orchestration, and synthesis without importing TUI internals; machine mode suppresses desktop opening. Interactive asset, config, memory, OKF, and Skill discovery paths are injected from the same invocation. | Move the remaining interactive panel operations onto shared application services and converge TUI, Exec, Web, and Research session construction. |
| Proxy boundary | Box, Bench, Search, and Use are explicit registered proxies with raw native arguments, child status preservation, an explicit child working directory, compatibility policy variables, and versioned `A3S_CLI_*` context. | Complete signal forwarding, non-UTF-8/Windows-wide argument, and first-party child output-context conformance tests. |
| Documentation cutover | Product, architecture, Use, component, and cross-platform designs exist. | Update the docs application and README examples only as each implementation gate passes; remove stale aliases after the documented window. |

### Execution Order

1. **P0 — Contract convergence:** finish one parse-dispatch-render boundary,
   typed Code read/mutation outcomes, structured deprecation warnings,
   cancellation, and stdout/stderr/broken-pipe conformance.
2. **P0 — Invocation context:** replace handler reads of mutable process-global
   policy with immutable workspace, ACL, output, interaction, network, and
   cancellation objects.
3. **P1 — Transactional components:** implement plan digests, apply-equivalent
   dry-runs, journals, rollback/recovery, and explicit outcome-unknown states.
4. **P1 — Registry trust:** consume signed TUF catalog metadata, validate open
   `SourceId` values, and prohibit registry-supplied installer commands.
5. **P2 — Supported platform backends:** finish managed artifacts and
   conformance for macOS and Linux targets; treat WSL as Linux.
6. **P3 — Windows promotion:** add native artifacts, locking, process,
   recovery, and Browser persistent-session conformance before advertising
   Windows runtime support.
7. **P4 — Cutover:** generate/check documentation from parser metadata,
   publish compatibility telemetry limited to deprecation IDs, then retire
   aliases only after their release window.

No later priority can be used to claim earlier acceptance. In particular,
parsing `--source`, `--channel`, or `--scope` does not mean a backend supports
that combination; capability resolution must prove it before mutation.

### Invocation Context Checkpoint

The current P0 checkpoint has removed root-wide `set_current_dir`, `set_var`,
and `remove_var` propagation. Modern root commands receive one immutable
invocation value containing:

- a canonical effective directory and an explicit config path resolved from
  that directory;
- a captured environment snapshot rather than repeated command-level policy
  parsing;
- output, verbosity, color, terminal, interaction, progress, offline, and
  first-use-install policy;
- one invocation-owned cancellation token driven by one root Ctrl-C listener;
- platform-native component data, state, and cache roots; and
- a child-process adapter that sets the proxy working directory and emits
  `A3S_CLI_CONTEXT_VERSION=1` with versioned output, offline, interaction, and
  progress fields.

Machine output is non-interactive. Human invocations with unsafe non-TTY input
or diagnostic streams are also non-interactive, and non-TTY diagnostics disable
progress. Components receive paths, offline state, and progress explicitly;
the root does not set `A3S_OFFLINE` or `A3S_NO_PROGRESS` for in-process calls.

The checkpoint is covered by built-binary tests for relative `--config`,
relative `A3S_DATA_HOME`/`A3S_STATE_HOME`/`A3S_CACHE_HOME`, relative ACL and
environment-backed Agent/MCP/Skill/Flow roots, workspace OKF discovery,
native non-UTF-8 asset argv where the platform permits it,
sequential context construction, zero-network offline install, native proxy
argv/status/context, Code session/KB/Context/Memory paths, effective
model/config resolution, and detached Web startup with `-C` plus an explicit
relative ACL file. A focused in-process regression also resolves all TUI launch
paths while the process current directory points somewhere else and verifies
that the process directory is unchanged. Built-binary cancellation tests cover
Code Exec, Top JSONL monitoring, and followed Web logs, including sequenced
terminal events and exit `130`.

This does **not** complete the target context architecture. The interactive TUI
no longer enters a current-directory compatibility guard: it stores the
resolved workspace, ACL, asset, memory, OKF, and Skill paths supplied by the
invocation. Some TUI authentication paths still export session environment
variables, and some non-TUI compatibility callers still use legacy path
convenience functions. Legacy public component convenience functions remain
environment-driven for compatibility, although root dispatch uses their
explicit `_with` entry points. Foreground Web serving and proxy children have
not yet converged on the invocation cancellation token or signal-forwarding
contract. The context still lacks the target diagnostic sink, injected
effective config, and fully injectable platform-path environment. These are P0
work, not completed claims.

The DeepResearch application runtime now lives under `commands/code`, and its
workflow source, prompt/budget policy, evidence normalization, report artifact
validation, permission gate, executor, and tests are split by concern. The TUI
imports that application layer for interactive presentation; the Research
runtime does not import TUI internals.

## 1. Migration Rules

- New canonical forms land before compatible old forms are removed.
- An ordinary alias remains for at least two minor releases. After 1.0,
  removal waits for a major release unless security requires earlier action.
- Human mode prints one warning to stderr with the exact replacement.
- JSON adds a structured warning to its result and never mixes prose into
  stdout. Proxies never receive warnings on their stdout.
- Primary help, examples, and completion show canonical forms only. Migration
  help lists deprecated forms.
- An alias preserves its old semantics and exit status while it exists.
- Positional credentials are rejected immediately instead of receiving a
  compatibility window.
- Fuzzy fallbacks that reinterpret unknown commands as queries are removed
  after the compatibility window.

Compatibility is data-driven where possible:

```rust
struct Deprecation {
    id: &'static str,
    old_display: &'static str,
    replacement: &'static str,
    introduced: Version,
    remove_after: RemovalWindow,
    severity: DeprecationSeverity,
}
```

Normalization records only the deprecation ID, never argument values. Tests
prove that both spellings create the same canonical request. The normalizer is
deleted entry by entry and must not become a second permanent parser.

## 2. Current-to-Target Disposition

| Current form | Canonical form | Disposition |
| --- | --- | --- |
| `a3s code`, `code resume [id]` | Same | Canonical and retained |
| `a3s code <prompt-or-unknown-word>` TUI fallthrough | `a3s code exec <prompt>` or a valid subcommand | Accidental fallthrough removed |
| supported `code <family> <verb>` lifecycle forms | Same, except Agent kind grammar | Canonical and retained |
| `a3s box ...`, `a3s use ...` | Same | Registered proxies retained |
| `a3s list`, `install`, `uninstall`, `top` | Same command names | Canonical and extended |
| `a3s update` | `a3s self update` | Deprecated alias |
| `a3s update <id>...` | `a3s upgrade <id>...` | Deprecated alias |
| `a3s update --all` | `a3s upgrade --all` | Deprecated alias |
| `a3s code update` | `a3s self update` | Deprecated alias |
| `a3s web [flags]` | `a3s web start [flags]` | No-verb human shortcut retained |
| `a3s web -d` | `a3s web start --detach` | `-d` deprecated |
| Web `-w`, `--workspace` | global `-C`, `--directory` | Deprecated aliases |
| removed `a3s code serve` | `a3s web start` | Existing hard error retained |
| `a3s code login` | `a3s auth login os` | Deprecated alias |
| `a3s code login <token>` | `a3s auth login os --token-stdin` | Positional secret rejected immediately |
| `a3s code logout` | `a3s auth logout os` | Deprecated alias |
| `a3s code auth ...` | `a3s auth ...` | Deprecated alias |
| bare `a3s code auth` | `a3s auth status os` | Deprecated implicit verb |
| `a3s code models`, `model [list]` | `a3s model list` | Deprecated aliases |
| bare `a3s code config` | `a3s config path` | Deprecated implicit verb |
| `a3s code config path` | `a3s config path` | Deprecated alias |
| `code config init [path]` | `[--config <path>] config init [--scope ...]` | Positional path deprecated |
| `a3s code config cat` | `a3s config show` | Deprecated alias |
| `a3s code config check` | `a3s config validate` | Deprecated alias |
| `a3s code config dirs`, `code dirs` | `a3s config paths` | Deprecated aliases |
| `a3s code deepresearch`, `deep-research` | `a3s code research` | Deprecated aliases |
| research `--local`, `--os`, `--runtime` | `--local-only`, `--web` | Runtime selection removed; evidence scope remains explicit |
| `<family> local`, `<family> ls` | `<family> list --location local` | Deprecated aliases |
| `<family> list [query]` | `<family> list --location os [query]` | Bare form deprecated as ambiguous |
| Agent `publish|run|open|logs|status <kind> [path]` | `<action> [path] --kind <kind>` | Deprecated positional grammar |
| `a3s code kb home` | `a3s code kb stats` | Deprecated alias |
| bare `a3s code kb` | `a3s code kb stats` | Deprecated implicit verb |
| `a3s code kb vault`, `kb dir` | `a3s code kb path` | Deprecated aliases |
| `a3s code ctx ...` | `a3s code context ...` | Deprecated alias |
| `ctx show <event>` | `context show event <event>` | Deprecated alias |
| `ctx session <id>` | `context show session <id>` | Deprecated alias |
| bare `ctx <words>` fallback | `context search <query>` | Fuzzy fallback removed after grace period |
| `a3s code mem ...` | `a3s code memory ...` | Deprecated alias |
| bare `a3s code memory` | `a3s code memory list` | Deprecated implicit verb |
| `memory dir` | `memory path` | Deprecated alias |
| bare `memory <query>` fallback | `memory list <query>` | Fuzzy fallback removed after grace period |
| `a3s code top` | `a3s top` | Deprecated alias |
| top tab flags such as `--agents` | `a3s top --view agents` | Deprecated aliases |
| positional `top <container>` | `top --container <container>` | Deprecated positional shorthand |
| `top --watch <duration>`, `--interval <duration>` | `top --watch --interval <duration>` | Deprecated implicit/combined grammar |
| `--active-only`, `--compact-columns` | `--active`, `--compact` | Deprecated aliases |
| `top -h` opening the TUI help panel | `top --help`; press `h` inside the TUI for its panel | Deprecated nonstandard short-help behavior |
| `top -v` printing a version | `a3s version`; `-v` becomes global verbosity | Deprecated conflicting short flag |
| `--json` | `--output json` | Stable shorthand |
| `a3s --version` | `a3s version` | Stable shorthand |
| `a3s help`, `--help`, nested help | Same generated help surface | Canonical and retained |

`a3s code resume`, all supported asset lifecycle verbs, and the explicit Box
and Use proxy namespaces remain supported. Bench and Search gain explicit
registered proxy namespaces; this does not restore arbitrary external-command
dispatch.

The README currently documents `a3s code view`, but the CLI does not route
that command. It receives no compatibility alias. Remove the stale example at
documentation cutover. Explicit browser work uses `a3s use browser open`, and
OS lifecycle commands continue to open views returned by their operations.

## 3. Delivery Milestones

### Milestone 1: Characterize

- inventory all root and Code-owned commands, flags, output, and exit behavior;
- snapshot current help and parser errors;
- add built-binary fixtures for proxies and component commands;
- record accidental behavior that will intentionally not be preserved.

Exit condition: every current public form has a test and a disposition in this
document.

### Milestone 2: Establish the CLI Spine

- add Clap root types, `InvocationContext`, `CliError`, and central renderers;
- adapt existing handlers without changing their core behavior;
- remove deep `process::exit` calls from root-owned paths;
- generate help and shell completion from the parser.

Exit condition: manual root routing is gone and the existing compatibility
suite passes through typed requests.

### Milestone 3: Add Canonical Administration

- add `self update`, `upgrade`, and top-level auth, model, and config groups;
- install compatibility rewrites and structured warnings;
- reject positional tokens with a redacted safe replacement;
- introduce the global output, TTY, offline, and interaction policies.

Exit condition: the overloaded `update` path is no longer canonical and every
root-owned read command has stable JSON.

### Milestone 4: Normalize Services and Code

- implement Web start, stop, status, logs, and open with safe process identity;
- add `code exec`, session inspection, and `code research`;
- require explicit asset location and normalize Agent kind options;
- normalize Top view and watch options;
- add registered Bench and Search proxies.

Exit condition: all new service and Code commands pass human, JSON, non-TTY,
and cancellation tests.

### Milestone 5: Complete Component Management

- implement component info, safe no-argument upgrade, and doctor;
- add registry and cache groups;
- complete dry-run, offline planning, partial outcomes, and plan digests;
- deliver declared macOS, Linux, and Windows source backends;
- align first-party child CLIs with the invocation/output context.

Exit condition: component lifecycle conformance passes for every advertised
target and unsupported targets fail before download.

### Milestone 6: Cut Over and Retire

- switch README, docs site, examples, and completions to canonical forms;
- validate documentation command tables against parser metadata;
- if existing opt-in telemetry is enabled, measure only deprecation IDs, never
  arguments, paths, prompts, or secrets;
- remove aliases only after their documented release window.

Each milestone leaves the CLI releasable. Parser replacement is not coupled to
finishing every platform backend or new domain in one release.

## 4. Verification Matrix

### 4.1 Parser and Help

- every canonical command, option, conflict, operand, and enum value;
- complete alias table and identical canonical requests;
- typo suggestions without automatic execution;
- stable help snapshots at a deterministic terminal width;
- Bash, Zsh, Fish, PowerShell, and Elvish completion generation;
- no unknown-command, TUI-launch, or fuzzy-query fallthrough.

### 4.2 Output and Interaction

- golden JSON for success, error, warning, and partial success;
- JSONL sequence numbers and required terminal event;
- stdout/stderr separation and broken-pipe behavior;
- human TTY, non-TTY, `NO_COLOR`, quiet, verbose, and progress behavior;
- no prompts, color, or progress in JSON and JSONL;
- missing consent in non-interactive mode returns an actionable error.

### 4.3 Security and Mutation

- no credential material in argv, diagnostics, logs, snapshots, receipts, or
  machine output;
- offline mode performs zero network requests;
- dry-run performs zero mutations and produces the apply-equivalent plan;
- plan-digest mismatch, interrupted journal, rollback, and outcome-unknown;
- receipt ownership, archive traversal, stale link, and path-boundary checks;
- stale Web PID records never signal an unrelated process.

### 4.4 Proxy and Platform

- raw non-UTF-8 or native wide arguments where supported;
- stdin/stdout/stderr inheritance, child directory, context, exit code,
  cancellation, and signal forwarding;
- missing, broken, external, system, and managed component states;
- first-use allowed, disabled, offline, and incompatible-version behavior;
- macOS, declared Linux libc/distribution targets, native Windows, and WSL;
- native-manager adapters use isolated fixtures rather than the developer's
  live package database in unit tests.

Built-binary integration tests set `A3S_NO_AUTO_INSTALL=1`, isolated
data/state/cache homes, and explicit temporary workspaces. They never download
software or mutate the developer environment unless a separately gated real
integration test explicitly opts in.

## 5. Final Acceptance Criteria

- Every public root command is a typed parser node with generated help.
- Every current form is unchanged, migrated, deprecated, or intentionally
  removed in tests and this plan.
- Unknown commands exit with code 2 and never execute arbitrary binaries.
- Root-owned reads support stable JSON; supported streams use stable JSONL.
- Human, machine, TTY, non-TTY, color, offline, and non-interactive contracts
  are covered by integration tests.
- A3S ACL is parsed and generated with `a3s-acl`; no HCL parser is introduced.
- CLI JSON remains a process contract, not JSON-RPC; MCP and Skills retain
  their native standards.
- Proxy tests prove raw argument and child-status preservation.
- Component mutations prove planning, ownership, dry-run, failure, recovery,
  and partial success on advertised operating systems.

## 6. Industry References

- uv CLI reference: <https://docs.astral.sh/uv/reference/cli/>
- GitHub CLI manual: <https://cli.github.com/manual/gh>
- GitHub CLI formatting: <https://cli.github.com/manual/gh_help_formatting>
- GitHub CLI exit codes: <https://cli.github.com/manual/gh_help_exit-codes>
- Docker CLI reference: <https://docs.docker.com/reference/cli/docker/>
- kubectl conventions: <https://kubernetes.io/docs/reference/kubectl/conventions/>
- Winget documentation: <https://learn.microsoft.com/windows/package-manager/winget/>
- Command Line Interface Guidelines: <https://clig.dev/>
