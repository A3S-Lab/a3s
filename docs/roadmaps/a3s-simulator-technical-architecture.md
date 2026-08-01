# A3S Simulator Technical Architecture

Status: Proposed

Architecture baseline: 2026-08-01

Product plan: [Product and Development Plan](a3s-simulator-development-plan.md)

Initial target: local Linux GUI alpha

## 1. Architecture decisions at a glance

1. Simulator is a standalone Visual Environment Engine, not an A3S Box
   backend and not a generic extension of Runtime Task or Service.
2. A local daemon is the sole lifecycle owner; CLI, SDK, Skill, and viewers are
   clients.
3. Planning is immutable. Provider, accelerator, resources, image identities,
   ports, and fallbacks are resolved before allocation.
4. A provider-neutral domain model wraps provider-native machines, devices,
   frames, input, checkpoints, and logs without pretending their capabilities
   are identical.
5. Raw pixels and display geometry are mandatory. Accessibility, OCR, DOM, and
   native automation are optional, freshness-scoped semantic adapters.
6. Lifecycle commands support exact replay. GUI actions return dispatch-state
   receipts and are not blindly retried.
7. Every mutation is fenced by instance generation and, where applicable, an
   input-controller lease.
8. A guest bridge owns readiness, artifact transfer, process launch, and guest
   telemetry. It is versioned independently from the host daemon.
9. Box integration uses public SDK and artifact contracts; Simulator never
   parses Box CLI output or copies Box runtime state.
10. Checkpoints remain provider-native and carry strict compatibility fences.
11. First-party providers compile into the daemon or use checked helpers. No
   stable dynamic Rust plugin ABI is promised in v1.
12. macOS uses public Apple Virtualization.framework APIs on approved Apple
   hardware. Hackintosh is an unsupported lab-only research path.
13. Fleet scheduling reuses the local engine contract; it does not replace it
   with a separate remote-only lifecycle.

## 2. System context

Simulator sits between an agent-facing control surface and heterogeneous
graphical compute providers.

~~~text
 A3S Code / coding agent / CI / operator
                 |
        CLI · Skill · Rust SDK
                 |
       local authenticated API
                 |
 +---------------------------------------------+
 | Simulator daemon                            |
 | planner · engine · leases · reconciler      |
 | visual broker · guest bridge · evidence     |
 | image store · checkpoints · pools · audit   |
 +---------------------------------------------+
        |              |              |
  desktop VM       mobile device   optional adapters
  providers         providers       CUA · Test · Cloud
        |              |
 QEMU / Apple VZ   Android / iOS
        |
  guest bridge + graphical session
        |
  artifacts and services from A3S Box
~~~

### 2.1 Dependency direction

- domain and protocol types depend on no provider implementation;
- the engine depends on provider, store, clock, ID, and policy interfaces;
- providers depend on platform SDKs or checked helper protocols;
- CLI and SDK depend on the public client protocol, not engine internals;
- Skill instructions depend on stable CLI and JSON schemas;
- Box, CUA, Test, GUI, and Cloud integrations live behind explicit adapters;
- provider-specific facts can flow upward as typed capabilities and evidence,
  never as implicit behavior.

### 2.2 Architectural qualities

Priority order for the first cycle:

1. recoverability and cleanup;
2. truthful capability reporting;
3. isolation and host safety;
4. graphical correctness and evidence;
5. reproducibility;
6. agent usability;
7. startup latency and density; and
8. breadth of platform support.

## 3. Deployment shapes

### 3.1 Local development

One user-scoped daemon owns all Simulator processes and local state. Its API is
bound to a user-private Unix socket or Windows named pipe. The CLI launches or
discovers the daemon, performs a version handshake, and submits idempotency keys
for mutations.

Provider helpers run as daemon children with bounded privileges and inherited
lease identity. Direct QEMU, `simctl`, `adb`, or Virtualization.framework
processes launched by clients are outside the supported contract.

### 3.2 Local viewer

The graphical viewer is a replaceable client. It subscribes to frames and
events, requests a time-bounded controller lease, and sends input with the
instance generation plus current frame identity. Closing the viewer releases
control but does not necessarily terminate the environment.

