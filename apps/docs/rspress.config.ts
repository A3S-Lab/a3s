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
    title: 'A3S — 为AI Native组织构建的AI操作系统生态',
    description: 'A3S 项目索引与工程博客。查看 36 个项目的职责、交付阶段、当前版本或通道和代码入口。',
    ecosystem: '生态',
    blog: '博客',
    articles: '文章',
    alternate: 'EN',
  },
  en: {
    title: 'A3S — The AI operating system ecosystem for AI Native organizations',
    description: 'The A3S project index and engineering blog. Browse 36 projects, their delivery stages, current versions or channels, sites, and source.',
    ecosystem: 'Ecosystem',
    blog: 'Blog',
    articles: 'Articles',
    alternate: '中文',
  },
} as const;

const articles = {
  cn: [
    ['A3S Code 6.8 的多 Agent 工作流：task、Flow 与预算边界', 'programmable-agent-workflows'],
    ['DDD 在 A3S Cloud 里怎么落地', 'domain-driven-design'],
    ['A3S Box 的 Windows 路径：libkrun、libkrunfw 与 WHPX', 'libkrun-libkrunfw-whpx'],
    ['HTTP 402 与 Agent 付费：先把协议边界说清楚', 'http-402-generative-ui-agent-economy'],
    ['A3S Code 6.8：Coding Agent 运行时现在包含什么', 'why-coding-agent-is-the-core'],
    ['A3S Gateway 1.0：AI 流量层的实际边界', 'why-ai-native-gateway'],
    ['A3S Power 0.7：模型无关推理、TEE 与可验证收据', 'a3s-power-technical-deep-dive'],
    ['A3S Box 3.2：默认 MicroVM，显式 Sandbox', 'a3s-box-technical-deep-dive'],
  ],
  en: [
    ['Multi-Agent Workflows in A3S Code 6.8: task, Flow, and Budget Limits', 'programmable-agent-workflows'],
    ['How DDD Is Used in A3S Cloud', 'domain-driven-design'],
    ['The Windows Runtime Path in A3S Box: libkrun, libkrunfw, and WHPX', 'libkrun-libkrunfw-whpx'],
    ['HTTP 402 and Agent Payments: Start with the Protocol Boundary', 'http-402-generative-ui-agent-economy'],
    ['A3S Code 6.8: What the Coding-Agent Runtime Includes', 'why-coding-agent-is-the-core'],
    ['A3S Gateway 1.0: The Actual AI Traffic Boundary', 'why-ai-native-gateway'],
    ['A3S Power 0.7: Model-Neutral Inference, TEEs, and Verifiable Receipts', 'a3s-power-technical-deep-dive'],
    ['A3S Box 3.2: MicroVM by Default, Sandbox by Request', 'a3s-box-technical-deep-dive'],
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
