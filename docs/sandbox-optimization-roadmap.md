# Sandbox Optimization Roadmap (First-Principles Gap Plan)

Status: proposal derived from the 2026-09 competitive gap analysis of
`a3s-sandbox` + `a3s-box` against mainstream agent-harness sandboxes
(Codex CLI, Claude Code sandboxing, Gemini CLI, E2B/Modal/Daytona/Morph).
Companion analysis: see the gap-review conversation notes; this document is
the actionable follow-up. It extends — and does not replace —
[`crates/sandbox/ROADMAP.md`](../crates/sandbox/ROADMAP.md): Gates 1–7 and
their statuses there remain authoritative.

## First principles

The stack exists so agents can execute untrusted code boldly with a bounded
blast radius, without the boundary itself making the agent loop unusable.
Four filters decide what enters this roadmap, in addition to the five in the
crate roadmap:

1. **Blast-radius completeness.** Every axis an untrusted tree can damage —
   filesystem, credentials, network, resources — must be enforced by the OS,
   or explicitly refused at policy compile. Today credentials still land
   inside the boundary, non-HTTP traffic is all-or-nothing on two of three
   platforms, and CPU/PID/disk quotas are unclaimed on two of three
   platforms.
2. **A security posture needs an authorization write-path.** A policy system
   whose only write path is "human edits the ACL file" rots under pressure.
   Deny-all defaults plus structured denial events (Gate 2) must converge
   into a user-approved, typed, persisted grant loop — otherwise the
   fail-closed advantage never ships as product experience.
3. **Recovery primitives change agent behavior.** Agents that can roll back
   take bigger steps. Snapshot/fork already exists in `a3s-box`; the stack
   lacks only the composition.
4. **Secrets before pipes.** Do not widen any network mediation before
   host-held secrets exist: opening egress around plaintext credentials
   multiplies the exact risk the mediator exists to fence.

## What we are NOT chasing (deliberate refusals)

| Refusal | Reason |
| --- | --- |
| TLS interception / MITM as the masking mechanism | Existing non-goal; SNI/CONNECT-level origin allowlists plus egress-side secret injection achieve the security outcome without a private CA |
| Virtual bash / in-process language VMs | Existing non-goal; OS boundary only |
| SRT/TypeScript settings clone | Existing non-goal |
| Hosted multi-tenant sandbox-as-a-service | Belongs to the Cloud substrate (BX-line), not this boundary stack |
| GPU passthrough | Belongs to the `a3s-box` hardware/TEE line; no local-harness competitor ships it either |
| Full Docker/BuildKit parity in one step | Tracked as a feature-parity ledger (H3), not a big-bang rewrite |

## Gate sequence

Gates 8–11 are the P0 wave. Gate 12–13 are P1. Each gate keeps the crate
discipline: falsifiable exit tests first, minimal enforcement second, full
local gates (`fmt`, `clippy -D warnings`, `test --all-targets`) before the
next gate, and negative security tests for every surface expansion.

### Gate 8 — Credential containment (secrets never land) [P0]

**Why first:** filter 4. The strongest pattern in the field (Claude Code
credential deny/mask; E2B/Daytona/Devin egress injection) is "the guest never
holds the secret; the host re-injects at the mediation point." `a3s-sandbox`
today only scrubs the environment and denies writes to protected paths —
once any mediation is off, leak containment depends entirely on path
enumeration. Prior art in-tree: `crates/box/docs/host-held-secrets-spike.md`
and the Linux `runtime-secrets` tmpfs (`secret_environment`), which macOS and
Windows currently parse but refuse to execute.

Scope:

- A host-held secret store contract (typed `SecretRef`, never bytes in
  policy/config/state — follow the box Compose `secret_environment` rule).
- Native sandbox: env values may be `SecretRef`s; the guest receives a
  sentinel; the host materializes the real value only at the mediation point
  for allowlisted egress (Gate 4 CONNECT path first).
