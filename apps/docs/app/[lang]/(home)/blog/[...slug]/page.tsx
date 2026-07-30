import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { BlogPost, blogPostMetadata } from '@/components/blog-post';
import { blogPostSlug, getBlogPosts } from '@/lib/blog';
import { isLocale, locales } from '@/lib/i18n';

interface PageProps {
  params: Promise<{ lang: string; slug: string[] }>;
}

export function generateStaticParams() {
  return locales.flatMap((lang) =>
    getBlogPosts(lang).map((post) => ({
      lang,
      slug: blogPostSlug(post.info.path).split('/'),
    })),
  );
}

export async function generateMetadata({ params }: PageProps): Promise<Metadata> {
  const { lang, slug } = await params;
  if (!isLocale(lang)) notFound();
  return blogPostMetadata({ locale: lang, slug: slug.join('/') });
}

export default async function Page({ params }: PageProps) {
  const { lang, slug } = await params;
  if (!isLocale(lang)) notFound();
  return <BlogPost locale={lang} slug={slug.join('/')} />;
}
