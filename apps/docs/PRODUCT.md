# A3S Documentation Site

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Platform engineers, AI application builders, and operators evaluating or
learning the A3S ecosystem. They need a reliable project directory, release
and installation paths, and a way to understand a workflow by manipulating the
same graph concepts exposed by A3S Cloud without provisioning a control plane
first.

## Product Purpose

The bilingual A3S site is the ecosystem index, Desktop download surface, and
interactive Workflow Designer Playground. Success means a visitor can find the
product that owns a capability, compose and simulate a representative workflow
directly in the browser, and understand how that graph maps to A3S Cloud
terminology.

## Positioning

The Playground is a repository-derived teaching surface. Its node catalog and
labels follow the current A3S Cloud semantic workflow contract instead of
inventing a separate visual-only runtime model.

A3S Flow owns reusable workflow-authoring components, framework integrations,
CLI and Skill usage, and node reference documentation on its product site. The
root site links to that authoritative surface while keeping the browser-local
Playground as a safe, zero-setup learning and integration experience. It does
not duplicate Flow's package documentation or claim to publish workflows.

## Operating Context

- Rspress builds Chinese at `/` and English under `/en/`.
- Desktop downloads live at `/download/` and `/en/download/`.
- Product cards use committed previews captured from healthy public sites.
- The Playground runs entirely in the browser and requires no credentials.
- Users work on a pannable node canvas, configure the selected step in a side
  panel, and inspect validation, variables, execution trace, and run history.
- Example runs are deterministic simulations and must be labeled as such.

## Capabilities and Constraints

- The project directory remains the source of truth for 34 project entries.
- Every entry exposes a categorical delivery stage and a checked release or
  channel. The site never turns those categories into completion percentages.
- Public product sites receive a primary link and a separate source link.
- The node catalog mirrors the current Cloud semantic step kinds: Input,
  Output, Transform, Branch, Human Decision, Execution, Agent, MCP, Model,
  Tool, Service, Memory, and Subworkflow.
- Graph editing includes adding, selecting, moving, connecting, duplicating,
  deleting, undoing, redoing, zooming, fitting, and resetting the example.
- Debugging includes graph validation, per-step runs, whole-workflow runs,
  editable input variables, execution trace, output inspection, and local run
  history.
- Playground state is local and ephemeral. It does not publish revisions or
  call production A3S Cloud APIs.
- Public copy, source comments, route metadata, and documentation use only A3S
  product terminology.
- Product cards and navigation link to each independently maintained product
  site for detailed capabilities and release documentation.
- Existing homepage routes, locale behavior, and project-directory behavior
  remain intact.
- The build rejects removed Form, blog, docs, and tutorial routes.

## Brand Commitments

- Use the A3S name, logo, Geist type family, cool white surfaces, restrained
  blue selection state, and green execution state already present in
  `apps/docs`.
- Keep the interface precise, compact, and operational. Decoration must not
  compete with the graph.

## Evidence on Hand

- `apps/docs/rspress.config.ts` defines the bilingual build and routes.
- `apps/docs/components/home/styles/home-base.css` contains the existing A3S
  site tokens and interaction language.
- `apps/docs/components/home/project-links.ts` owns product destinations.
- `apps/docs/components/home/ecosystem-status.ts` records checked release and
  delivery-stage data.
- `apps/cloud/docs/domain-model.md` documents the Workflow semantic step kinds
  and standalone-to-Cloud mapping.
- `apps/cloud/crates/control-plane/src/modules/workflow/domain/workflow_contract.rs`
  is the executable source of truth for the closed step-kind catalog.
- No production credentials, customer data, benchmarks, or hosted execution
  endpoint are available to the browser Playground.
- <https://a3s-lab.github.io/Flow/> is the owned workflow product surface.

## Product Principles

1. Route every product concept to its owning repository and maintained site.
2. Show checked status and source instead of invented completion metrics.
3. Demonstrate real A3S concepts instead of marketing abstractions.
4. Keep graph manipulation familiar enough to learn by direct use.
5. Make validation and runtime state visible at the point of action.
6. Preserve a safe boundary between deterministic simulation and production
   execution.
7. Keep Chinese and English experiences structurally equivalent.

## Accessibility & Inclusion

All essential actions must be keyboard reachable, focus-visible, and labeled.
Touch targets must remain usable on small screens, and motion must respect the
user's reduced-motion preference.
