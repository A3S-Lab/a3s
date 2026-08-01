# A3S Simulator Product and Development Plan

Status: Proposed

Planning baseline: 2026-08-01

Decision horizon: local alpha through fleet beta

Companion document: [Technical Architecture](a3s-simulator-technical-architecture.md)

## 1. Executive summary

A3S Simulator is a proposed GUI-first Visual Environment Engine for coding
agents. It will let an agent provision, observe, operate, checkpoint, and
release graphical Linux, Windows, macOS, Android, and iOS environments through
one programmable contract.

The first release is deliberately narrower than that vision. It proves one
vertical journey on a real Linux/KVM host:

1. build an application in A3S Box;
2. boot a clean Linux desktop with the QEMU provider;
3. transfer the artifact through a governed guest bridge;
4. wait for graphical readiness;
5. observe pixels and optional semantic state;
6. perform a fenced input action;
7. collect screenshots, logs, and action receipts;
8. restore a checkpoint; and
9. release the lease without leaving host resources behind.

Windows, mobile, macOS, and fleet operation advance only after this contract is
measured on real hosts. With the team and hardware assumptions in this plan,
the current planning ranges are:

| Outcome | Indicative elapsed time |
| --- | ---: |
| Linux GUI local alpha | 11 weeks |
| Linux and Windows desktop preview | 19 weeks |
| Desktop and mobile preview | 31 weeks |
| Hardened local beta | 37 weeks |
| Fleet beta | 49 weeks |

These are planning ranges, not release promises. Milestone M0 must replace
them with evidence from technical spikes, legal review, and host-capacity
measurements.

## 2. Product charter

### 2.1 Product statement

For coding agents that must build and verify software in a real graphical
operating system, A3S Simulator provides a typed, recoverable environment
lifecycle instead of a collection of hypervisor scripts and coordinate-only
automation commands.

### 2.2 Primary users

- coding-agent authors who need reproducible GUI application validation;
- agent-platform engineers who need one lifecycle across multiple providers;
- CI and release engineers who need screenshots, recordings, logs, and replay
  evidence tied to an exact environment;
- mobile teams that need isolated Android and iOS simulator leases; and
- A3S operators who need local-first operation today and schedulable nodes
  later.

The first-cycle buyer and operator is a platform or developer-tools team. The
first-cycle end user is a coding agent acting under an explicit host policy.

### 2.3 Jobs to be done

1. Give an agent a clean graphical machine without teaching it provider CLI
   syntax.
2. Move a locally built artifact into that machine without exposing arbitrary
   host paths.
3. Tell the agent when the desktop and target application are actually ready.
4. Provide pixels, display metadata, and optional accessibility state under one
   observation schema.
5. Apply input exactly once or return an honest dispatch state.
6. Restore a known checkpoint when exploration damages the environment.
7. Produce a self-contained evidence bundle for review or evaluation.
8. Reclaim every process, disk overlay, port, socket, and input lease.

### 2.4 Goals

- one stable resource model across desktop VMs and mobile simulators;
- GUI readiness as a first-class condition, not a boot-process heuristic;
- a raw visual contract that works without semantic automation;
- optional A3S CUA adapters for accessibility-aware computer use;
- local-first lifecycle ownership with an upgrade path to remote nodes;
- immutable planning and generation-fenced mutations;
- checkpoint, pool, and evidence semantics with provider-specific capability
  disclosure;
- artifact and service integration with A3S Box through public contracts; and
- CLI, JSON, Rust SDK, and coding-agent Skill surfaces generated from the same
  domain model.

### 2.5 Non-goals for the first product cycle

- replacing A3S Box as the Linux OCI execution product;
- promising one snapshot format across unrelated hypervisors;
- shipping Apple, Microsoft, Google, or third-party proprietary system images;
- claiming macOS support on non-Apple hardware;
- hiding installation, licensing, acceleration, or hardware requirements;
- making screen-coordinate actions deterministic across arbitrary UI changes;
- exposing an unauthenticated remote desktop service;
- implementing a stable dynamic Rust plugin ABI; or
- treating every parsed configuration as a supported environment.

