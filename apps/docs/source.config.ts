import { defineDocs, defineCollections, defineConfig } from 'fumadocs-mdx/config';
import { remarkCodeHike, recmaCodeHike } from 'codehike/mdx';
import { z } from 'zod';

const chConfig = {
  components: { code: 'Code' },
  syntaxHighlighting: { theme: 'one-dark-pro' },
};

// Regular docs — use fumadocs' default rehype-code (Shiki) for code blocks
export const docs = defineDocs({
  dir: 'content/docs',
});

// Tutorials — codehike enabled for scrollycoding
export const tutorials = defineDocs({
  dir: 'content/tutorials',
  docs: {
    mdxOptions: {
      remarkPlugins: [[remarkCodeHike, chConfig]],
      recmaPlugins: [[recmaCodeHike, chConfig]],
    },
  },
});

export const blog = defineCollections({
  type: 'doc',
  dir: 'content/blog',
  schema: z.object({
    title: z.string(),
    description: z.string(),
    date: z.string(),
    author: z.string().optional(),
    tags: z.array(z.string()).optional(),
  }),
});

export default defineConfig({
  mdxOptions: {
    rehypeCodeOptions: {
      themes: {
        light: 'one-dark-pro',
        dark: 'one-dark-pro',
      },
    },
  },
});
