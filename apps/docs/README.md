# A3S website and documentation

The public A3S site combines a product homepage, bilingual documentation,
tutorials, and the engineering blog in one statically exported Next.js app.

Chinese is the default language and uses unprefixed URLs such as `/docs`.
English uses the `/en` prefix. The previously published `/cn` routes remain
build-time compatibility aliases, while canonical metadata and navigation
always point to the unprefixed Chinese route.

## Architecture

- **Next.js App Router** owns routes, metadata, and static export.
- **Fumadocs** owns documentation and tutorial content.
- **Locale routing** in `lib/i18n.ts` is the single source of truth for default
  language detection, canonical paths, and language switches.
- **Documentation versions** are discovered from product-local `vX.Y.Z`
  directories. The documentation header shows a version selector whenever a
  product has at least one immutable snapshot, and keeps the current topic when
  that topic exists in the selected version.
- **Home components** under `components/home/` keep localized content, layout,
  interaction, and design tokens separate.
- **Architecture atlas** renders the repository map from one 34-project data
  source. Every project exposes an interactive five-node topology with localized
  responsibilities, keyboard-operable project and node selection, and a direct
  documentation or repository link.
- **Canvas UI Grid** provides the viewport-sized interactive background across
  the homepage with a progressive CSS fallback; its notice is recorded in
  `THIRD_PARTY_NOTICES.md`.
- **GitHub Pages** receives the generated `out/` directory from the repository
  documentation workflow.

The homepage structure follows the same maintainable pattern as the A3S Code
site: one composition component, isolated client-side canvas behavior, and
centralized visual tokens rather than route-local styling.

## Local development

```bash
bun install --frozen-lockfile
bun run dev
```

## Verification

```bash
bun run typecheck
bun run test
bun run build
bun run check:site
```

`check:site` validates both localized homepage exports and confirms the
homepage CSS was emitted into the production bundle.

## Publishing a documentation version

Documentation versions follow the independent release cadence of each A3S
product; there is no monorepo-wide product version.

1. Copy the release-owned pages into
   `content/docs/<locale>/<product>/vX.Y.Z/` for both `cn` and `en`.
2. Add a localized `meta.json` and keep the snapshot immutable after release.
3. Add the version directory to the product's localized `meta.json` navigation.
4. When a product has a release evidence manifest, record the canonical Chinese
   and English document URLs there.
5. Run `bun run check` and any product-specific documentation validator.

The version selector reads the Fumadocs page tree, so publishing a version does
not require a second version registry.
