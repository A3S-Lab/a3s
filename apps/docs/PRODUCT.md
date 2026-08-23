# A3S Documentation Site

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Platform engineers, AI application builders, and operators evaluating or
learning the A3S ecosystem. They need to understand a workflow by manipulating
the same graph concepts exposed by A3S Cloud, without provisioning a control
plane first.

## Product Purpose

The bilingual A3S documentation site explains the ecosystem and provides an
interactive Workflow Designer Playground. Success means a visitor can compose,
inspect, validate, and simulate a representative workflow directly in the
browser, then understand how that graph maps to A3S Cloud terminology.

## Positioning

The Playground is a repository-derived teaching surface. Its node catalog and
labels follow the current A3S Cloud semantic workflow contract instead of
inventing a separate visual-only runtime model.

## Operating Context

- Rspress builds Chinese at `/` and English under `/en/`.
- The Playground runs entirely in the browser and requires no credentials.
- Users work on a pannable node canvas, configure the selected step in a side
  panel, and inspect validation, variables, execution trace, and run history.
- Example runs are deterministic simulations and must be labeled as such.

## Capabilities and Constraints

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
- Existing homepage routes, locale behavior, and project-directory behavior
  remain intact.

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
- `apps/cloud/docs/domain-model.md` documents the Workflow semantic step kinds
  and standalone-to-Cloud mapping.
- `apps/cloud/crates/control-plane/src/modules/workflow/domain/workflow_contract.rs`
  is the executable source of truth for the closed step-kind catalog.
- No production credentials, customer data, benchmarks, or hosted execution
  endpoint are available to the browser Playground.

## Product Principles

1. Demonstrate real A3S concepts instead of marketing abstractions.
2. Keep graph manipulation familiar enough to learn by direct use.
3. Make validation and runtime state visible at the point of action.
4. Preserve a safe boundary between deterministic simulation and production
   execution.
5. Keep Chinese and English experiences structurally equivalent.

## Accessibility & Inclusion

All essential actions must be keyboard reachable, focus-visible, and labeled.
Touch targets must remain usable on small screens, and motion must respect the
user's reduced-motion preference.
