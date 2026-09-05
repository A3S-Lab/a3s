# A3S Ecosystem Site

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Platform engineers, AI application builders, and operators evaluating the A3S
ecosystem. They need a reliable directory that explains project ownership,
delivery status, public sites, source repositories, and installation paths.

## Product Purpose

The bilingual A3S site explains the ecosystem, publishes Desktop downloads,
and routes visitors to each product's owned website. Success means a visitor
can find the project responsible for a job, understand its current delivery
boundary, and continue to the maintained product surface.

## Positioning

The root site is an index, not a second home for product interfaces. A3S Flow
owns workflow authoring components, node documentation, React and Vue hooks,
CLI commands, and its Agent Skill. The root site links to the Flow website and
uses a captured preview from that deployed product surface.

## Operating Context

- Rspress builds Chinese at `/` and English under `/en/`.
- Desktop downloads live at `/download/` and `/en/download/`.
- Product cards use committed previews captured from healthy public sites.
- Product-specific documentation and interactive authoring stay in the
  repository that owns the product.

## Capabilities and Constraints

- The project directory remains the source of truth for 35 project entries.
- Every entry exposes a categorical delivery stage and a checked release or
  channel. The site never turns those categories into completion percentages.
- Public product sites receive a primary link and a separate source link.
- Desktop downloads lead with installer aliases from the root A3S repository's
  `desktop-latest` Release and retain tag-pinned historical packages with
  checked bilingual release notes.
- The primary navigation covers the ecosystem, principles, Desktop downloads,
  locale selection, and source access. Product destinations remain in the
  directory and footer rather than becoming top-level product menus.
- Chinese and English routes remain structurally equivalent.
- The build rejects standalone Playground, removed Form, blog, docs, and
  tutorial routes.

## Brand Commitments

- Use the A3S name, logo, Geist type family, cool white surfaces, restrained
  blue accents, and green delivery state already present in `apps/docs`.
- Keep the interface precise, readable, and operational. Product previews
  support navigation and must not become decorative mockups.

## Evidence on Hand

- `apps/docs/rspress.config.ts` defines the bilingual build and route boundary.
- `apps/docs/components/home/styles/home-base.css` contains the existing A3S
  site tokens and interaction language.
- `apps/docs/components/home/project-links.ts` owns product destinations.
- `apps/docs/components/home/ecosystem-status.ts` records checked release and
  delivery-stage data.
- <https://a3s-lab.github.io/Flow/> is the owned workflow product surface.

## Product Principles

1. Route every product concept to its owning repository.
2. Show checked status and source instead of invented completion metrics.
3. Use real deployed pages for previews.
4. Keep product-specific components out of the root site.
5. Keep Chinese and English experiences structurally equivalent.

## Accessibility & Inclusion

All essential actions must be keyboard reachable, focus-visible, and labeled.
Touch targets must remain usable on small screens, and motion must respect the
user's reduced-motion preference.