- Box: raise `secret_environment` from Linux-only to macOS/Windows execution
  parity, or keep the per-host refusal explicit in `probe`.
- Mask mode for deny-read credential files: guest reads a sentinel; mediator
  re-injects only on allowlisted origins. SigV4 re-signing and JWT claim
  masking are explicitly deferred until a concrete consumer demands them
  (minimal surface).

Exit criteria:

- Falsifiable negative tests: secret bytes never appear in guest-visible
  env/filesystem when `SecretRef`s are used; unlisted egress carries only the
  sentinel; store unavailability fails closed before spawn.
- Per-platform claim matrix updated (`mediated_credential_transform` style
  capability flags); platforms that cannot enforce refuse at compile.

### Gate 9 — Non-HTTP mediation parity [P0]

**Why:** blast-radius completeness for non-HTTP traffic. Today
`mediated_socks` and `unix_socket_allowlist` are true only on macOS
(`THREAT_MODEL.md`); Linux and Windows remain fail-closed, so SSH, database
drivers, and gRPC are all-or-nothing on two of three platforms. The fences
already exist per platform — this gate is generalization, not invention:

- Linux: SOCKS over the proven netns + `a3s-sandbox-relay` TCP→Unix bridge
  (same staging as the claimed HTTP CONNECT path).
- Windows: SOCKS over the inherited named-pipe pattern used by
  `windows_appcontainer_named_pipe_*`.
- Shared origin allowlist decisions (`decide_mediated_socks`) extended with
  the existing alias hardening (no `localhost`↔`127.0.0.1`, hostname↔literal
  IP aliasing).

Exit criteria:

- Live guest allow/deny evidence on all three platforms; denied CONNECT/SOCKS
  never reaches upstream; negative tests for FD/env/network bypass classes.
- Capability matrix: `mediated_socks` 3/3 claimed or honestly refused per
  platform with probe visibility.

### Gate 10 — Approval-to-policy loop [P0]

**Why:** filters 1–2 together. Claim B (mediated-network-as-default) is
blocked on independent review — correctly. But without a runtime grant path,
the product form is "permanently deny" or "user hand-edits ACL", which is the
availability ceiling behind every P0 gap. The reframe: do not default
mediation on; make **typed, user-approved, persisted grants** the write path.

Scope:

- Map structured denial reasons (Gate 2 `AuditEvent`) to authorization
  request objects: subject (domain / unix socket / path / capability),
  session, policy digest.
- Host integration (`a3s-code`): approval surface →
  `replace_policy` with `allow_broadening` only for the granted subject →
  persisted to ACL permissions with digest lineage. The existing
  `sandbox_permissions: require_escalated` bash contract stays for full-host
  escapes; this loop is for boundary-scoped grants.
- Default profile unchanged: deny-all, mediation off until a grant activates
  it for that subject. Claim B remains an independent-review decision; this
  gate makes any future default safe instead of binary.

Exit criteria:

- Every mediated allow traces in the audit log to a user grant + policy
  digest (replayable); no code path broadens without host authorization;
  monotonicity property tests (a grant for `api.example.com:443` cannot
  unlock `.example.com` apex or port neighbors).

### Gate 11 — Resource enforcement depth [P0]

**Why:** blast-radius completeness for availability. Gate 2 delivered
timeout + output ceilings everywhere, Windows Job process/memory quotas, and
Linux `RLIMIT_AS`; fork bombs, compile memory storms, and log floods remain
undefended by OS primitives on Linux (pids/cpu) and macOS (memory), and disk
quotas are unclaimed everywhere.

Scope:

- Linux: cgroup v2 delegation (systemd user-slice delegation) for
  CPU/pids/memory. Probe delegation availability; fail closed where absent —
  no RLIMIT_AS approximation of a pid/cpu quota.
- Windows: extend the Job Object surface (IO/rate caps where the platform
  enforces).
- Disk: per-sandbox scratch caps via platform primitives (managed drive
  quota on Windows; sparse-image scratch on macOS; loop/project quota on
  Linux) — where a platform cannot enforce, refuse at compile, per house
  style.
