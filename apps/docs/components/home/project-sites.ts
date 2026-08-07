export const featuredProjectSites = [
  {
    id: 'cloud',
    href: 'https://a3s-lab.github.io/Cloud/',
    captureUrl: 'https://a3s-lab.github.io/Cloud/',
    displayUrl: 'a3s-lab.github.io/Cloud',
    screenshot: '/ecosystem-sites/cloud.png',
  },
  {
    id: 'code',
    href: 'https://a3s-lab.github.io/Code/',
    captureUrl: 'https://a3s-lab.github.io/Code/',
    displayUrl: 'a3s-lab.github.io/Code',
    screenshot: '/ecosystem-sites/code.png',
  },
  {
    id: 'office',
    href: 'https://a3s-lab.github.io/Office/',
    captureUrl: 'https://a3s-lab.github.io/Office/',
    displayUrl: 'a3s-lab.github.io/Office',
    screenshot: '/ecosystem-sites/office.png',
  },
  {
    id: 'use',
    href: 'https://a3s-lab.github.io/Use/',
    captureUrl: 'https://a3s-lab.github.io/Use/',
    displayUrl: 'a3s-lab.github.io/Use',
    screenshot: '/ecosystem-sites/use.png',
  },
  {
    id: 'ui',
    href: 'https://a3s-lab.github.io/UI/',
    captureUrl: 'https://a3s-lab.github.io/UI/',
    displayUrl: 'a3s-lab.github.io/UI',
    screenshot: '/ecosystem-sites/ui.png',
  },
  {
    id: 'gateway',
    href: 'https://a3s-lab.github.io/Gateway/',
    captureUrl: 'https://a3s-lab.github.io/Gateway/',
    displayUrl: 'a3s-lab.github.io/Gateway',
    screenshot: '/ecosystem-sites/gateway.png',
  },
  {
    id: 'site',
    href: '/blog/',
    captureUrl: 'https://a3s-lab.github.io/a3s/blog/',
    displayUrl: 'a3s.dev/blog',
    screenshot: '/ecosystem-sites/blog.png',
  },
] as const;

export type FeaturedProjectSite = (typeof featuredProjectSites)[number];
