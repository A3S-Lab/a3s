import type { ProjectId } from './project-links';

export type ProjectSiteCaptureLanguage = 'en' | 'zh';

export interface LocalizedProjectSitePreview {
  captureLanguage?: ProjectSiteCaptureLanguage;
  captureUrl?: string;
  screenshot: string;
}

export interface FeaturedProjectSite {
  id: ProjectId;
  href: string;
  captureUrl: string;
  captureLanguage?: ProjectSiteCaptureLanguage;
  displayUrl: string;
  screenshot: string;
  localizedPreviews?: Partial<Record<'cn' | 'en', LocalizedProjectSitePreview>>;
  settleMs: number;
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
    settleMs: 10_600,
    mode: 'live',
    destination: 'site',
  },
  {
    id: 'code',
    href: 'https://a3s-lab.github.io/Code/',
    captureUrl: 'https://a3s-lab.github.io/Code/',
    displayUrl: 'a3s-lab.github.io/Code',
    screenshot: '/ecosystem-sites/code.png',
    settleMs: 14_000,
    mode: 'live',
    destination: 'site',
  },
  {
    id: 'flow',
    href: 'https://a3s-lab.github.io/Flow/',
    captureUrl: 'https://a3s-lab.github.io/Flow/',
    displayUrl: 'a3s-lab.github.io/Flow',
    screenshot: '/ecosystem-sites/flow.png',
    localizedPreviews: {
      en: {
        captureUrl: 'https://a3s-lab.github.io/Flow/en/',
        screenshot: '/ecosystem-sites/flow-en.png',
      },
    },
    settleMs: 1_800,
    mode: 'live',
    destination: 'site',
  },
  {
    id: 'office',
    href: 'https://a3s-lab.github.io/Office/',
    captureUrl: 'https://a3s-lab.github.io/Office/',
    displayUrl: 'a3s-lab.github.io/Office',
    screenshot: '/ecosystem-sites/office.png',
    settleMs: 1_800,
    mode: 'live',
    destination: 'site',
  },
  {
    id: 'power',
    href: 'https://a3s-lab.github.io/Power/',
    captureUrl: 'https://a3s-lab.github.io/Power/',
    displayUrl: 'a3s-lab.github.io/Power',
    screenshot: '/ecosystem-sites/power.png',
    settleMs: 5_000,
    mode: 'live',
    destination: 'site',
  },
  {
    id: 'box',
    href: 'https://a3s-lab.github.io/Box/',
    captureUrl: 'https://a3s-lab.github.io/Box/',
    displayUrl: 'a3s-lab.github.io/Box',
    screenshot: '/ecosystem-sites/box.png',
    settleMs: 4_600,
    mode: 'live',
    destination: 'site',
  },
  {
    id: 'use',
    href: 'https://a3s-lab.github.io/Use/',
    captureUrl: 'https://a3s-lab.github.io/Use/',
    displayUrl: 'a3s-lab.github.io/Use',
    screenshot: '/ecosystem-sites/use.png',
    settleMs: 3_200,
    mode: 'live',
    destination: 'site',
  },
  {
    id: 'ui',
    href: 'https://a3s-lab.github.io/UI/',
    captureUrl: 'https://a3s-lab.github.io/UI/',
    displayUrl: 'a3s-lab.github.io/UI',
    screenshot: '/ecosystem-sites/ui.png',
    settleMs: 2_400,
    mode: 'live',
    destination: 'site',
  },
  {
    id: 'gateway',
    href: 'https://a3s-lab.github.io/Gateway/',
    captureUrl: 'https://a3s-lab.github.io/Gateway/',
    captureLanguage: 'zh',
    displayUrl: 'a3s-lab.github.io/Gateway',
    screenshot: '/ecosystem-sites/gateway.png',
    localizedPreviews: {
      en: {
        captureLanguage: 'en',
        screenshot: '/ecosystem-sites/gateway-en.png',
      },
    },
    settleMs: 3_500,
    mode: 'live',
    destination: 'site',
  },
] as const satisfies readonly FeaturedProjectSite[];
