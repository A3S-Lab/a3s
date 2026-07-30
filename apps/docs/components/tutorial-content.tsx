import Link from 'next/link';
import { ArrowLeft, ArrowRight } from 'lucide-react';
import { notFound } from 'next/navigation';
import type { Metadata } from 'next';
import { SiteNav } from '@/components/site-nav';
import { tutorialsSource } from '@/lib/tutorials';
import { getMDXComponents } from '@/mdx-components';
import { createLocalizedRelativeMdxLink } from '@/lib/localized-mdx-link';
import {
  localePath,
  localizedUrl,
  openGraphLocale,
  type Locale,
} from '@/lib/i18n';

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev';

interface TutorialContentProps {
  locale: Locale;
  slug?: string[];
}

function TutorialList({ locale }: { locale: Locale }) {
  const isChinese = locale === 'cn';
  const tutorials = tutorialsSource.getPages(locale);

  return (
    <main
      className="min-h-screen"
      style={{ background: 'var(--ct-bg)', color: 'var(--ct-text)', fontFamily: 'var(--ct-font)' }}
    >
      <SiteNav lang={locale} section={isChinese ? '教程' : 'Tutorials'} />
      <section className="px-4 py-16 sm:px-6 sm:py-20">
        <div className="mx-auto max-w-3xl">
          <span className="text-xs font-semibold uppercase tracking-widest text-indigo-500">A3S Lab</span>
          <h1 className="mt-2 text-4xl font-extrabold tracking-tight text-slate-900 dark:text-slate-100 sm:text-5xl">
            {isChinese ? '教程' : 'Tutorials'}
          </h1>
          <p className="mt-4 text-lg text-slate-500 dark:text-slate-400">
            {isChinese
              ? '使用 A3S 构建生产级 AI Agent 的分步指南。'
              : 'Step-by-step guides for building production AI agents with A3S.'}
          </p>
        </div>
      </section>
      <section className="px-4 pb-24 sm:px-6">
        <div className="mx-auto max-w-3xl space-y-4">
          {tutorials.map((tutorial) => (
            <Link
              key={tutorial.url}
              href={localePath(tutorial.url, locale)}
              className="module-card group flex flex-col gap-3 rounded-xl border border-slate-100 bg-white p-6 hover:-translate-y-0.5 dark:border-slate-700 dark:bg-slate-800/60"
            >
              <div className="flex items-start justify-between gap-4">
                <div className="flex-1">
                  <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
                    {tutorial.data.title}
                  </h2>
                  <p className="mt-1.5 text-sm leading-relaxed text-slate-500 dark:text-slate-400">
                    {tutorial.data.description}
                  </p>
                </div>
                <ArrowRight
                  className="mt-1 h-4 w-4 shrink-0 text-slate-300 transition-transform duration-200 group-hover:translate-x-1 group-hover:text-indigo-500 dark:text-slate-600 dark:group-hover:text-indigo-400"
                  strokeWidth={2}
                />
              </div>
            </Link>
          ))}
        </div>
      </section>
      <footer className="border-t border-slate-200 px-4 py-8 dark:border-slate-700/60 sm:px-6">
        <div className="mx-auto flex max-w-7xl items-center justify-between">
          <span className="bg-gradient-to-r from-indigo-600 to-violet-600 bg-clip-text text-sm font-bold text-transparent">
            A3S Lab
          </span>
          <Link
            href={localePath('/', locale)}
            className="text-sm text-slate-400 transition-colors hover:text-indigo-600 dark:text-slate-500 dark:hover:text-indigo-400"
          >
            ← {isChinese ? '首页' : 'Home'}
          </Link>
        </div>
      </footer>
    </main>
  );
}

export function TutorialContent({ locale, slug }: TutorialContentProps) {
  if (!slug || slug.length === 0) return <TutorialList locale={locale} />;

  const page = tutorialsSource.getPage(slug, locale);
  if (!page) notFound();

  const isChinese = locale === 'cn';
  const MDX = page.data.body;
  const LocalizedLink = createLocalizedRelativeMdxLink(tutorialsSource, page, locale);

  return (
    <main
      className="min-h-screen"
      style={{ background: 'var(--ct-bg)', color: 'var(--ct-text)', fontFamily: 'var(--ct-font)' }}
    >
      <div className="border-b border-slate-200 px-4 py-3 dark:border-slate-700/60 sm:px-6">
        <div className="mx-auto max-w-7xl">
          <Link
            href={localePath('/tutorials', locale)}
            className="inline-flex items-center gap-1.5 text-sm text-slate-400 transition-colors hover:text-indigo-600 dark:text-slate-500 dark:hover:text-indigo-400"
          >
            <ArrowLeft className="h-3.5 w-3.5" />
            {isChinese ? '所有教程' : 'All tutorials'}
          </Link>
        </div>
      </div>
      <div className="mx-auto w-full max-w-7xl px-4 py-10 sm:px-6 xl:px-8">
        <h1 className="mb-3 text-3xl font-extrabold tracking-tight text-slate-900 dark:text-slate-100 sm:text-4xl">
          {page.data.title}
        </h1>
        {page.data.description && (
          <p className="mb-10 text-lg text-slate-500 dark:text-slate-400">
            {page.data.description}
          </p>
        )}
        <div className="prose prose-slate max-w-none dark:prose-invert prose-headings:font-extrabold prose-headings:tracking-tight prose-a:text-indigo-600 prose-a:no-underline hover:prose-a:underline dark:prose-a:text-indigo-400 prose-code:rounded prose-code:bg-slate-100 prose-code:px-1 prose-code:py-0.5 prose-code:text-[0.875em] dark:prose-code:bg-slate-800 prose-pre:border-0 prose-pre:bg-transparent prose-pre:p-0">
          <MDX
            components={getMDXComponents({
              a: LocalizedLink,
            })}
          />
        </div>
      </div>
    </main>
  );
}

export function tutorialMetadata({ locale, slug }: TutorialContentProps): Metadata {
  const isIndex = !slug || slug.length === 0;
  const page = isIndex ? null : tutorialsSource.getPage(slug, locale);
  if (!isIndex && !page) notFound();

  const isChinese = locale === 'cn';
  const pathname = `/tutorials${isIndex ? '' : `/${slug.join('/')}`}`;
  const title = page?.data.title ?? (isChinese ? '教程' : 'Tutorials');
  const description = page?.data.description ?? (isChinese
    ? '使用 A3S 构建 AI Agent 的分步教程。'
    : 'Step-by-step guides for building with A3S.');

  return {
    title,
    description,
    alternates: {
      canonical: localizedUrl(siteUrl, pathname, locale),
      languages: {
        'zh-CN': localizedUrl(siteUrl, pathname, 'cn'),
        en: localizedUrl(siteUrl, pathname, 'en'),
      },
    },
    openGraph: {
      title,
      description,
      locale: openGraphLocale(locale),
    },
  };
}
