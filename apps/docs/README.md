# A3S website and CLI documentation

The public A3S site combines the product homepage, A3S CLI documentation, and
the engineering blog in one statically generated Rspress application.

Chinese is the default language and uses unprefixed routes such as `/docs`.
English uses `/en`, for example `/en/docs`.

## Stack

- Rspress 2 owns routing, search, static generation, and the documentation
  shell.
- CodeHike renders syntax-highlighted code blocks.
- MDX content and Rspress `_nav.json` / `_meta.json` files live under `docs/`.
- The public `/docs` tree covers only the root-owned A3S CLI. Historical
  project manuals are excluded from route generation.
- The custom `HomeLayout` renders the interactive homepage.
- GitHub Pages publishes the generated `out/` directory.

The homepage data under `components/home/architecture/` describes the internal
technical architecture of each listed project. The lifecycle view introduces
those projects in the order an agent moves from local development to deployment
and operation.

## Local development

```bash
npm ci
npm run dev
```

## Verification

```bash
npm run check:cloud
npm run test
npm run typecheck
npm run build
npm run check:site
npm run lint
npm run format:check
```

## CLI documentation structure

Public CLI pages live directly under `docs/<language>/docs/`. Add the same
slug in `zh` and `en`, then include it in each `docs/_meta.json`. Product-level
API manuals belong in the corresponding product repository.