## 3. Position in the A3S product system

Simulator is a separate product boundary. Existing A3S components remain the
owners of their current responsibilities.

| Component | Ownership in a Simulator workflow |
| --- | --- |
| A3S CLI | discovery, installation policy, configuration, authentication, and command routing |
| A3S Code | agent session, permissions, tool routing, and user interaction |
| A3S Simulator | graphical environment planning, lifecycle, visual sessions, guest bridge, checkpoints, pools, and evidence capture |
| A3S Box | Linux OCI builds, auxiliary services, artifacts, volumes, networks, and isolated non-GUI execution |
| A3S CUA | semantic observation, target selection, and higher-level computer-use policy |
| A3S Test / Bench | scenario ownership, judging, evaluation identity, and result admission |
| A3S GUI / WebView | optional native viewer and authenticated remote presentation |
| A3S Cloud | future node enrollment, placement, desired state, and durable remote operations |
| A3S Runtime | reusable finite Task and long-running Service contracts where they fit; it does not become the interactive VM state machine |

The critical integration rule is that Box and Simulator compose; neither parses
the other's CLI output or reaches into the other's runtime state.

~~~text
A3S Box build       A3S Simulator environment      A3S Test evidence
OCI/artifact  --->  boot · install · launch  --->  observe · act · judge
      |                       |
      +---- service link -----+
~~~

## 4. Product principles

### 4.1 Truth before convenience

Every command returns requested state, effective state, provider, accelerator,
image identity, generation, and capability facts. A fallback must appear in the
immutable plan before allocation; it cannot happen silently during boot.

### 4.2 Pixels are the baseline

A screenshot and display geometry are the minimum portable observation. An
accessibility tree, OCR result, DOM, or platform automation handle is optional
and must carry freshness and source metadata.

### 4.3 Lifecycle is leased

Every mutable resource has an owner, lease, deadline, and cleanup path. The
daemon is the sole local lifecycle owner. A disconnected client cannot leave an
unbounded input controller or immortal VM.

### 4.4 Recovery is designed, not improvised

Lifecycle operations are replay-safe. GUI actions return receipts that
distinguish `not_dispatched`, `dispatched`, and `confirmed`; an unknown action
is never blindly repeated.

### 4.5 Support is evidence-backed

Build success, host detection, or a parsed template does not constitute support.
Every supported cell has a named real-host qualification scenario and an
evidence-retention policy.

## 5. Experience contract

The following CLI illustrates the intended workflow. It is a proposed contract,
not a released command surface.

~~~bash
# Inspect the host before selecting a template.
a3s simulator doctor --json
a3s simulator template plan linux-desktop --json

# Build once in Box and pass an immutable artifact reference to Simulator.
a3s box build -t local/example:dev .
a3s simulator create linux-desktop \
  --artifact oci://local/example:dev \
  --name example-ui \
  --wait gui-ready \
  --json

# Observe, act, collect evidence, and recover.
a3s simulator observe example-ui --screenshot out/current.png --json
a3s simulator act example-ui --click 640,420 --receipt out/click.json
a3s simulator evidence collect example-ui --output out/evidence.tar.zst
a3s simulator checkpoint restore example-ui clean-installed
a3s simulator delete example-ui --wait
~~~

The Skill should route an agent through the same safe sequence:

1. run `doctor` and inspect capabilities;
2. plan without allocation;
3. request a bounded lease;
4. wait for an explicit readiness condition;
5. observe before every action;
6. prefer semantic targets when a fresh adapter is available;
7. preserve action receipts and evidence;
8. checkpoint before destructive exploration; and
9. release the environment in a finalizer.

Skill guidance must not invent provider support, download unapproved images,
disable confirmation policy, or retry an action with an unknown dispatch state.

## 6. Scope and support ladder

Each platform progresses through the same states:

`research -> spike -> experimental -> preview -> supported`

