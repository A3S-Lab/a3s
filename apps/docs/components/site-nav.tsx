import Link from 'next/link';
import { Github } from 'lucide-react';
import { LangDropdown } from '@/components/lang-dropdown';
import { ThemeToggle } from '@/components/theme-toggle';

interface SiteNavProps {
  lang?: string;
  section?: string; // e.g. 'Blog', 'Tutorials', 'Docs'
}

export function SiteNav({ lang = 'en', section }: SiteNavProps) {
  const isCn = lang === 'cn';
  const homeHref = isCn ? '/cn' : '/';
  const docsHref = isCn ? '/cn/docs/code' : '/docs/code';
  const tutorialsHref = isCn ? '/cn/tutorials' : '/tutorials';

  return (
    <nav className="sticky top-0 z-50 border-b border-slate-200 bg-white/80 backdrop-blur-md dark:border-slate-700/60 dark:bg-slate-900/80">
      <div className="mx-auto flex max-w-7xl items-center justify-between px-4 py-4 sm:px-6">
        {/* Logo */}
        <Link href={homeHref} className="flex items-center gap-2">
          <span className="bg-gradient-to-r from-indigo-600 to-violet-600 bg-clip-text text-xl font-extrabold tracking-tight text-transparent">
            A3S
          </span>
          {section && (
            <span className="text-sm font-medium text-slate-400 dark:text-slate-500">{section}</span>
          )}
        </Link>

        {/* Links */}
        <div className="flex items-center gap-1">
          <Link href={docsHref} className="rounded-lg px-3 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100">
            {isCn ? '文档' : 'Docs'}
          </Link>
          <Link href={tutorialsHref} className="rounded-lg px-3 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100">
            {isCn ? '教程' : 'Tutorials'}
          </Link>
          <Link href="/blog" className="rounded-lg px-3 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100">
            Blog
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
  );
}