The first implementation may use a minimal native window. A3S GUI or WebView
can host the mature surface after the raw frame/input contract is stable.

### 3.3 Fleet

A future node runs the same daemon and providers. A3S Cloud supplies desired
state and placement; the node remains authoritative for local leases, process
ownership, reconciliation, and evidence staging. Remote visual traffic uses an
authenticated relay and never opens a provider's VNC or management endpoint
directly to the network.

## 4. Core domain model

Every persisted resource has a stable ID, tenant or local-user identity,
creation time, schema version, requested specification, effective facts, and
generation.

### 4.1 Template

A `Template` is declarative input to planning. It includes:

- guest family, architecture, version selector, and graphical profile;
- image references and acquisition policy;
- CPU, memory, disk, display, audio, GPU, and accelerator preferences;
- network, clipboard, host-folder, USB, and secret policy;
- guest-bridge requirement and minimum version;
- readiness conditions;
- checkpoint, pool, and evidence policy;
- allowed providers and explicit fallbacks; and
- metadata and compatibility revision.

A template may parse successfully while remaining unschedulable on the current
host. Validation and planning are separate operations.

### 4.2 Plan

A `Plan` is the immutable result of resolving a template against one host
capability inventory. It records:

- normalized template digest;
- selected provider and exact provider version;
- effective accelerator and emulated machine/device model;
- resolved CPU, memory, disk, display, and networking;
- immutable base image and mutable overlay strategy;
- allocated-but-not-yet-bound port and socket intents;
- guest-bridge compatibility decision;
- every accepted fallback and warning;
- capability claims required for execution; and
- plan expiry and host inventory digest.

Allocation refuses an expired plan or a changed inventory unless the caller
requests a new plan. There is no in-boot fallback from acceleration to slow
emulation.

### 4.3 Instance

An `Instance` is a generation-fenced realization of a plan. It owns:

- provider-native identity and process group;
- lease owner and deadline;
- lifecycle phase, conditions, and last transition reason;
- overlay disks and ephemeral directories;
- bound sockets, ports, networks, and device handles;
- current visual session and controller lease;
- guest-bridge session and boot identity;
- checkpoint ancestry;
- evidence stream; and
- cleanup journal.

Names are mutable aliases. Automation uses stable instance IDs and generations.

### 4.4 Visual session

A `VisualSession` is independent of process lifecycle. It describes:

- display IDs, logical and physical dimensions, scale, rotation, and color
  format;
- latest frame ID, timestamp, damage region, and encoding;
- viewer subscriptions and backpressure state;
- controller lease, expiry, and input capabilities;
- cursor and optional clipboard policy; and
- semantic adapter availability and freshness.

An instance can be running without a ready visual session, and a visual session
can be reconnecting while the guest remains healthy.

### 4.5 Observation

An `Observation` is immutable evidence with:

- instance ID and generation;
- display and frame identity;
- host monotonic timestamp and optional guest timestamp;
- image reference or inline bounded payload;
- geometry, scale, rotation, cursor, and occlusion facts;
- optional semantic snapshot references;
- capture source and freshness; and
- redaction policy and content digest.

The action API accepts an optional observation precondition. A stale-frame
failure is preferable to clicking a materially changed screen.

### 4.6 Action receipt

Every action returns a receipt containing:

- action ID and caller idempotency key;
- instance ID, generation, and controller lease ID;
- source frame and target description;
- normalized input events;
- dispatch state: `not_dispatched`, `dispatched`, `confirmed`, or `unknown`;
- provider acknowledgement and timestamps;
- post-action observation when requested; and
- error and retry classification.

Only `not_dispatched` is automatically retryable with the same action identity.
An `unknown` outcome requires observation and agent-level recovery.

### 4.7 Checkpoint, pool, artifact, and link

- `Checkpoint` is provider-native state plus a compatibility manifest and
  immutable ancestry.
- `Pool` is a set of sanitized, compatible leases governed by warm capacity,
  fairness, pressure, and rebuild policy.
- `Artifact` is content-addressed application or test input with provenance,
  media type, integrity, and retention.
