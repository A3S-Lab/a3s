import type { DocsLayoutProps } from 'fumadocs-ui/layout';

export const baseOptions: Partial<DocsLayoutProps> = {
  nav: {
    title: 'A3S',
  },
  links: [
    {
      text: 'Documentation',
      url: '/docs',
      active: 'nested-url',
    },
    {
      text: 'GitHub',
      url: 'https://github.com/A3S-Lab/a3s',
    },
  ],
};
