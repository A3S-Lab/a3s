import type { Metadata } from 'next';
import { blog } from '@/lib/blog';
import Link from 'next/link';
import { ArrowRight, Github } from 'lucide-react';
import { LangDropdown } from '@/components/lang-dropdown';
import { ThemeToggle } from '@/components/theme-toggle';

export const metadata: Metadata = {
  title: 'Blog',
  description: 'Engineering articles, release notes, and deep dives from the A3S team.',
  openGraph: {
    title: 'A3S Blog',
    description: 'Engineering articles, release notes, and deep dives from the A3S team.',
  },
};

function postSlug(path: string) {
  return path.replace(/^\//, '').replace(/\.mdx$/, '');
}

export default async function BlogPage() {
  const posts = [...blog].sort(
    (a, b) => new Date(b.date).getTime() - new Date(a.date).getTime()
  );

  return (
    <main
      className="min-h-screen"
      style={{ background: 'var(--ct-bg)', fontFamily: 'var(--ct-font)', color: 'var(--ct-text)' }}
    >
      {/* ── Nav ── */}
      <nav className="sticky top-0 z-50 border-b border-slate-200 bg-white/80 backdrop-blur-md dark:border-slate-700/60 dark:bg-slate-900/80">
        <div className="mx-auto flex max-w-7xl items-center justify-between px-4 py-4 sm:px-6">
          <Link href="/" className="flex items-center gap-2">
            <span className="bg-gradient-to-r from-indigo-600 to-violet-600 bg-clip-text text-xl font-extrabold tracking-tight text-transparent">
              A3S
            </span>
            <span className="text-sm font-medium text-slate-400 dark:text-slate-500">Blog</span>
          </Link>
          <div className="flex items-center gap-1">
            <Link
              href="/docs/code"
              className="rounded-lg px-3 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100"
            >
              Docs
            </Link>
            <Link
              href="https://github.com/A3S-Lab"
              target="_blank"
              rel="noopener noreferrer"
              className="hidden items-center gap-1.5 rounded-lg px-3 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100 sm:flex"
            >
              <Github className="h-4 w-4" />
              GitHub
            </Link>
            <LangDropdown />
            <ThemeToggle />
          </div>
        </div>
      </nav>

      {/* ── Header ── */}
      <section className="px-4 py-16 sm:px-6 sm:py-20">
        <div className="mx-auto max-w-3xl">
          <span className="text-xs font-semibold uppercase tracking-widest text-indigo-500">A3S Lab</span>
          <h1 className="mt-2 text-4xl font-extrabold tracking-tight text-slate-900 dark:text-slate-100 sm:text-5xl">
            Blog
          </h1>
          <p className="mt-4 text-lg text-slate-500 dark:text-slate-400">
            Engineering articles, release notes, and deep dives from the A3S team.
          </p>
        </div>
      </section>

      {/* ── Post list ── */}
      <section className="px-4 pb-24 sm:px-6">
        <div className="mx-auto max-w-3xl space-y-4">
          {posts.map((post) => {
            const slug = postSlug(post.info.path);
            return (
              <Link
                key={slug}
                href={`/blog/${slug}`}
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

      {/* ── Footer ── */}
      <footer className="border-t border-slate-200 px-4 py-8 dark:border-slate-700/60 sm:px-6">
        <div className="mx-auto flex max-w-7xl items-center justify-between">
          <span className="bg-gradient-to-r from-indigo-600 to-violet-600 bg-clip-text text-sm font-bold text-transparent">
            A3S Lab
          </span>
          <Link
            href="/"
            className="text-sm text-slate-400 transition-colors hover:text-indigo-600 dark:text-slate-500 dark:hover:text-indigo-400"
          >
            ← Home
          </Link>
        </div>
      </footer>
    </main>
  );
}