- `ServiceLink` is a lease-scoped route from a guest to an explicitly exported
  Box or host service, with no ambient host-network access.

## 5. Lifecycle and reconciliation

### 5.1 Instance phase

~~~text
planned -> allocating -> starting -> booting -> guest-ready -> gui-ready
   |           |            |          |            |            |
   +-----------+------------+----------+------------+------------+
                              |                     |
                           failed               stopping
                                                      |
                                                   stopped
                                                      |
                                                  deleting
                                                      |
                                                   deleted
~~~

Checkpoint restore may create a new generation or a replacement instance. It
never mutates identity in a way that lets an old controller continue acting.

Conditions are orthogonal to phase:

- `ProviderReady`;
- `GuestBooted`;
- `BridgeReady`;
- `GuiReady`;
- `SemanticReady`;
- `NetworkReady`;
- `Degraded`; and
- `EvidenceComplete`.

### 5.2 Exact replay

Mutating lifecycle requests carry an idempotency key. The daemon persists the
intent before provider allocation and returns the original outcome when the
same identity is replayed. Provider calls receive stable operation IDs where
possible.

The cleanup journal records every acquired resource before the next acquisition.
Reconciliation walks the journal in reverse, then inventories provider-native
resources by owner labels to catch work completed after a crash but before the
database commit.

### 5.3 Startup recovery

On daemon startup:

1. lock the state directory and verify schema compatibility;
2. mark in-flight operations for reconciliation;
3. inventory provider processes and native resources;
4. match them by persisted owner and generation;
5. reattach only when identity and plan digest agree;
6. fence visual controllers and guest sessions from the prior daemon epoch;
7. resume bounded cleanup or restore the requested state; and
8. emit a recovery report before accepting new allocations.

Unknown resources are quarantined for operator review or deleted only under an
explicit, narrowly scoped orphan policy.

## 6. Public surfaces

### 6.1 Local protocol

The daemon exposes a versioned RPC or HTTP-over-local-socket protocol with:

- host `doctor` and capability inventory;
- template validate and plan;
- instance create, inspect, list, stop, start, and delete;
- readiness waits and event streams;
- observe and bounded frame subscription;
- acquire/release controller and dispatch action;
- guest file, process, log, and readiness operations;
- checkpoint and pool operations;
- artifact and service-link operations; and
- evidence collection and audit queries.

Errors use a stable taxonomy: `invalid`, `unsupported`, `conflict`, `stale`,
`unauthorized`, `unavailable`, `deadline`, `integrity`, `provider`, and
`internal`. Provider text is diagnostic detail, not the machine-readable code.

### 6.2 CLI and JSON

Human output is concise and diagnostic. `--json` returns versioned schemas with
no ANSI formatting. Scripts use IDs, digests, and conditions rather than
scraping tables. Destructive actions require explicit identity and respect the
active A3S permission policy.

### 6.3 SDK and Skill

The Rust SDK is the reference typed client. Python, TypeScript, and Go can follow
after the local protocol stabilizes. The coding-agent Skill contains workflow
and recovery guidance; it does not bypass daemon policy or implement a second
orchestrator in shell commands.

## 7. Provider contract

The provider interface is asynchronous, cancellation-aware, and capability
driven. Conceptually it supports:

~~~text
probe(host) -> CapabilityInventory
plan(template, host) -> ProviderPlan
allocate(operation, plan) -> NativeResource
start(operation, instance) -> StartReceipt
inspect(instance) -> ProviderObservation
stop(operation, instance, mode) -> StopReceipt
delete(operation, instance) -> DeleteReceipt

open_visual(instance) -> VisualEndpoint
capture(instance, display) -> Frame
dispatch(instance, lease, action) -> DispatchReceipt

guest_connect(instance) -> GuestSession
checkpoint(instance, request) -> NativeCheckpoint
restore(checkpoint, request) -> NativeResource
inventory(owner_scope) -> NativeResource[]
~~~

Capability fields include host/guest architectures, acceleration, display and
input types, checkpoint modes, guest transport, networking, semantic adapters,
and known incompatibilities. An absent capability is not inferred from provider
name.