| Guest surface | Initial provider | First useful scope | First-cycle ceiling |
| --- | --- | --- | --- |
| Linux desktop | QEMU with KVM/HVF/WHPX where qualified | app install, launch, pixels, input, logs, checkpoint | Target: supported on enrolled Linux/KVM hosts |
| Windows desktop | QEMU with WHPX/KVM where qualified | installer, app launch, UI validation, evidence | Preview |
| Android | Android Emulator and ADB | APK install, Activity launch, screenshots, input, UIAutomator adapter | Preview |
| iOS | CoreSimulator through `xcrun simctl` | app install, launch, screenshots, input/automation adapter, pooled devices | Preview on enrolled macOS hosts |
| macOS | Apple Virtualization.framework helper on Apple Silicon | VM lifecycle, artifact transfer, pixels, guarded input | Local beta after legal and real-host gates |
| Hackintosh | QEMU/OpenCore research only | feasibility and compatibility learning | Unsupported lab path; never advertised or distributed |

Tart informs the Apple provider's use of Virtualization.framework, automation,
and OCI-backed VM distribution. It is not the stable A3S provider API. SimSlim
informs per-device iOS profiles, feature-aware preflight, safe cleanup, and pool
density; Simulator must retain a stock-compatible path and verify required
features before leasing a slimmed device.

## 7. Workstreams

| Workstream | Accountable outcome |
| --- | --- |
| Product and protocol | charter, scenarios, resource schemas, compatibility policy, CLI and Skill behavior |
| Engine | planner, state machines, persistence, leases, reconciliation, API, SDK |
| Desktop providers | QEMU first; Apple Virtualization.framework after M0 |
| Mobile providers | Android Emulator, ADB, CoreSimulator, and iOS density profiles |
| Visual and guest I/O | frames, input, clipboard, file transfer, readiness, semantic adapters |
| Box integration | immutable artifact export/import and service links through public SDKs |
| Images and checkpoints | manifests, acquisition policy, overlays, compatibility fences, pools |
| Security and compliance | authentication, secrets, networks, proprietary media, platform licensing |
| Quality and operations | real-host lab, golden scenarios, evidence bundles, leak and soak testing |
| Developer experience | installer, `doctor`, diagnostics, documentation, Skill, examples |

## 8. Team and operating model

The baseline assumes eight to ten dedicated people:

- one product lead with developer-tools experience;
- one staff architect or technical lead;
- two engine and protocol engineers;
- two desktop virtualization engineers;
- one mobile automation engineer, growing to two for M3;
- one security/reliability engineer; and
- shared design, technical writing, legal, release, and hardware-lab support.

Reducing below six dedicated engineers changes the plan from a parallel
multi-platform program to a serial Linux-first program. macOS and iOS require
dedicated Apple hardware, signing administration, and a named operator. Windows
requires dedicated WHPX qualification hosts. Hosted CI compilation is not a
substitute for these machines.

Operating cadence:

- weekly provider and golden-scenario review;
- biweekly product demo from a clean host image;
- monthly security, licensing, and support-matrix review;
- an ADR before every irreversible public contract; and
- no milestone promotion without a retained evidence bundle.

## 9. Milestone plan

### M0 — contract and feasibility gate, weeks 1-3

Deliver:

- approve product ownership and non-goals;
- freeze resource vocabulary and draft protocol v0;
- implement pure state machines and a fake provider;
- spike QEMU frame/input/QMP behavior on Linux/KVM;
- spike Apple Virtualization.framework frame and input feasibility;
- spike Android and iOS automation under interactive sessions;
- validate Box artifact export and guest import without shared internal state;
- complete image, OS-license, Tart-license, and Hackintosh policy review; and
- price the required real-host lab.

Exit only when the fake-provider golden scenario passes, provider capability
gaps are documented, cleanup is demonstrably bounded, and staffing plus host
capacity are funded.

### M1 — Linux GUI local alpha, weeks 4-11

Deliver:

- local daemon, SQLite state, lease manager, reconciliation, and event stream;
- immutable planning and `doctor` capability report;
- QEMU/KVM provider with QMP lifecycle, graphical readiness, screenshots,
  absolute pointer, keyboard, serial log, guest bridge, and qcow2 overlays;
