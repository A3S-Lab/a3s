import path from 'node:path';
import { defineConfig } from '@rspress/core';

type SiteLocale = 'cn' | 'en';

const locale: SiteLocale = process.env.SITE_LOCALE === 'en' ? 'en' : 'cn';
const publicSite = new URL(process.env.SITE_URL ?? 'https://a3s.dev');
const deploymentPath = publicSite.pathname.replace(/\/+$/, '');
const rootBase = `${deploymentPath || ''}/`.replace(/\/+/g, '/');
const base = locale === 'en' ? `${rootBase}en/` : rootBase;
const alternateBase = locale === 'en' ? rootBase : `${rootBase}en/`;
const powerBase = locale === 'en' ? `${rootBase}power/en/` : `${rootBase}power/`;

const copy = {
  cn: {
    title: 'A3S — 为AI Native组织构建的AI操作系统生态',
    description: 'A3S 项目索引。查看 34 个项目的职责、交付阶段、当前版本或通道和代码入口。',
    ecosystem: '生态',
    power: 'Power',
    alternate: 'EN',
  },
  en: {
    title: 'A3S — The AI operating system ecosystem for AI Native organizations',
    description: 'The A3S project index. Browse 34 projects, their delivery stages, current versions or channels, sites, and source.',
    ecosystem: 'Ecosystem',
    power: 'Power',
    alternate: '中文',
  },
} as const;

function canonicalRoute(routePath: string) {
  const normalizedRoute = routePath.startsWith('/') ? routePath : `/${routePath}`;
  return `${publicSite.origin}${base.replace(/\/$/, '')}${normalizedRoute}`;
}

export default defineConfig({
  root: path.join(__dirname, 'site', locale),
  base,
  siteOrigin: publicSite.origin,
  title: copy[locale].title,
  description: copy[locale].description,
  lang: locale === 'cn' ? 'zh-CN' : 'en',
  i18nSource(source) {
    for (const translations of Object.values(source)) {
      translations['zh-CN'] ??= translations.zh ?? translations.en;
    }
    return source;
  },
  icon: new URL('./public/brand/a3s-os-logo.png', import.meta.url),
  logo: '/brand/a3s-os-logo.png',
  logoText: 'A3S',
  outDir: process.env.SITE_OUT_DIR ?? path.join('site_build', locale),
  route: {
    cleanUrls: true,
  },
  search: {
    mode: 'local',
  },
  head: [
    ['meta', { name: 'theme-color', content: '#ffffff' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:site_name', content: 'A3S' }],
    ['meta', { property: 'og:image', content: `${publicSite.origin}${rootBase}brand/a3s-os-logo.png` }],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    (route) => ['link', { rel: 'canonical', href: canonicalRoute(route.routePath) }],
  ],
  builderConfig: {
    source: {
      define: {
        __A3S_SITE_LOCALE__: JSON.stringify(locale),
      },
    },
    resolve: {
      alias: {
        '@': __dirname,
      },
    },
    server: {
      publicDir: {
        name: path.join(__dirname, 'public'),
      },
    },
  },
  themeConfig: {
    darkMode: 'force-light',
    enableContentAnimation: true,
    lastUpdated: true,
    nav: [
      { text: copy[locale].ecosystem, link: '/' },
      { text: copy[locale].power, link: powerBase },
      {
        text: copy[locale].alternate,
        link: `${publicSite.origin}${alternateBase}`,
      },
    ],
    socialLinks: [
      {
        icon: 'github',
        mode: 'link',
        content: 'https://github.com/A3S-Lab/a3s',
      },
    ],
  },
});