### 7.1 QEMU provider

The first provider wraps an A3S-qualified QEMU build while preferring upstream
stable protocols:

- QMP for lifecycle, device state, pause/resume, and migration/checkpoint
  primitives;
- QEMU Guest Agent only for explicitly supported guest operations;
- virtio-serial or vsock for the A3S guest bridge where the host supports it;
- SPICE, VNC, display listeners, or a purpose-built frame path selected during
  M0 based on latency, input correctness, and packaging;
- absolute pointer and scancode/keycode normalization;
- qcow2 backing chains for disposable overlays; and
- KVM, HVF, and WHPX only when the exact host path passes qualification.

The A3S QEMU fork should remain a thin, traceable patch set. Every downstream
patch needs an owner, upstream status, security update path, and conformance
test. The daemon must not rely on undocumented monitor text.

### 7.2 Apple Virtualization provider

An independently implemented Swift helper uses public
Virtualization.framework APIs on Apple Silicon. It owns VM configuration,
graphics, input, storage attachment, and platform restore-image handling, and
speaks a versioned private helper protocol to the daemon.

Tart is an architectural reference for Virtualization.framework performance,
automation, and OCI-backed VM distribution. Simulator does not expose Tart's
CLI as its provider contract and must complete an independent license and patch
review before reusing code.

### 7.3 Android provider

The Android provider composes the official Emulator, ADB, and UIAutomator:

- AVD and system-image identity are immutable plan inputs;
- emulator gRPC or supported console control manages lifecycle facts;
- ADB handles app install, Activity launch, logs, port routing, and files;
- screenshots and input have a raw provider path;
- UIAutomator supplies optional semantic observations; and
- quick-boot state is compatibility-fenced by emulator, image, device, and
  configuration versions.

### 7.4 iOS provider

The iOS provider runs only on enrolled macOS hosts with Xcode runtimes:

- `xcrun simctl` owns device creation, boot, install, launch, media, logs, and
  erase/delete flows;
- a checked automation helper provides input and semantic queries;
- runtime, device type, Xcode build, and host version form the compatibility
  identity; and
- devices are cloned or rebuilt from controlled baselines and sanitized between
  leases.

SimSlim informs optional density profiles. A profile is per-device, declared,
and feature-aware. `doctor` verifies capabilities such as push, StoreKit, or
universal links before a lease. Stock devices remain available. Destructive disk
cleanup is separate, allowlisted, confirmed, and followed by requalification.

### 7.5 Hackintosh research adapter

QEMU/OpenCore projects demonstrate useful boot, disk, networking, and headless
patterns, but also expose acceleration, device, maintenance, and licensing
uncertainty. Any experiment is isolated to a research adapter with no image
distribution, no support claim, no production scheduler placement, and no path
to silently satisfy a `macos` template. Supported macOS means approved Apple
hardware and the Apple provider.

## 8. Visual and semantic I/O

### 8.1 Frame pipeline

Providers emit native frames into a bounded pipeline. The visual broker:

- assigns monotonically increasing frame IDs per display generation;
- normalizes geometry, scale, rotation, pixel format, and color space;
- keeps a bounded last-frame cache and drops intermediate frames under pressure;
- supports full frames first and damage regions only after correctness tests;
- records frame age and transport latency; and
- persists only frames requested by evidence policy.

Backpressure favors fresh observations over lossless video. Recording is a
separate evidence sink and cannot block interactive control.

### 8.2 Input pipeline

Input is normalized to pointer movement, button, wheel, key, text, touch,
multi-touch, and platform command intents. Each action is checked against:

- caller authorization;
- instance generation;
- controller lease and expiry;
- display generation and optional source frame;
- coordinate bounds and rotation;
- platform capability; and
- confirmation policy for sensitive combinations or clipboard transfer.

The provider acknowledgement determines the action receipt dispatch state. A
post-action screenshot can confirm visible change but is not universal proof of
the intended semantic outcome.

### 8.3 Semantic adapters

Semantic observations use a common envelope around provider-specific nodes:

