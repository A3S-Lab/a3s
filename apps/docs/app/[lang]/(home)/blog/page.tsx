import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { BlogList } from '@/components/blog-list';
import { isLocale, localizedUrl, openGraphLocale } from '@/lib/i18n';

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev';

interface PageProps {
  params: Promise<{ lang: string }>;
}

export async function generateMetadata({ params }: PageProps): Promise<Metadata> {
  const { lang } = await params;
  if (!isLocale(lang)) notFound();
  const isChinese = lang === 'cn';

  return {
    title: isChinese ? '博客' : 'Blog',
    description: isChinese
      ? 'A3S 团队的技术文章、版本说明与深度解析。'
      : 'Engineering articles, release notes, and deep dives from the A3S team.',
    alternates: {
      canonical: localizedUrl(siteUrl, '/blog', lang),
      languages: {
        'zh-CN': localizedUrl(siteUrl, '/blog', 'cn'),
        en: localizedUrl(siteUrl, '/blog', 'en'),
      },
    },
    openGraph: { locale: openGraphLocale(lang) },
  };
}

export default async function BlogPage({ params }: PageProps) {
  const { lang } = await params;
  if (!isLocale(lang)) notFound();
  return <BlogList locale={lang} />;
}
