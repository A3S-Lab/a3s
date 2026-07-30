'use client';

import { GitBranch } from 'lucide-react';
import { usePathname, useRouter } from 'next/navigation';
import {
  docsVersionPath,
  latestDocsVersion,
  matchDocsVersion,
  type DocsProductVersions,
} from '@/lib/docs-versions';
import type { Locale } from '@/lib/i18n';

interface DocsVersionDropdownProps {
  catalog: DocsProductVersions[];
  locale: Locale;
}

export function DocsVersionDropdown({ catalog, locale }: DocsVersionDropdownProps) {
  const pathname = usePathname();
  const router = useRouter();
  const active = matchDocsVersion(pathname, catalog);

  if (!active || active.product.versions.length < 2) return null;

  const label = locale === 'cn' ? '文档版本' : 'Documentation version';
  const latestLabel = locale === 'cn' ? '最新版' : 'Latest';

  return (
    <label className="relative inline-flex min-w-0 items-center text-fd-muted-foreground">
      <span className="sr-only">{label}</span>
      <GitBranch aria-hidden="true" className="pointer-events-none absolute left-2 size-3.5" />
      <select
        aria-label={label}
        className="h-8 max-w-32 appearance-none rounded-md border border-fd-border bg-fd-secondary py-1 pl-7 pr-2 text-xs font-medium text-fd-secondary-foreground outline-none transition-colors hover:bg-fd-accent focus-visible:ring-2 focus-visible:ring-fd-ring"
        value={active.version.id}
        onChange={(event) => {
          const href = docsVersionPath(pathname, event.target.value, catalog, locale);
          if (href) router.push(href);
        }}
      >
        {active.product.versions.map((version) => (
          <option key={version.id} value={version.id}>
            {version.id === latestDocsVersion ? latestLabel : version.id}
          </option>
        ))}
      </select>
    </label>
  );
}
