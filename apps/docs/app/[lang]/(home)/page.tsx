import type { Metadata } from 'next';
import HomePage from '@/components/home-page';
import { isLocale, locales, localizedUrl, openGraphLocale } from '@/lib/i18n';
import { notFound } from 'next/navigation';

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev';

export function generateStaticParams() {
  return locales.map((lang) => ({ lang }));
}

export async function generateMetadata({ params }: { params: Promise<{ lang: string }> }): Promise<Metadata> {
  const { lang } = await params;
  if (!isLocale(lang)) notFound();
  const isCn = lang === 'cn';

  return {
    title: isCn ? 'A3S — 可治理 Agent 与可组合基础设施' : 'A3S — Governed Agents. Composable Infrastructure.',
    description: isCn
      ? '面向可治理 Agent、本地 AI 工作与可组合基础设施的 Rust 原生平台。'
      : 'A Rust-native platform for governed agents, local AI work, and composable infrastructure.',
    alternates: {
      canonical: localizedUrl(siteUrl, '/', lang),
      languages: {
        'zh-CN': localizedUrl(siteUrl, '/', 'cn'),
        en: localizedUrl(siteUrl, '/', 'en'),
      },
    },
    openGraph: {
      locale: openGraphLocale(lang),
    },
  };
}

export default async function Page({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params;
  if (!isLocale(lang)) notFound();
  return <HomePage lang={lang} />;
}
