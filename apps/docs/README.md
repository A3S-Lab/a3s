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
- **Architecture atlas** renders the repository map from one 37-project data
  source. Every project has its own evidence-linked topology with real component
  names, variable node counts, explicit relationships, keyboard-operable project
  and node selection, and direct documentation or repository links.
- **Homepage diagrams** use repository-native SVG and CSS. The visual system is
  deliberately monochrome: black surfaces, fine solid and dotted relationships,
  square controls, monospace labels, and one restrained status color.
- **GitHub Pages** receives the generated `out/` directory from the repository
  documentation workflow.

The homepage keeps localized content, project architecture data, interaction,
and visual tokens in separate modules. Search, node selection, relationship
navigation, and mobile horizontal diagram scrolling are all client-side; the
project topology itself stays in typed source files.

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
