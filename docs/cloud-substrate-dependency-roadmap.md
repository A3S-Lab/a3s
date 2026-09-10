# A3S Cloud Substrate Dependency Roadmap

**Status as of 2026-09-10.**

This document is the **monorepo index** of what each A3S component must supply
so A3S Cloud can close its first-principles gates. It does not replace Cloud
gate status or crate-local product roadmaps.

| Document | Role |
| --- | --- |
| [Cloud architecture optimization roadmap](../apps/cloud/docs/architecture-optimization-roadmap.md) | Execution waves, dual-track I0, Cloud-only backlog |
| [Cloud ecosystem project roadmaps](../apps/cloud/docs/project-roadmaps/README.md) | Portfolio missions and local outcome IDs per subproject |
| [Cloud ROADMAP](../apps/cloud/ROADMAP.md) | Named gates and availability claims |
| [Cloud platform gap analysis](../apps/cloud/docs/platform-gap-analysis.md) | Structural vs delivery gaps |
| [compat/cloud-stack.acl](../compat/cloud-stack.acl) | Exact locked component revisions and protocol levels |
| This document | Per-crate obligations and anti-patterns |

Capability claims remain gate-driven. Module presence is not availability.

## 1. First principles (unchanged)

```text
clients → Gateway → product owner → Operations/Flow
  → Executions|Workloads → Fleet → Node Agent → Runtime → Box → payload
Edge desired state → Fleet → Gateway applied state
```

| Concern | Sole authority |
| --- | --- |
| Desired state / tenancy / placement | Cloud (PostgreSQL + Outbox) |
| Durable coordination | Cloud Operations + Flow |
| Unit lifecycle contract | Runtime |
| Node-local execution/build | Box (+ OCI Runtime drivers) |
| Inference serving process | Power as Box-hosted Runtime Service |
| Public request bytes | Gateway only |
| Product config language | ACL (`a3s-acl`) |

**Forbidden everywhere:** second scheduler, Cloud request-byte proxy, omniscient
policy database, inventing Cloud `workers` / billing tokenizer /
`InferenceDeployment` before Power observation delivery, treating historical
`R0`/`N0`/`D0`/`E0` as current Box certification.

## 2. Execution waves

```text
Wave 0  Architecture integrity (Cloud parallel; crates keep contracts stable)
  -> Wave 1  BX0 (Box) then PW0 (Power)
  -> Wave 2  WI → CD0 → H0.3–H0.5 → OBS/COMP
  -> Wave 3  Verticals (AaaS/WaaS/FaaS/Cell/I0-data-plane/WEB0)
```

## 3. Critical-path crate obligations

| Crate | Wave | Cloud gates | Must deliver | Must not |
| --- | --- | --- | --- | --- |
| [Box](../crates/box/ROADMAP.md) | 1 | `BX0` | Sole execution/build provider; MicroVM/TEE + sandbox isolation evidence; clean-host EXIT; Runtime driver re-cert | Docker fallback; product semantics; second node channel |
| [OCI Runtime](../crates/oci-runtime/ROADMAP.md) | 1 | under `BX0` | Spec-complete isolation drivers Box consumes via `a3s-oci-sdk` | Box product types; registry/build ownership |
| [Sandbox](../crates/sandbox/ROADMAP.md) | 1 | `BX0.3` | Host command boundary fail-closed for Box/Code | Softening required isolation |
| [Runtime](../crates/runtime/ROADMAP.md) | 1–2 | `BX0`, `WI`, verticals | Task/Service contract; generation fencing; opaque WI attestation binding; Box pairing | Placement, tenancy, Gateway ingress, product profiles |
| [Power](../crates/power/ROADMAP.md) | 1 | `PW0`, `I0` Track B | ACL Power Service profile; health/inference/recovery; **observation delivery**; enter `cloud-stack.acl` | Scheduler; inventing Cloud workers |
| [Gateway](../crates/gateway/ROADMAP.md) | 1–3 | `H0.2`+, `I0.2b`+, `WEB0` | Snapshot apply; OpenAI dispatch with real workers; static-object target | Bearer store; east-west mesh control; Dashboard |
| [Flow](../crates/flow/ROADMAP.md) | 2–3 | `CD0`, `W0`, `F0` | Durable history/receipts for Operations and Delivery Pipelines | Second workflow engine; Agent transcript authority |
| [ACL](../crates/acl/ROADMAP.md) | 0–2 | `COMP`, all ACL surfaces | Byte-stable digests; schema stability for COMP | TOML/HCL as product config |
| [ORM](../crates/orm/docs/roadmap.md) | 0–2 | `F0`, Wave 0 | Typed PG truth helpers; migration fencing support | Cross-provider XA as product truth |
| [Event](../crates/event/ROADMAP.md) | 0–2 | `F0`, Outbox relay | Transport of committed facts | Desired-state authority |
| [Boot](../crates/boot/ROADMAP.md) | 0–2 | `F0` roles | Process composition for api/worker/relay/agent | Authz/tenancy policy |
| [Observer](../crates/observer/ROADMAP.md) | 2 | `H0.5-OBS*` | Kernel evidence / OTel-correlated signals | Mutating Cloud desired state |
| [Sentry](../crates/sentry/ROADMAP.md) | 2 | `OBS`, `EV0`, `POL*` | Runtime-security judgment + receipts | Owning incidents or desired state |
| [Use](../crates/use/ROADMAP.md) | 3 | `U0` | Signed package apply/observation; Plugin Manager contracts | Scheduler or node channel |
| [Code](../crates/code/ROADMAP.md) | 3 | `A1` | Native Harness under Cloud provider contract | Second Agent lifecycle/scheduler |
| [Lane](../crates/lane/ROADMAP.md) | 2–3 | CD0/pressure | Post-commit admission fairness | Quota truth (Redis/Lane never) |
| [Updater](../crates/updater/ROADMAP.md) | 2 | `CD0` adjacency | Distribution helpers only | Parallel privileged Cloud updater |

## 4. Dual-track inference (`I0`)

| Track | Owners | Rule |
| --- | --- | --- |
| **A — Control plane** | Cloud Identity / Inference / Edge | Continue without inventing Power workers; empty `workers` is correct |
| **B — Data plane** | Box `BX0` + Power `PW0` + Gateway `I0.2b`+ | Only Track B may claim OpenAI inference service available |

## 5. Adjacent (not Wave 1–2 blockers)

| Component | Relation to Cloud |
| --- | --- |
| Memory, Search, Browser, Science, Desktop, OCR, Vec, MoE, … | Agent/Use/local capability sides. Not Cloud PostgreSQL truth, placement, or public ingress. Do not grow Cloud-critical gates here. |

## 6. Lock and verification

- Bump component gitlink, Cargo pin, and `compat/cloud-stack.acl` together.
- Run `just cloud-stack-check` when changing the lock.
- Crate roadmaps below link here; Cloud keeps gate names (`BX0`, `PW0`, …).
- Success = named gate `Verified` with retained evidence, not folder existence.
