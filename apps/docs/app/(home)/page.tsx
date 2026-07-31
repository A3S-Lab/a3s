import type { Metadata } from 'next';
import HomePage from '@/components/home-page';
import { localizedUrl } from '@/lib/i18n';

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev';
const socialImage = `${siteUrl.replace(/\/$/, '')}/opengraph-image`;
const pageTitle = 'A3S — Agent 工具、工作流与运行时';
const pageDescription =
  'A3S 包含 CLI、Code、Browser、Office、Flow、Runtime、Cloud 等独立项目，可按需安装并通过公开接口组合。';

export const metadata: Metadata = {
  title: { absolute: pageTitle },
  description: pageDescription,
  alternates: {
    canonical: siteUrl,
    languages: {
      'zh-CN': localizedUrl(siteUrl, '/', 'cn'),
      en: localizedUrl(siteUrl, '/', 'en'),
    },
  },
  openGraph: {
    locale: 'zh_CN',
    title: pageTitle,
    description: pageDescription,
    images: [
      {
        url: socialImage,
        width: 1200,
        height: 630,
        alt: 'A3S — Agent tools, workflows, and runtimes',
      },
    ],
  },
  twitter: {
    card: 'summary_large_image',
    title: pageTitle,
    description: pageDescription,
    images: [socialImage],
  },
};

const jsonLd = {
  '@context': 'https://schema.org',
  '@type': 'SoftwareApplication',
  name: 'A3S',
  description:
    'A3S provides independently released CLI, Code, Browser, Office, Flow, Runtime, Cloud, and related projects that connect through public interfaces.',
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
