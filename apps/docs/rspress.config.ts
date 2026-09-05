import path from 'node:path';
import { defineConfig } from '@rspress/core';

type SiteLocale = 'cn' | 'en';

const locale: SiteLocale = process.env.SITE_LOCALE === 'en' ? 'en' : 'cn';
const publicSite = new URL(process.env.SITE_URL ?? 'https://a3s.dev');
const deploymentPath = publicSite.pathname.replace(/\/+$/, '');
const rootBase = `${deploymentPath || ''}/`.replace(/\/+/g, '/');
const base = locale === 'en' ? `${rootBase}en/` : rootBase;
const alternateBase = locale === 'en' ? rootBase : `${rootBase}en/`;

const copy = {
  cn: {
    title: 'A3S — 为 Agent 时代构建 AI 原生操作系统',
    description: 'A3S OS 让流体智力在新问题中即时组合，并把经过验证的判断沉淀为团队可复用的晶体智力。',
    ecosystem: '生态',
    alternate: 'EN',
  },
  en: {
    title: 'A3S — An AI-native operating system for fluid and crystal intelligence',
    description: 'A3S OS helps teams combine resources around new problems and turn verified judgment into reusable crystal intelligence.',
    ecosystem: 'Ecosystem',
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