- macOS memory: remains honestly unsupported (rlimits cannot express it and
  QoS classes are not quotas); visible via `probe`, never approximated.
- Network bandwidth shaping: explicit non-goal for now — the only honest
  locus is the mediator, and value is low vs. Gate 8–10.

Exit criteria:

- Per-platform negative tests enforced by the OS itself: fork-bomb killed by
  pid quota, compile storm bounded by memory quota, log flood bounded by disk
  cap; unclaimable quota requests refuse at policy compile with a probe-
  visible reason.

### Gate 12 — Session composition: snapshot/checkpoint integration [P1]

**Why:** filter 3. All local harnesses (Codex/Claude/Gemini) ship zero
environment snapshot or rollback; E2B/Morph made fork the core agent
primitive. `a3s-box` already owns stopped-filesystem snapshots, COW restore,
warm pools, and opt-in Linux memory snapshot-fork. The stack lacks only the
composition: today `a3s-code` bash always runs through the native boundary
on the real workspace.

Scope:

- `a3s-code` session option: an explicit box-backed execution mode
  (typed option object, per the SDK typed-extension rule) with checkpoint
  every N steps / before destructive commands, and fork-on-failure retry.
- Native mode stays the default (per-command spawn latency wins for the
  common loop); box mode is opt-in for risky exploratory phases.
- Snapshot digests recorded in audit events for replay.

Exit criteria:

- E2E test: destructive command → fork from the pre-command checkpoint →
  deterministic workspace recovery; documented p50 fork latency per platform
  where qualified; no silent fallback to native mode.

### Gate 13 — Convergence hardening [P1]

Independent engineering-debt items, sequenced last because none block the
others:

- **H1 Windows concurrency (G5):** design spike — replace shared-state
  temporary ACL mutation with per-exec ACL scoped by workspace identity so
  same-workspace commands stop serializing. Ship only if exact prior-ACL
  restore correctness is preserved; otherwise keep serialization and document.
- **H2 Capability matrix single source (G10):** generate the per-platform
  capability table from `probe`/`BackendCapabilities` into docs; CI checks it
  cannot drift from claims.
- **H3 Build parity ledger (G8, box):** enumerate Dockerfile/Compose feature
  support (multi-stage, cache mounts, ARG scope, Compose verbs) as a tested
  ledger; grow on demand, publish what is unsupported.
- **H4 Audit export (G11):** opt-in OTel exporter for `AuditEvent`, redaction
  preserving; integration test proves SIEM consumers get denial→grant
  lineages (feeds Gate 10).

Exit criteria: per-item; H2's generated table is CI-enforced.

## Dependency order and waves

```text
Gate 8 (secrets)  ──► Gate 9 (non-HTTP pipes)  ──► Gate 10 (grant loop)
        │                                                  │
        └──────────── Gate 11 (quotas) ────────────────────┤
                                                           ▼
                     Gate 12 (session composition) ═══ Gate 13 (hardening)
```

- Wave 1 (P0): Gates 8–11. 8 before 9/10 (secrets before pipes; grants
  before any default change). 11 is independent and can proceed in parallel.
- Wave 2 (P1): Gates 12–13. 12 depends only on the existing box SDK
  contract; start any time after Wave 1 design freeze.

Rough effort (one experienced engineer, house-style estimates): Gate 8
3–4 weeks; Gate 9 2–3 weeks; Gate 10 2–3 weeks (host integration may
dominate); Gate 11 3–4 weeks (cgroup delegation matrix); Gate 12 2–3 weeks;
Gate 13 per-item 1–2 weeks each.

## Risks and decisions

- **cgroup delegation is not universal** (container hosts, non-systemd).
  Probe + fail closed; never approximate with rlimits.
- **Grant-loop UX lives in hosts, not the crate.** The crate owns request
  objects and monotonic `replace_policy` semantics; `a3s-code` owns prompting.
  Do not pull UI into the sandbox.