- Box-built artifact transfer and loopback service link;
- checkpoint create/restore with compatibility fences;
- CLI, JSON output, Rust SDK preview, and first-party coding-agent Skill; and
- install, launch, observe, act, evidence, restore, and cleanup golden scenario.

Gate: 100 consecutive scenario runs on enrolled Linux/KVM hosts, zero leaked
resources, 99 percent boot-to-GUI success, and no unresolved critical security
finding.

### M2 — Windows desktop preview, weeks 12-19

Deliver:

- Windows image preparation runbook with bring-your-own-media policy;
- QEMU/WHPX or qualified alternative provider path;
- signed guest bridge installation, reboot-aware readiness, installer workflow,
  and Windows accessibility adapter spike;
- host accelerator and nested-virtualization diagnostics; and
- Windows application install, launch, interaction, evidence, and cleanup
  scenario.

Gate: 50 consecutive runs per supported host class, documented licensing, and
no silent KVM/WHPX/TCG fallback.

### M3 — Android and iOS preview, weeks 20-31

Deliver:

- Android Emulator/ADB provider with UIAutomator semantics;
- CoreSimulator provider with `simctl` lifecycle and automation bridge;
- per-device pool sanitation and feature-aware slim profiles;
- app install, deep-link, permission, rotation, keyboard, screenshot, and log
  flows; and
- mobile evidence schemas compatible with desktop scenarios.

Gate: deterministic pool reset, capability preflight for every optimized iOS
profile, and 200 lease/release cycles without cross-tenant state.

### M4 — macOS and hardened local beta, weeks 32-37

Deliver:

- independently implemented Apple Virtualization.framework helper;
- Apple Silicon macOS VM lifecycle on approved hardware and media;
- local authentication, signed packages, migration tooling, quotas, and audit;
- pool fairness, pressure eviction, and crash-recovery soak; and
- compatibility policy for daemon, CLI, SDK, templates, and guest bridge.

Gate: four-week real-host soak across the supported matrix, restore rehearsal,
and legal approval of every advertised path. Hackintosh remains outside the
support matrix regardless of technical feasibility.

### M5 — fleet beta, weeks 38-49

Deliver:

- enrolled Simulator nodes with signed capability inventory;
- A3S Cloud placement adapter, durable operations, drain, and maintenance mode;
- authenticated visual relay with explicit controller leases;
- content-addressed image and artifact distribution; and
- fleet quotas, scheduling fairness, evidence upload, and operator dashboards.

Gate: loss-of-node recovery, rolling upgrade, scheduler reconciliation, and
multi-host capacity tests all pass without weakening local engine semantics.

## 10. Golden scenarios and verification

### 10.1 Required golden scenarios

1. Linux desktop plus Box artifact: build, boot, install, launch, interact,
   collect evidence, restore, and delete.
2. Windows desktop: install an MSI or unpackaged build, complete a guarded GUI
   interaction, and prove cleanup after reboot.
3. Android: install an APK, launch an Activity, handle a permission prompt, use
   a semantic target, and reset the device.
4. iOS: lease a compatible device, verify required services, install and launch
   an app, interact, and sanitize the lease.
5. Recovery: kill the client, daemon, and provider helper at each lifecycle
   phase and converge to the documented state.
6. Fencing: prove a stale generation and expired input controller cannot mutate
   a replacement instance.

### 10.2 Test layers

- pure domain and state-machine model tests;
- fake-provider contract tests and fault injection;
- provider conformance tests;
- golden image and guest-bridge compatibility tests;
- real-host functional, soak, leak, and pressure tests;
- security tests for authentication, secrets, network policy, path handling,
  clipboard, and artifact integrity; and
- CLI, SDK, Skill, upgrade, downgrade, and migration compatibility tests.

A capability is complete only when its schema, cancellation behavior,
timeouts, error taxonomy, audit fields, cleanup path, documentation, and
real-host evidence are all present.

## 11. Metrics

### Product

- time from a coding-agent request to a GUI-ready lease;
- percentage of sessions completed without human provider intervention;
- first-attempt success for the golden workflow;
- useful evidence bundles per failed run;
- checkpoint recovery success; and
- agent actions completed with semantic targets versus raw coordinates.

