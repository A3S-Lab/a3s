import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { TutorialContent, tutorialMetadata } from '@/components/tutorial-content';
import { tutorialsSource } from '@/lib/tutorials';
import { isLocale, locales } from '@/lib/i18n';

interface PageProps {
  params: Promise<{ lang: string; slug?: string[] }>;
}

export default async function Page({ params }: PageProps) {
  const { lang, slug } = await params;
  if (!isLocale(lang)) notFound();
  return <TutorialContent locale={lang} slug={slug} />;
}

export function generateStaticParams() {
  return locales.flatMap((lang) => [
    { lang, slug: undefined },
    ...tutorialsSource.getPages(lang).map((page) => ({
      lang,
      slug: page.slugs.length > 0 ? page.slugs : undefined,
    })),
  ]);
}

export async function generateMetadata({ params }: PageProps): Promise<Metadata> {
  const { lang, slug } = await params;
  if (!isLocale(lang)) notFound();
  return tutorialMetadata({ locale: lang, slug });
}
