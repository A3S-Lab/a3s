# A3S Box v0.6.x Development Plan

## Mission

A3S Box core mission: run any OCI workload in MicroVM isolation with VM-level security boundaries. Application-agnostic infrastructure layer.

v0.6.x goal: solve "trustworthy" and "production-ready" — not more features.

---

## P0 — Production Blockers

### 1. CRI Persistent State Store

**Problem:** CRI server restart orphans running VMs. Kubelet loses track of all sandboxes. Unacceptable in production K8s.

**What to build:**
- Persist `SandboxState` and `ContainerState` to disk (JSON or SQLite) on every state transition
- On CRI server startup, reload state and reconcile with actual running VMs (re-attach or mark as stopped)
- Atomic writes to prevent corruption on crash

**Acceptance criteria:**
- Kill and restart `a3s-box-cri` — all previously running sandboxes are visible to kubelet
- No VM is orphaned after CRI restart
- Tests: CRI restart roundtrip, crash-recovery simulation

**Crate:** `a3s-box-cri`

---

## P1 — Core Differentiator Validation

### 2. TEE Hardware Validation (AMD SEV-SNP)

**Problem:** All TEE code runs only in simulation. "Hardware validation pending" in README means TEE is a claim, not a fact.

**What to do:**
- Provision Azure DCasv5 (or bare-metal EPYC Milan/Genoa)
- Run full flow: `run --tee` → `attest` → `seal` → `unseal` → `inject-secret`
- Verify VCEK → ASK → ARK certificate chain against AMD KDS
- Verify RA-TLS handshake with real SNP report
- Document any gaps found and fix them

**Acceptance criteria:**
- All 7 TEE integration tests pass on real SEV-SNP hardware (not `--allow-simulated`)
- README "Pending Validation" section removed or updated to "Validated on Azure DCasv5"
- VCEK cert chain verification confirmed end-to-end

**Crate:** `a3s-box-runtime` (attestation module)

### 3. Warm Pool CLI/CRI Integration

**Problem:** Warm Pool is implemented as library-only. Pre-booted VM pool has zero user-facing value without integration.

**What to build:**
- CLI: `a3s-box pool start --size N --image alpine:latest` / `pool stop` / `pool status`
- CRI: when kubelet creates a sandbox, check warm pool first before cold-booting
- Config: `warm_pool_size` in `BoxConfig` wired to CRI startup
- Prometheus metric `a3s_box_warm_pool_hits_total` already exists — wire it up

**Acceptance criteria:**
- `a3s-box run` with warm pool enabled shows sub-50ms start (vs 200ms cold)
- CRI sandbox creation uses pool when available, falls back to cold boot
- `pool status` shows size, capacity, hit rate

**Crate:** `a3s-box-cli`, `a3s-box-cri`, `a3s-box-runtime`

---

## P2 — Architectural Completeness

### 4. SafeClaw + Box Integration

**Problem:** Architecture diagram shows SafeClaw running inside Box VM alongside the agent, but there is no defined integration path. The A3S security architecture exists only on paper.

**What to design and build:**
- Define the integration model: compose sidecar vs SDK-level vs Box-native sidecar support
- Recommended: Box-native sidecar — `BoxConfig::sidecar: Option<SidecarConfig>` that auto-injects SafeClaw as a co-process inside the VM
- SafeClaw gets vsock port allocation (e.g., 4092) for host-side control
- Document the data flow: agent traffic → SafeClaw → classified/sanitized → LLM

**Acceptance criteria:**
- `a3s-box run --sidecar safeclaw:latest myagent:latest` starts both processes in the same VM
- SafeClaw intercepts agent's outbound LLM calls
- Integration test: agent + safeclaw in one VM, verify classification runs

**Crates:** `a3s-box-core` (config), `a3s-box-runtime` (sidecar launch), `a3s-box-guest-init` (multi-process PID 1)

### 5. Intel TDX Runtime

**Problem:** TDX config variant exists (`TeeConfig::Tdx`) but runtime is a no-op. Half-implemented TEE support is misleading.

**What to build:**
- Implement TDX TDREPORT generation via `/dev/tdx_guest` ioctl
- TDX quote generation (TD Quote via TDQE)
- Wire into existing attestation flow (same trait, different backend)
- Simulation mode via `A3S_TEE_SIMULATE=1`

**Dependency:** Requires Intel TDX-capable hardware (4th Gen Xeon or Azure DCesv5). Can implement + simulate first, validate on hardware second.

**Acceptance criteria:**
- `a3s-box run --tee --tee-type tdx --tee-simulate` works end-to-end
- TDX attestation path covered by unit tests in simulation mode
- Hardware validation tracked as separate milestone

**Crate:** `a3s-box-runtime`

### 6. Live CPU/Memory Hot-Resize

**Problem:** `container-update` command exists but requires VM restart for resource changes. K8s VPA needs live resize.

**What to build:**
- libkrun hot-resize API for vCPU count and memory balloon
- Wire into `container-update --cpus N --memory Xg` without VM restart
- CRI: handle `UpdateContainerResources` RPC
- Fallback: if libkrun version doesn't support it, return clear error (not silent failure)

**Acceptance criteria:**
- `a3s-box container-update dev --cpus 4` changes vCPU count without restart
- Memory balloon inflate/deflate works
- CRI `UpdateContainerResources` RPC tested

**Crate:** `a3s-box-runtime`, `a3s-box-shim`, `a3s-box-cri`

---

## P3 — Feature Completion

### 7. OCI Image Signing on Push

**Problem:** `pull` verifies signatures, `push` does not sign. Asymmetric.

**What to build:**
- `a3s-box push --sign-key cosign.key myimage:tag`
- Keyless signing via OIDC + Rekor (same as verify path)
- Sign after push, attach signature to registry

**Crate:** `a3s-box-runtime` (OCI module)

### 8. ADD .tar.bz2 / .tar.xz Support

**Problem:** Dockerfile `ADD` auto-extracts `.tar.gz` but not `.tar.bz2` or `.tar.xz`.

**What to build:** Add `bzip2` and `xz` decompression to the ADD handler. Two deps, ~30 lines.

**Crate:** `a3s-box-runtime` (Dockerfile builder)

---

## Not Doing

**Multi-node compose** — Cross-host service discovery is K8s's job. Box doing this violates the "application-agnostic infrastructure" principle and duplicates K8s networking. If needed, use K8s Services + DNS.

---

## Milestone Summary

| Milestone | Items | Target |
|-----------|-------|--------|
| v0.6.0 | CRI persistent state (#1) | Production unblock |
| v0.6.1 | Warm pool integration (#3), TEE hardware validation (#2) | Core differentiator |
| v0.6.2 | SafeClaw integration (#4), TDX runtime (#5) | Architecture completeness |
| v0.6.3 | Live hot-resize (#6), OCI signing on push (#7), ADD bz2/xz (#8) | Polish |

---

## Definition of Done (per item)

- [ ] Tests written before implementation (TDD)
- [ ] `just test` passes
- [ ] README updated (Features + Roadmap)
- [ ] No test artifacts left on filesystem
- [ ] `cargo fmt --all` run before commit
- [ ] Pruning audit done (no dead wrappers, no orphaned exports)