### Reliability and efficiency

- boot-to-GUI p50, p95, and p99 by provider and template;
- frame age and input acknowledgement latency;
- guest-bridge readiness and transfer success;
- leaked-resource count after normal, cancelled, and crash paths;
- warm-pool hit rate and sanitation time;
- host memory, disk, and accelerator pressure per active lease; and
- unknown action-dispatch outcomes per 1,000 actions.

Metrics must be segmented by host class, provider version, guest image digest,
template revision, and guest-bridge version. Aggregates without these dimensions
must not drive support decisions.

## 12. Principal risks and mitigations

| Risk | Mitigation and decision trigger |
| --- | --- |
| Scope explosion across five OS families | Linux vertical slice first; every additional cell needs a provider owner and golden scenario |
| GUI automation flakiness | pixels as baseline, freshness metadata, semantic adapters, explicit readiness, receipts, and evidence |
| Proprietary OS and image restrictions | bring-your-own-media, content-addressed local manifests, legal review, no redistribution by default |
| macOS-on-non-Apple ambiguity | lab-only research; never a marketed, packaged, or fleet-scheduled path |
| Provider capability drift | immutable plans, version probes, compatibility fences, and real-host qualification |
| Hidden coupling to Box | public SDK/artifact contracts and independent lifecycle state |
| Unsafe input or remote viewing | local-only default, authenticated relay, controller lease, generation fencing, and audit |
| Pool cross-tenant contamination | disposable overlays, identity reset, feature preflight, sanitation evidence, and periodic destructive rebuild |
| Fork maintenance burden | keep A3S QEMU/Tart forks thin; prefer upstream protocols and record every patch delta |
| Schedule optimism | M0 measured spikes, explicit confidence ranges, and scope reduction before staffing fiction |

## 13. Go/no-go policy

Proceed from M0 only when:

- raw frame and input paths work on the selected first provider;
- lifecycle replay and cleanup pass through the fake provider;
- Box integration works through a stable public boundary;
- Apple and iOS spikes define honest support limits;
- proprietary media and license policy has an accountable owner;
- real-host capacity and code-signing administration are funded; and
- the revised estimate fits the approved team.

Pause or reduce scope when:

- a claimed path requires private OS APIs;
- interactive semantic automation cannot be reproduced in a GUI session;
- provider checkpoints cannot be compatibility-fenced;
- management or visual endpoints cannot remain private by default;
- image governance cannot be enforced; or
- cleanup repeatedly leaves resources after a milestone gate.

## 14. Immediate next actions

1. Approve the product boundary and M0 exit gate.
2. Assign an accountable owner to every workstream.
3. Reserve Linux/KVM, Windows/WHPX, and Apple Silicon hosts.
4. Complete legal review of system media, Tart, iOS runtimes, Windows images,
   and the unsupported Hackintosh research path.
5. Accept the architecture decisions listed in the companion
   [Technical Architecture](a3s-simulator-technical-architecture.md).
6. Create `A3S-Lab/Simulator` only after M0 scope approval; do not add a Rust
   crate directly to this integration repository.
7. Implement the domain model and fake provider before production providers.
8. Run the four M0 spikes and replace ranges with measured work packages.
9. Freeze protocol v0 only after the fake-provider golden scenario passes.
10. Start the QEMU Linux slice and Box artifact bridge together so the alpha
    proves the intended agent workflow rather than an isolated hypervisor demo.

## 15. References

- [A3S Simulator Technical Architecture](a3s-simulator-technical-architecture.md)
- [A3S Box](https://github.com/A3S-Lab/Box)
- [A3S GUI](https://github.com/A3S-Lab/GUI)
- [A3S Cloud architecture](https://github.com/A3S-Lab/Cloud/blob/main/docs/architecture.md)
- [A3S QEMU fork](https://github.com/A3S-Lab/qemu)
- [A3S Tart fork](https://github.com/A3S-Lab/tart)
- [SimSlim](https://github.com/MobAI-App/simslim)
- [OSX-KVM](https://github.com/kholia/OSX-KVM)
