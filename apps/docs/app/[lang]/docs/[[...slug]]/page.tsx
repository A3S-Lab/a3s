import { notFound } from 'next/navigation';
import { DocsContent, docsMetadata } from '@/components/docs-content';
import { isLocale, locales } from '@/lib/i18n';
import { source } from '@/lib/source';
import type { Metadata } from 'next';

interface PageProps {
  params: Promise<{ lang: string; slug?: string[] }>;
}

export default async function Page(props: PageProps) {
  const { lang, slug } = await props.params;
  if (!isLocale(lang)) notFound();
  return <DocsContent locale={lang} slug={slug} />;
}

export function generateStaticParams() {
  return locales.flatMap((lang) =>
    source.getPages(lang).map((page) => ({
      lang,
      slug: page.slugs.length > 0 ? page.slugs : undefined,
    })),
  );
}

export async function generateMetadata(props: PageProps): Promise<Metadata> {
  const { lang, slug } = await props.params;
  if (!isLocale(lang)) notFound();
  return docsMetadata({ locale: lang, slug });
}