- stable only within one semantic snapshot;
- role, label, value, state, bounds, actions, source, and confidence;
- explicit relationship to display generation and source frame;
- adapter version and capture timestamp; and
- a stale-after deadline.

A3S CUA chooses between semantic and visual strategies. Simulator owns raw
capture and dispatch, not high-level task reasoning.

## 9. Guest bridge

The guest bridge is a small, signed, versioned service with least privilege. It
provides:

- boot and GUI readiness probes;
- bounded file upload/download with content digests;
- process launch, cancellation, exit, stdout, and stderr;
- application install adapters;
- structured guest logs and health;
- screen/session identity facts; and
- optional clipboard and service-link setup under policy.

The host authenticates the bridge with per-instance ephemeral material delivered
through a provider-specific secure channel. The bridge does not accept ambient
LAN connections. Every request carries instance generation, deadline, and
operation identity.

Version negotiation occurs during planning. A missing or incompatible bridge
can be allowed only for a template whose requested operations do not require it.

## 10. A3S Box integration

Box remains the execution and build plane for Linux OCI workloads. Simulator
adds the graphical environment plane.

### 10.1 Artifact bridge

1. Code requests a build through the Box public SDK.
2. Box returns an immutable OCI digest or exported content-addressed artifact.
3. Simulator records provenance and imports only the requested artifact.
4. The guest bridge verifies the digest before install or launch.
5. Evidence links the Box source identity, Simulator plan, guest install result,
   and application process.

Simulator does not interpret Box internal disk snapshots as VM checkpoints.
OCI can transport artifacts and VM bundles, but that does not make a VM disk a
Box container image.

### 10.2 Service links

A service running in Box may be exported to one Simulator lease through a
generation-fenced link. The link specifies protocol, source service identity,
guest-visible address, allowed direction, deadline, and audit identity. It is
removed before the instance lease is considered released.

### 10.3 Image preparation

Box can run reproducible tools that prepare unattended configuration, guest
packages, or metadata. Provider-native disk assembly and proprietary installer
media remain under Simulator image policy. Build isolation does not grant a
right to redistribute the resulting OS image.

## 11. Images, checkpoints, and pools

### 11.1 Image manifest

Every base image manifest includes:

- guest family, version, edition, architecture, and locale;
- content digest and source provenance;
- acquisition method and redistribution policy;
- required provider, machine/device model, and accelerator facts;
- firmware and bootloader identity;
- guest-bridge version and provisioning revision;
- last security update and expiration policy; and
- supported checkpoint and pool modes.

Secrets, product keys, Apple personalization material, and signing credentials
are references to a secret store, never manifest values.

### 11.2 Checkpoints

Checkpoint compatibility is the conjunction of provider, provider version,
host architecture, accelerator, guest image digest, machine/device model,
firmware, resource topology, guest-bridge version, and template compatibility
revision.

Disk-only checkpoints are preferred for portability. Memory checkpoints are
explicitly provider- and host-fenced. Restore creates a fresh runtime generation
and invalidates visual, semantic, guest, port, and controller leases.

### 11.3 Pools

Pool members have states `preparing`, `ready`, `leased`, `sanitizing`,
`quarantined`, and `rebuilding`. Sanitation includes identity reset, app/data
removal, secret rotation, clipboard clearing, network teardown, evidence close,
feature preflight, and a new generation.

Warm pools are an optimization. A cold, disposable path must remain available
and is the correctness reference. Pressure eviction never takes an active lease
unless its policy and deadline permit termination.

## 12. Networking and host integration

Local default networking is outbound NAT with no inbound host or LAN exposure.
Templates explicitly request:

- offline;
- outbound-only;
- allowlisted destinations;
- lease-scoped service links;
- published loopback ports; or
- a named isolated network.

Port assignment occurs in the plan as intent and at allocation as a fenced
binding. The daemon records both. Provider management, QMP, ADB, bridge, VNC,
and visual endpoints stay on private sockets or loopback with authentication.

Host-folder sharing, clipboard, USB, camera, microphone, location, and GPU are
separate capabilities with explicit policy. None is inherited merely because a
provider supports it.

## 13. Security model

