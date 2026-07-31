import type { Metadata } from 'next';
import HomePage from '@/components/home-page';
import { isLocale, locales, localizedUrl, openGraphLocale } from '@/lib/i18n';
import { notFound } from 'next/navigation';

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev';
const socialImage = `${siteUrl.replace(/\/$/, '')}/opengraph-image`;
const metadataCopy = {
  cn: {
    title: 'A3S — Agent 工具、工作流与运行时',
    description:
      'A3S 包含 CLI、Code、Browser、Office、Flow、Runtime、Cloud 等独立项目，可按需安装并通过公开接口组合。',
  },
  en: {
    title: 'A3S — Agent tools, workflows, and runtimes',
    description:
      'A3S provides independently released CLI, Code, Browser, Office, Flow, Runtime, Cloud, and related projects that connect through public interfaces.',
  },
} as const;

export function generateStaticParams() {
  return locales.map((lang) => ({ lang }));
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ lang: string }>;
}): Promise<Metadata> {
  const { lang } = await params;
  if (!isLocale(lang)) notFound();
  const copy = metadataCopy[lang];

  return {
    title: { absolute: copy.title },
    description: copy.description,
    alternates: {
      canonical: localizedUrl(siteUrl, '/', lang),
      languages: {
        'zh-CN': localizedUrl(siteUrl, '/', 'cn'),
        en: localizedUrl(siteUrl, '/', 'en'),
      },
    },
    openGraph: {
      locale: openGraphLocale(lang),
      title: copy.title,
      description: copy.description,
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
      title: copy.title,
      description: copy.description,
      images: [socialImage],
    },
  };
}

export default async function Page({
  params,
}: {
  params: Promise<{ lang: string }>;
}) {
  const { lang } = await params;
  if (!isLocale(lang)) notFound();
  return <HomePage lang={lang} />;
}
