import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: 'A3S Docs',
    },
    links: [
      {
        text: 'GitHub',
        url: 'https://github.com/A3S-Lab',
      },
    ],
  };
}