### 13.1 Trust boundaries

- the host daemon and its state store are trusted local control-plane code;
- provider processes and helpers are constrained but privileged relative to
  guests;
- guest OS, applications, artifacts, accessibility data, and clipboard are
  untrusted;
- model and agent output is untrusted input to policy checks;
- remote nodes and relays require mutual authentication and enrollment; and
- images and checkpoints require provenance and integrity verification.

### 13.2 Required controls

- user-private local endpoint and peer-credential verification;
- least-privilege helpers and bounded provider command construction;
- no shell interpolation of template values;
- generation fencing on every mutation;
- short-lived controller, guest, service-link, and remote-view leases;
- content-addressed artifact verification;
- encrypted secrets with per-instance delivery and revocation;
- clipboard, file, device, and network policy enforced by the daemon;
- redaction hooks before screenshots or semantic trees enter evidence;
- audit records for planning, allocation, control, artifact, checkpoint, and
  cleanup operations; and
- quotas for CPU, memory, disk, frame bandwidth, pool size, and evidence.

Proprietary images are bring-your-own-media by default. The image store records
policy but never claims that possession implies a redistribution or deployment
right.

## 14. Persistence, events, and evidence

The local daemon uses SQLite in WAL mode for desired state, effective facts,
operations, leases, conditions, cleanup journals, and audit indexes. Large
frames, disks, artifacts, checkpoints, and recordings live in a
content-addressed store referenced by digest.

Every state change appends a typed event with daemon epoch, resource ID,
generation, operation ID, monotonic sequence, wall time, actor, and reason.
Events support bounded replay for clients; the database remains the source of
truth after compaction.

An evidence bundle contains:

- scenario and actor identity;
- template and immutable plan;
- host capability digest and provider versions;
- base image, artifact, checkpoint, and guest-bridge identities;
- lifecycle transitions and readiness evidence;
- selected frames and semantic snapshots;
- action receipts;
- guest application logs and provider diagnostics;
- cleanup result; and
- a signed or hashed manifest.

Sensitive values are excluded or redacted before packaging. Evidence retention
is a policy decision, not an unconditional recording default.

## 15. Failure handling

| Failure | Required behavior |
| --- | --- |
| Client disconnect | lease policy decides whether work continues; controller lease expires quickly |
| Daemon crash | startup recovery fences the old epoch, inventories providers, and resumes reconciliation |
| Provider helper crash | mark degraded, capture diagnostics, restart only when operation semantics allow it |
| Guest bridge loss | keep VM phase separate, retry bounded handshake, expose `BridgeReady=false` |
| Visual transport loss | preserve guest, rotate display generation, require new controller lease |
| Host pressure | stop new allocations, evict eligible warm members, then enforce lease policy |
| Disk full | stop evidence growth, protect state commits, fail new allocation, and preserve cleanup capacity |
| Checkpoint incompatibility | refuse before mutation and explain every mismatched fence |
| Unknown action dispatch | observe and return control to the agent; never automatic replay |
| Orphaned native resource | quarantine or clean only when owner identity and configured policy permit it |

Fault injection must cover every row at every lifecycle transition.

## 16. Repository and packaging plan

After M0 approval, create a standalone `A3S-Lab/Simulator` repository. Do not add
a new crate directly to the A3S integration root. A proposed workspace is:

~~~text
crates/
  simulator-domain/       pure types, plans, state machines, errors
  simulator-protocol/     versioned wire schemas
  simulator-engine/       operations, leases, reconciliation, policy
  simulator-store/        SQLite and content-addressed metadata
  simulator-visual/       frames, input, sessions, evidence
  simulator-guest/        host-side guest bridge client
  simulator-provider-qemu/
  simulator-provider-android/
  simulator-provider-ios/
  simulator-provider-apple/
  simulator-sdk/          typed local client
apps/
  simulator-daemon/
  simulator-cli/
  simulator-viewer/
helpers/
  apple-vz/
  guest-bridge/
integrations/
  a3s-box/
  a3s-cua/
  a3s-test/
  a3s-cloud/
  skills/a3s-simulator/
~~~