- **SecretRef temptation:** no secret bytes in ACL/config/state/labels ever;
  the box Compose rule is the precedent.
- **Claim B is not re-litigated here.** Gate 10 makes a future default safe;
  the independent-review requirement stands regardless.
- **macOS resource honesty:** shipping a fake quota would cost more trust
  than the gap does; keep refusing loudly.

## Acceptance of this roadmap

1. Every gate maps to a named gap in the 2026-09 analysis and a first-
   principles filter above.
2. Every exit criterion is falsifiable by test, in the crate's existing
   negative-security test classes.
3. Existing non-goals survive intact; new refusals are listed, not implied.
4. Gates 1–7 statuses in `crates/sandbox/ROADMAP.md` remain authoritative;
   nothing here re-opens a closed claim.

## Addendum (2026-09-26): Gate 12 design freeze + Gate 13 H1 spike

### Gate 12 — session composition: checkpoint/checkpoint-restore (design freeze)

Box-side primitives are already evidenced: `a3s-box snapshot create/restore`
plus the `real_core_filesystem_image_snapshot_commands` smoke proves
snapshot → restore → deterministic re-creation of guest state on real-core
hosts. The remaining gap is purely composition inside `a3s-code`. Design
decisions frozen before any code:

1. **Typed session option** (no string flags):

   ```rust
   pub enum BashBoundary {
       /// Default: native per-command boundary on the real workspace.
       NativeHost,
       /// Exploratory phases: commands run in a box record with checkpoints.
       BoxIsolated {
           checkpoint: CheckpointPolicy,
       },
   }

   pub enum CheckpointPolicy {
       /// Every N completed commands.
       EveryCommands(u32),
       /// Manual, tool-invoked (`checkpoint` tool).
       Manual,
   }
   ```

2. **Checkpoint mechanics honor box semantics**: box filesystem snapshots
   are taken from *stopped* boxes. A checkpoint is therefore
   stop → snapshot → restart of the session box, and the design must publish
   per-platform p50 cost before claiming the mode. Linux x86_64/KVM may use
   the opt-in memory snapshot-fork instead; other platforms use the
   stop-snapshot-restart path or refuse. No hidden degradation to native.

3. **Fork-on-failure**: a failed/destructive step can restore the latest
   checkpoint into a fresh box record (never in-place), then re-execute.
   Every checkpoint and restore records its snapshot digest in the session
   audit trail.

4. **Exit criteria (falsifiable)**:
   - E2E: destructive command → restore latest checkpoint into a new box →
     byte-identical recovery of the pre-command workspace state.
   - Documented p50 checkpoint cost per platform where the mode claims to
     work; the capability report lists platforms where it does not.
   - Default remains `NativeHost`; switching boundaries is an explicit,
     typed session choice. No silent fallback in either direction.
   - Tests live beside the box smokes (real-core gated) plus a code-side
     session-mode unit suite with a mocked boundary.

5. **Deliberately out**: per-step memory forking on macOS/Windows (box
   rejects it), checkpointing the native workspace (git remains the
   recovery path there), multi-box fan-out (Cloud substrate's territory).

### Gate 13 H1 — Windows same-workspace concurrency (design spike)

Today AppContainer runs serialize per workspace because temporary ACL and
DOS-drive-map mutations are shared process state. Candidate resolutions, in
order of preference:

1. **Per-exec AppContainer profile**: derive the container SID from a
   per-execution nonce so the ACL grant and drive map become per-execution
   state; keep the exact prior-ACL restore contract by snapshotting only the
   paths this execution touched. Spike must prove ACL-restore correctness
   under concurrent same-workspace executes before any claim.
2. **Privileged broker**: a scoped service owns ACL/drive mutations;
   executions pass capability handles. Heavier; only if (1) fails the
   restore-correctness bar.
3. Keep serialization and document it (status quo; still compliant).

Exit: either a Windows CI gate proving concurrent same-workspace executes
with exact ACL restoration, or a written refusal with the spike's evidence.
