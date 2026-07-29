import * as path from 'node:path';
import { defineConfig } from '@rspress/core';

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
  lang: 'zh',
  icon: '/favicon.svg',
  logo: '/a3s-mark.svg',
  logoText: 'A3S CLI',
  outDir: 'doc_build',
  llms: true,
  locales: [
    {
      lang: 'zh',
      label: '简体中文',
      title: 'A3S CLI',
      description: '用一个 CLI 安装、配置和运行 A3S 本地工具链。',
    },
    {
      lang: 'en',
      label: 'English',
      title: 'A3S CLI',
      description:
        'Install, configure, and run the A3S local toolchain from one command-line interface.',
    },
  ],
  head: [
    ['meta', { name: 'theme-color', content: '#f7f8fb' }],
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
    darkMode: 'force-light',
    search: false,
    localeRedirect: 'never',
    enableContentAnimation: false,
    socialLinks: [
      {
        icon: 'github',
        mode: 'link',
        content: 'https://github.com/A3S-Lab/a3s',
      },
    ],
  },
});
