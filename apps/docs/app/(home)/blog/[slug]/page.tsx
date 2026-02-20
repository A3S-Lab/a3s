import { blog } from '@/lib/blog';
import { getMDXComponents } from '@/mdx-components';
import { notFound } from 'next/navigation';
import Link from 'next/link';
import { ArrowLeft, Github } from 'lucide-react';
import { LangDropdown } from '@/components/lang-dropdown';
import { ThemeToggle } from '@/components/theme-toggle';
import type { Metadata } from 'next';

interface PostProps {
  params: Promise<{ slug: string }>;
}

function postSlug(path: string) {
  return path.replace(/^\//, '').replace(/\.mdx$/, '');
}

export function generateStaticParams() {
  return blog.map((post) => ({ slug: postSlug(post.info.path) }));
}

export async function generateMetadata({ params }: PostProps): Promise<Metadata> {
  const { slug } = await params;
  const post = blog.find((p) => postSlug(p.info.path) === slug);
  if (!post) notFound();
  return {
    title: post.title,
    description: post.description,
  };
}

export default async function BlogPost({ params }: PostProps) {
  const { slug } = await params;
  const post = blog.find((p) => postSlug(p.info.path) === slug);
  if (!post) notFound();

  const { body: MDX } = await post.load();

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
              href="/blog"
              className="rounded-lg px-3 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100"
            >
              ← All posts
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

      {/* ── Article ── */}
      <article className="mx-auto max-w-3xl px-4 py-16 sm:px-6 sm:py-20">
        {/* Meta */}
        <div className="mb-8">
          <Link
            href="/blog"
            className="mb-6 inline-flex items-center gap-1.5 text-sm text-slate-400 transition-colors hover:text-indigo-600 dark:text-slate-500 dark:hover:text-indigo-400"
          >
            <ArrowLeft className="h-3.5 w-3.5" />
            Back to blog
          </Link>
          <div className="mt-4 flex flex-wrap items-center gap-2">
            <span className="text-xs text-slate-400 dark:text-slate-500">
              {new Date(post.date).toISOString().slice(0, 10)}
            </span>
            {post.author && (
              <span className="text-xs text-slate-400 dark:text-slate-500">· {post.author}</span>
            )}
            {post.tags?.map((tag) => (
              <span
                key={tag}
                className="rounded-full bg-slate-100 px-2 py-0.5 text-[0.6875rem] font-medium text-slate-500 dark:bg-slate-700 dark:text-slate-400"
              >
                {tag}
              </span>
            ))}
          </div>
        </div>

        {/* Content */}
        <div className="prose prose-slate max-w-none dark:prose-invert
          prose-headings:font-extrabold prose-headings:tracking-tight
          prose-a:text-indigo-600 prose-a:no-underline hover:prose-a:underline
          dark:prose-a:text-indigo-400
          prose-code:rounded prose-code:bg-slate-100 prose-code:px-1 prose-code:py-0.5 prose-code:text-[0.875em]
          dark:prose-code:bg-slate-800
          prose-pre:rounded-xl prose-pre:border prose-pre:border-slate-200
          dark:prose-pre:border-slate-700
          prose-img:rounded-xl">
          <MDX components={getMDXComponents({})} />
        </div>

        {/* Footer nav */}
        <div className="mt-16 border-t border-slate-200 pt-8 dark:border-slate-700">
          <Link
            href="/blog"
            className="inline-flex items-center gap-2 rounded-full border border-slate-200 bg-white px-5 py-2.5 text-sm font-semibold text-slate-700 transition-all hover:border-indigo-300 hover:text-indigo-700 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300 dark:hover:border-indigo-600 dark:hover:text-indigo-300"
          >
            <ArrowLeft className="h-4 w-4" />
            All posts
          </Link>
        </div>
      </article>
    </main>
  );
}
