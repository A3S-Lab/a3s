import * as path from 'node:path';
import { defineConfig } from '@rspress/core';
import { pluginSitemap } from '@rspress/plugin-sitemap';
import documentation from './documentation.json';

const base = process.env.SITE_BASE ?? '/a3s/';
const siteOrigin = process.env.SITE_ORIGIN ?? 'https://a3s-lab.github.io';
const siteUrl = `${siteOrigin}${base}`;

export default defineConfig({
  root: path.join(__dirname, 'docs'),
  base,
  siteOrigin,
  title: 'A3S CLI',
  description:
    'Install, configure, and run the A3S local toolchain from one command-line interface.',
  lang: documentation.defaultLocale,
  icon: '/favicon.svg',
  logo: '/a3s-mark.svg',
  logoText: 'A3S CLI',
  outDir: 'doc_build',
  llms: true,
  locales: documentation.locales.map(({ lang, label, title, description }) => ({
    lang,
    label,
    title,
    description,
  })),
  multiVersion: {
    default: documentation.defaultVersion,
    versions: documentation.versions.map(({ id }) => id),
  },
  plugins: [
    pluginSitemap({
      siteUrl,
      defaultChangeFreq: 'weekly',
      defaultPriority: '0.8',
    }),
  ],
  head: [
    ['meta', { name: 'theme-color', content: '#090a0d' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:site_name', content: 'A3S CLI' }],
    ['meta', { property: 'og:image', content: `${siteUrl}social-card.svg` }],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    '<script async src="https://www.googletagmanager.com/gtag/js?id=G-VWVNY8DWPG"></script>',
    `<script src="${base}analytics.js"></script>`,
    (route) => [
      'link',
      {
        rel: 'canonical',
        href: `${siteOrigin}${base.replace(/\/$/, '')}${route.routePath}`,
      },
    ],
  ],
  themeConfig: {
    darkMode: 'force-dark',
    search: false,
    localeRedirect: 'never',
    enableContentAnimation: true,
    socialLinks: [
      {
        icon: 'github',
        mode: 'link',
        content: 'https://github.com/A3S-Lab/a3s',
      },
    ],
  },
});
