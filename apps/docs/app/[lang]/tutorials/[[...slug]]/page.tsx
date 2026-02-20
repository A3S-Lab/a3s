import { tutorialsSource } from '@/lib/tutorials';
import { getMDXComponents } from '@/mdx-components';
import { createRelativeLink } from 'fumadocs-ui/mdx';
import { notFound } from 'next/navigation';
import Link from 'next/link';
import { ArrowLeft, ArrowRight } from 'lucide-react';
import { SiteNav } from '@/components/site-nav';
import type { Metadata } from 'next';

interface PageProps {
  params: Promise<{ lang: string; slug?: string[] }>;
}

// ── List page ────────────────────────────────────────────────
function TutorialsList({ lang }: { lang: string }) {
  const tutorials = tutorialsSource.getPages(lang);
  return (
    <main className="min-h-screen" style={{ background: 'var(--ct-bg)', color: 'var(--ct-text)', fontFamily: 'var(--ct-font)' }}>
      <SiteNav lang={lang} section="Tutorials" />
      <section className="px-4 py-16 sm:px-6 sm:py-20">
        <div className="mx-auto max-w-3xl">
          <span className="text-xs font-semibold uppercase tracking-widest text-indigo-500">A3S Lab</span>
          <h1 className="mt-2 text-4xl font-extrabold tracking-tight text-slate-900 dark:text-slate-100 sm:text-5xl">
            教程
          </h1>
          <p className="mt-4 text-lg text-slate-500 dark:text-slate-400">
            使用 A3S 构建生产级 AI 智能体的手把手指南。
          </p>
        </div>
      </section>
      <section className="px-4 pb-24 sm:px-6">
        <div className="mx-auto max-w-3xl space-y-4">
          {tutorials.map((tutorial) => (
            <Link
              key={tutorial.url}
              href={tutorial.url}
              className="module-card group flex flex-col gap-3 rounded-xl border border-slate-100 bg-white p-6 hover:-translate-y-0.5 dark:border-slate-700 dark:bg-slate-800/60"
            >
              <div className="flex items-start justify-between gap-4">
                <div className="flex-1">
                  <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">{tutorial.data.title}</h2>
                  <p className="mt-1.5 text-sm leading-relaxed text-slate-500 dark:text-slate-400">{tutorial.data.description}</p>
                </div>
                <ArrowRight className="mt-1 h-4 w-4 shrink-0 text-slate-300 transition-transform duration-200 group-hover:translate-x-1 group-hover:text-indigo-500 dark:text-slate-600 dark:group-hover:text-indigo-400" strokeWidth={2} />
              </div>
            </Link>
          ))}
        </div>
      </section>
      <footer className="border-t border-slate-200 px-4 py-8 dark:border-slate-700/60 sm:px-6">
        <div className="mx-auto flex max-w-7xl items-center justify-between">
          <span className="bg-gradient-to-r from-indigo-600 to-violet-600 bg-clip-text text-sm font-bold text-transparent">A3S Lab</span>
          <Link href={`/${lang}`} className="text-sm text-slate-400 transition-colors hover:text-indigo-600 dark:text-slate-500 dark:hover:text-indigo-400">← 首页</Link>
        </div>
      </footer>
    </main>
  );
}

// ── Tutorial page ────────────────────────────────────────────
export default async function Page({ params }: PageProps) {
  const { lang, slug } = await params;

  if (!slug || slug.length === 0) return <TutorialsList lang={lang} />;

  const page = tutorialsSource.getPage(slug, lang);
  if (!page) notFound();

  const MDX = page.data.body;

  return (
    <main className="min-h-screen" style={{ background: 'var(--ct-bg)', color: 'var(--ct-text)', fontFamily: 'var(--ct-font)' }}>
      <div className="border-b border-slate-200 dark:border-slate-700/60 px-4 py-3 sm:px-6">
        <div className="mx-auto max-w-7xl">
          <Link href={`/${lang}/tutorials`} className="inline-flex items-center gap-1.5 text-sm text-slate-400 transition-colors hover:text-indigo-600 dark:text-slate-500 dark:hover:text-indigo-400">
            <ArrowLeft className="h-3.5 w-3.5" />
            所有教程
          </Link>
        </div>
      </div>
      <div className="mx-auto w-full max-w-7xl px-4 py-10 sm:px-6 xl:px-8">
        <h1 className="text-3xl font-extrabold tracking-tight text-slate-900 dark:text-slate-100 sm:text-4xl mb-3">
          {page.data.title}
        </h1>
        {page.data.description && (
          <p className="text-lg text-slate-500 dark:text-slate-400 mb-10">{page.data.description}</p>
        )}
        <div className="prose prose-slate max-w-none dark:prose-invert
          prose-headings:font-extrabold prose-headings:tracking-tight
          prose-a:text-indigo-600 prose-a:no-underline hover:prose-a:underline dark:prose-a:text-indigo-400
          prose-code:rounded prose-code:bg-slate-100 prose-code:px-1 prose-code:py-0.5 prose-code:text-[0.875em] dark:prose-code:bg-slate-800
          prose-pre:p-0 prose-pre:bg-transparent prose-pre:border-0">
          <MDX components={getMDXComponents({ a: createRelativeLink(tutorialsSource, page) })} />
        </div>
      </div>
    </main>
  );
}

export function generateStaticParams() {
  return [
    { lang: 'cn', slug: undefined },
    ...tutorialsSource.getPages('cn').map((page) => ({
      lang: 'cn',
      slug: page.slugs.length > 0 ? page.slugs : undefined,
    })),
  ];
}

export async function generateMetadata({ params }: PageProps): Promise<Metadata> {
  const { lang, slug } = await params;
  if (!slug || slug.length === 0) return { title: '教程', description: '使用 A3S 构建 AI 智能体的手把手教程。' };
  const page = tutorialsSource.getPage(slug, lang);
  if (!page) notFound();
  return { title: page.data.title, description: page.data.description };
}
