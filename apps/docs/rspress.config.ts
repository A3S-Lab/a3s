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
    title: 'A3S — 为 AI Native 组织构建的生态系统',
    description: 'A3S 生态首页与工程博客。查看 36 个项目的职责、开发阶段、网站和代码入口。',
    ecosystem: '生态',
    blog: '博客',
    articles: '文章',
    alternate: 'EN',
  },
  en: {
    title: 'A3S — The ecosystem for AI Native organizations',
    description: 'The A3S ecosystem homepage and engineering blog. Explore 36 projects, their delivery stages, sites, and repositories.',
    ecosystem: 'Ecosystem',
    blog: 'Blog',
    articles: 'Articles',
    alternate: '中文',
  },
} as const;

const articles = {
  cn: [
    ['可编程、预算受限的多智能体工作流', 'programmable-agent-workflows'],
    ['领域驱动设计：回归本质的工程实践', 'domain-driven-design'],
    ['libkrun 深度解析：架构、模块与 Windows WHPX', 'libkrun-libkrunfw-whpx'],
    ['HTTP 402、生成式 UI 与智能体原生软件', 'http-402-generative-ui-agent-economy'],
    ['A3S Code v2.1.0：Coding Agent 运行时', 'why-coding-agent-is-the-core'],
    ['为什么需要重新设计 AI 流量层', 'why-ai-native-gateway'],
    ['10 美元硬件上的隐私 LLM 推理引擎', 'a3s-power-technical-deep-dive'],
    ['A3S Box：40MB MicroVM 运行时', 'a3s-box-technical-deep-dive'],
  ],
  en: [
    ['Programmable, Budget-Bounded Multi-Agent Workflows', 'programmable-agent-workflows'],
    ['Domain-Driven Design from the Ground Up', 'domain-driven-design'],
    ['libkrun: Architecture, Modules, and Windows WHPX', 'libkrun-libkrunfw-whpx'],
    ['HTTP 402, Generative UI, and Agent-Native Software', 'http-402-generative-ui-agent-economy'],
    ['A3S Code v2.1.0: The Coding Agent Runtime', 'why-coding-agent-is-the-core'],
    ['Why the AI Traffic Layer Needs a Redesign', 'why-ai-native-gateway'],
    ['A Privacy LLM Inference Engine on $10 Hardware', 'a3s-power-technical-deep-dive'],
    ['A3S Box: A 40MB MicroVM Runtime', 'a3s-box-technical-deep-dive'],
  ],
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
      { text: copy[locale].blog, link: '/blog/' },
      {
        text: copy[locale].alternate,
        link: `${publicSite.origin}${alternateBase}`,
      },
    ],
    sidebar: {
      '/blog/': [
        {
          text: copy[locale].articles,
          items: articles[locale].map(([text, slug]) => ({
            text,
            link: `/blog/${slug}`,
          })),
        },
      ],
    },
    socialLinks: [
      {
        icon: 'github',
        mode: 'link',
        content: 'https://github.com/A3S-Lab/a3s',
      },
    ],
  },
});
