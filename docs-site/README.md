# A3S Documentation Site

Official documentation website for the A3S ecosystem, built with [Fumadocs](https://fumadocs.dev/) and Next.js.

## Quick Start

### Prerequisites

- Node.js 18+ or Bun 1.0+
- pnpm, npm, or bun

### Installation

```bash
# Using bun (recommended)
bun install

# Or using pnpm
pnpm install

# Or using npm
npm install
```

### Development

```bash
# Start development server
bun dev
# or
pnpm dev
# or
npm run dev
```

The site will be available at http://localhost:3000 (or the next available port).

### Build

```bash
# Build for production
bun run build

# Start production server
bun start
```

## Documentation Structure

```
docs-site/
├── app/                    # Next.js App Router
│   ├── docs/              # Documentation pages
│   │   ├── layout.tsx     # Docs layout with sidebar
│   │   └── [[...slug]]/   # Dynamic docs routes
│   ├── layout.tsx         # Root layout
│   └── page.tsx           # Homepage
├── content/docs/          # MDX documentation content
│   ├── index.mdx          # Overview
│   ├── architecture.mdx   # System architecture
│   ├── code.mdx           # a3s-code documentation
│   ├── lane.mdx           # a3s-lane documentation
│   ├── context.mdx        # a3s-context documentation
│   ├── box.mdx            # a3s-box documentation
│   └── meta.json          # Navigation configuration
├── lib/
│   └── source.ts          # Fumadocs source loader
└── source.config.ts       # MDX configuration
```

## Adding Documentation

### Create a New Page

1. Create a new `.mdx` file in `content/docs/`:

```mdx
---
title: Your Page Title
description: Page description
---

# Your Page Title

Your content here...
```

2. Add the page to `content/docs/meta.json`:

```json
{
  "title": "Documentation",
  "pages": ["index", "architecture", "your-page"]
}
```

### MDX Features

Fumadocs supports:
- GitHub Flavored Markdown
- Code blocks with syntax highlighting
- Callouts and admonitions
- Tabs and accordions
- Custom React components

## Technology Stack

- **Framework**: Next.js 15 (App Router)
- **Documentation**: Fumadocs
- **Styling**: Tailwind CSS
- **Content**: MDX
- **Runtime**: Bun (recommended) or Node.js

## Links

- [A3S Main Repository](https://github.com/A3S-Lab/a3s)
- [Fumadocs Documentation](https://fumadocs.dev/)
- [Next.js Documentation](https://nextjs.org/docs)
