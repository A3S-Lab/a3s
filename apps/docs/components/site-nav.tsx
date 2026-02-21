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
            href="https://discord.gg/XVg6Hu6H"
            target="_blank"
            rel="noopener noreferrer"
            className="hidden items-center gap-1.5 rounded-lg px-3 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100 sm:flex"
          >
            <svg className="h-4 w-4" viewBox="0 0 24 24" fill="currentColor">
              <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z" />
            </svg>
            Discord
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
