# A3S website and documentation

The public A3S site combines a product homepage, bilingual documentation,
tutorials, and the engineering blog in one statically exported Next.js app.

## Architecture

- **Next.js App Router** owns routes, metadata, and static export.
- **Fumadocs** owns documentation and tutorial content.
- **Home components** under `components/home/` keep localized content, layout,
  interaction, and design tokens separate.
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
bun run build
bun run check:site
```

`check:site` validates both localized homepage exports and confirms the
homepage CSS was emitted into the production bundle.
