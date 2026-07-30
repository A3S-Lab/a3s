import type { Metadata } from 'next';
import HomePage from '@/components/home-page';
import { localizedUrl } from '@/lib/i18n';

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev';

export const metadata: Metadata = {
  title: 'A3S — 可治理 Agent 与可组合基础设施',
  description: '面向可治理 Agent、本地 AI 工作与可组合基础设施的 Rust 原生平台。',
  alternates: {
    canonical: siteUrl,
    languages: {
      'zh-CN': localizedUrl(siteUrl, '/', 'cn'),
      en: localizedUrl(siteUrl, '/', 'en'),
    },
  },
  openGraph: {
    locale: 'zh_CN',
  },
};

const jsonLd = {
  '@context': 'https://schema.org',
  '@type': 'SoftwareApplication',
  name: 'A3S',
  description:
    'A Rust-native platform for governed agents, local AI work, and composable infrastructure.',
  applicationCategory: 'DeveloperApplication',
  operatingSystem: 'Linux, macOS, Windows',
  url: siteUrl,
  author: {
    '@type': 'Organization',
    name: 'A3S Lab',
    url: 'https://github.com/A3S-Lab',
  },
  license: 'https://opensource.org/licenses/MIT',
  programmingLanguage: ['Rust', 'Python', 'TypeScript'],
  offers: {
    '@type': 'Offer',
    price: '0',
    priceCurrency: 'USD',
  },
};

export default function Page() {
  return (
    <>
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
      />
      <HomePage lang="cn" />
    </>
  );
}
