import type { ProjectId } from './project-links';

export interface FeaturedProjectSite {
  id: ProjectId;
  href: string;
  captureUrl: string;
  displayUrl: string;
  screenshot: string;
  mode: 'live' | 'build';
  destination: 'site' | 'repository';
}

export const featuredProjectSites = [
  {
    id: 'cloud',
    href: 'https://a3s-lab.github.io/Cloud/',
    captureUrl: 'https://a3s-lab.github.io/Cloud/',
    displayUrl: 'a3s-lab.github.io/Cloud',
    screenshot: '/ecosystem-sites/cloud.png',
    mode: 'live',
    destination: 'site',
  },
  {
    id: 'code',
    href: 'https://a3s-lab.github.io/Code/',
    captureUrl: 'https://a3s-lab.github.io/Code/',
    displayUrl: 'a3s-lab.github.io/Code',
    screenshot: '/ecosystem-sites/code.png',
    mode: 'live',
    destination: 'site',
  },
  {
    id: 'office',
    href: 'https://a3s-lab.github.io/Office/',
    captureUrl: 'https://a3s-lab.github.io/Office/',
    displayUrl: 'a3s-lab.github.io/Office',
    screenshot: '/ecosystem-sites/office.png',
    mode: 'live',
    destination: 'site',
  },
  {
    id: 'form',
    href: 'https://github.com/A3S-Lab/Form',
    captureUrl: 'https://a3s-lab.github.io/Form/playground/',
    displayUrl: 'a3s-lab.github.io/Form/playground',
    screenshot: '/ecosystem-sites/form.png',
    mode: 'build',
    destination: 'repository',
  },
  {
    id: 'box',
    href: 'https://a3s-lab.github.io/Box/',
    captureUrl: 'https://a3s-lab.github.io/Box/',
    displayUrl: 'a3s-lab.github.io/Box',
    screenshot: '/ecosystem-sites/box.png',
    mode: 'live',
    destination: 'site',
  },
  {
    id: 'use',
    href: 'https://a3s-lab.github.io/Use/',
    captureUrl: 'https://a3s-lab.github.io/Use/',
    displayUrl: 'a3s-lab.github.io/Use',
    screenshot: '/ecosystem-sites/use.png',
    mode: 'live',
    destination: 'site',
  },
  {
    id: 'ui',
    href: 'https://a3s-lab.github.io/UI/',
    captureUrl: 'https://a3s-lab.github.io/UI/',
    displayUrl: 'a3s-lab.github.io/UI',
    screenshot: '/ecosystem-sites/ui.png',
    mode: 'live',
    destination: 'site',
  },
  {
    id: 'gateway',
    href: 'https://a3s-lab.github.io/Gateway/',
    captureUrl: 'https://a3s-lab.github.io/Gateway/',
    displayUrl: 'a3s-lab.github.io/Gateway',
    screenshot: '/ecosystem-sites/gateway.png',
    mode: 'live',
    destination: 'site',
  },
] as const satisfies readonly FeaturedProjectSite[];