The first release artifacts are the daemon, CLI, guest bridge for supported
guests, provider helpers, Skill, checksums/signatures, and platform-specific
installer metadata. Image media is not bundled.

Protocol compatibility uses explicit major/minor negotiation. Persisted schemas
have forward migrations and tested rollback boundaries. Daemon and helper
versions are reported in every plan and evidence bundle.

## 17. Implementation order

1. Pure domain types, state-machine model tests, error taxonomy, and fake clock.
2. Fake provider with controllable delays, partial failure, crash, and orphan
   inventory.
3. SQLite operation journal, leases, daemon epoch, and reconciliation.
4. Local authenticated protocol, CLI `doctor/plan/create/inspect/delete`, and
   event waits.
5. QEMU Linux lifecycle through QMP with disposable overlays.
6. Raw screenshot and input with controller leases and receipts.
7. Guest bridge readiness, file transfer, process launch, and logs.
8. Box artifact adapter and one lease-scoped service link.
9. Evidence bundle, checkpoint, recovery, leak, and soak tests.
10. First-party Skill after the vertical contract and JSON schemas stabilize.
11. Windows provider qualification, then Android and iOS, then Apple macOS.
12. Pools only after cold-path sanitation is proven; fleet only after local
    recovery and compatibility are stable.

## 18. Conformance and release evidence

Every provider implements one conformance suite:

- capability inventory is stable and honest;
- planning has no side effects;
- allocation labels all native resources with recoverable ownership;
- cancellation at every boundary converges to a known state;
- repeated lifecycle operations return the original result or a stable conflict;
- frame geometry and input normalization pass reference fixtures;
- checkpoint fences reject incompatible restores;
- guest and visual session loss do not corrupt instance phase;
- stale generations and controllers cannot mutate resources;
- inventory discovers resources created immediately before a simulated crash;
- cleanup leaves the provider inventory at its pre-test baseline; and
- evidence contains enough identity to reproduce or explain the run.

Real-host release jobs are named by host class and may be conditionally armed,
but a skipped job is never presented as passing evidence. Each support-matrix
cell links to its last successful bundle and expiry date.

## 19. ADR queue

The following ADRs are required before implementation expands beyond M0:

1. product boundary and lifecycle owner;
2. local protocol and version negotiation;
3. immutable planning and capability vocabulary;
4. instance, visual-session, and controller state separation;
5. exact replay and GUI action receipts;
6. QEMU frame/input transport;
7. guest bridge transport and trust model;
8. image manifest and proprietary-media policy;
9. checkpoint compatibility fences;
10. Box artifact and service-link contracts;
11. Apple helper architecture and Tart reuse boundary;
12. iOS density profile and sanitation policy;
13. semantic adapter envelope;
14. evidence bundle and redaction;
15. provider packaging and downstream-fork maintenance; and
16. local-to-fleet scheduling boundary.

## 20. References

- [A3S Simulator Product and Development Plan](a3s-simulator-development-plan.md)
- [A3S Box local runtime](https://github.com/A3S-Lab/Box)
- [A3S GUI native runtime](https://github.com/A3S-Lab/GUI)
- [A3S Cloud architecture](https://github.com/A3S-Lab/Cloud/blob/main/docs/architecture.md)
- [A3S QEMU fork](https://github.com/A3S-Lab/qemu)
- [QEMU Machine Protocol](https://www.qemu.org/docs/master/interop/qmp-spec.html)
- [QEMU QMP command reference](https://www.qemu.org/docs/master/interop/qemu-qmp-ref.html)
- [QEMU Guest Agent](https://www.qemu.org/docs/master/interop/qemu-ga-ref.html)
- [A3S Tart fork](https://github.com/A3S-Lab/tart)
- [Apple Virtualization framework](https://developer.apple.com/documentation/virtualization)
- [Android Emulator command line](https://developer.android.com/studio/run/emulator-commandline)
- [Android UI Automator](https://developer.android.com/training/testing/other-components/ui-automator)
- [SimSlim](https://github.com/MobAI-App/simslim)
- [OSX-KVM](https://github.com/kholia/OSX-KVM)
