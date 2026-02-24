import type { Metadata } from 'next';
import { blog } from '@/lib/blog';
import Link from 'next/link';
import { ArrowRight } from 'lucide-react';
import { SiteNav } from '@/components/site-nav';

export const metadata: Metadata = {
  title: 'Blog',
  description: 'A3S 团队的技术文章、版本说明与深度解析。',
};

interface PageProps {
  params: Promise<{ lang: string }>;
}

function postFilename(path: string) {
  // path is like "cn/some-post.mdx" → "some-post"
  return path.replace(/^[^/]+\//, '').replace(/\.mdx$/, '');
}

export default async function BlogPage({ params }: PageProps) {
  const { lang } = await params;
  const posts = [...blog]
    .filter((p) => p.info.path.startsWith(`${lang}/`))
    .sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());

  return (
    <main
      className="min-h-screen"
      style={{ background: 'var(--ct-bg)', fontFamily: 'var(--ct-font)', color: 'var(--ct-text)' }}
    >
      <SiteNav section="Blog" />

      <section className="px-4 py-16 sm:px-6 sm:py-20">
        <div className="mx-auto max-w-3xl">
          <span className="text-xs font-semibold uppercase tracking-widest text-indigo-500">A3S Lab</span>
          <h1 className="mt-2 text-4xl font-extrabold tracking-tight text-slate-900 dark:text-slate-100 sm:text-5xl">
            Blog
          </h1>
          <p className="mt-4 text-lg text-slate-500 dark:text-slate-400">
            A3S 团队的技术文章、版本说明与深度解析。
          </p>
        </div>
      </section>

      <section className="px-4 pb-24 sm:px-6">
        <div className="mx-auto max-w-3xl space-y-4">
          {posts.map((post) => {
            const slug = postFilename(post.info.path);
            return (
              <Link
                key={slug}
                href={`/${lang}/blog/${slug}`}
                className="module-card group flex flex-col gap-3 rounded-xl border border-slate-100 bg-white p-6 hover:-translate-y-0.5 dark:border-slate-700 dark:bg-slate-800/60"
              >
                <div className="flex items-start justify-between gap-4">
                  <div className="flex-1">
                    <div className="mb-2 flex flex-wrap items-center gap-2">
                      <span className="text-xs text-slate-400 dark:text-slate-500">
                        {new Date(post.date).toISOString().slice(0, 10)}
                      </span>
                      {post.tags?.map((tag) => (
                        <span
                          key={tag}
                          className="rounded-full bg-slate-100 px-2 py-0.5 text-[0.6875rem] font-medium text-slate-500 dark:bg-slate-700 dark:text-slate-400"
                        >
                          {tag}
                        </span>
                      ))}
                    </div>
                    <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
                      {post.title}
                    </h2>
                    <p className="mt-1.5 text-sm leading-relaxed text-slate-500 dark:text-slate-400">
                      {post.description}
                    </p>
                  </div>
                  <ArrowRight
                    className="mt-1 h-4 w-4 shrink-0 text-slate-300 transition-transform duration-200 group-hover:translate-x-1 group-hover:text-indigo-500 dark:text-slate-600 dark:group-hover:text-indigo-400"
                    strokeWidth={2}
                  />
                </div>
              </Link>
            );
          })}
        </div>
      </section>

      <footer className="border-t border-slate-200 px-4 py-8 dark:border-slate-700/60 sm:px-6">
        <div className="mx-auto flex max-w-7xl items-center justify-between">
          <span className="bg-gradient-to-r from-indigo-600 to-violet-600 bg-clip-text text-sm font-bold text-transparent">
            A3S Lab
          </span>
          <Link
            href={`/${lang}`}
            className="text-sm text-slate-400 transition-colors hover:text-indigo-600 dark:text-slate-500 dark:hover:text-indigo-400"
          >
            ← Home
          </Link>
        </div>
      </footer>
    </main>
  );
}
