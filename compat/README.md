# Cloud Stack Compatibility Lock

`cloud-stack.acl` is the machine-readable compatibility boundary for the A3S
Cloud integration gate. It pins every participating submodule to a full commit,
records exact Cargo package versions, and names the protocol levels exercised
by the gate. The root-owned Updater crate is pinned by its exact package version
and by the root commit that contains it.

Components whose package manifest is below the repository root declare a
component-relative `manifest` path. Workspace-inherited package versions are
resolved from the nearest enclosing Rust workspace manifest.

The lock is parsed and regenerated with the checked-in `a3s-acl` Node SDK.
`node scripts/verify-cloud-stack.mjs` rejects non-canonical ACL, unknown fields,
unsafe or duplicate paths, missing gitlinks, unexpected submodule URLs,
revision drift, dirty component worktrees, Cargo manifest or lockfile drift,
and mismatched Cloud, Form, Box Runtime, A3S Use, or Gateway dependencies. The
verifier also requires Cloud's consumed Form interaction and submitted-value
evaluation fixtures to be byte-identical to their Form-owned conformance
fixtures. It parses and generates all tracked Cloud product-configuration
fixtures and rejects HCL/Terraform product configuration in the Cloud
integration surface.

The Use entry pins the repository once while the verifier derives the exact
`a3s-use-core` and `a3s-use-extension` package versions from that immutable
revision. The lock records every protocol-level-4 `PluginHostManager` schema
consumed by Cloud. It does not create another plugin manager or authorize
assignment mutation before the shared A3S Use manager saga is complete.

## Proposing An Update

1. Update the component in its own repository and obtain an immutable release
   or full commit revision.
2. Update the root submodule gitlink and the corresponding `component` block.
   Keep component and protocol blocks sorted by label, and keep attributes in
   the order produced by `a3s-acl`.
3. Update exact dependency declarations and Cargo lockfiles in the owning
   component repository before moving its root gitlink.
4. Run `just cloud-stack-check` from a clean recursive checkout.
5. Include the printed compatibility-lock digest and component revisions in
   the pull request evidence. Do not publish the compatibility update until the
   Cloud contract gate passes.

The compatibility lock does not replace component release processes. A lock
change is integration evidence: each component still owns its implementation,
tests, release notes, and publication.

## Workflow Platform Planning

The target Cloud, Flow, Boot, ORM, and Form composition is defined by the
[workflow platform architecture](workflow-platform-architecture.md) and its
ordered [development plan](workflow-platform-development-plan.md). The current
lock pins the exact Form Core, Flow, Boot, ORM, Cloud, interaction protocols,
evaluation protocols, and shared fixtures used by the verified Phase 1 and
current Phase 2 implementation baseline. Native Form compilation and
submitted-value evaluation have byte-identical Cloud evidence. Cloud's
project-scoped canonical drafts, immutable releases, and exact Goal/Plan-bound
WorkflowRuns persist through A3S ORM/PostgreSQL and share one CQRS authority
across REST `1.14.0`, TypeScript client, CLI, and Management MCP. The minimal
run slice executes Workflow-local `input`, `transform`, `branch`, and `output`
steps through A3S Flow. This does not claim protected Form submissions,
HumanTask commands, human/service/finite-task dispatch, typed capability steps,
compensation, production recovery, or end-to-end HumanTask availability.
