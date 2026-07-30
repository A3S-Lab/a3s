import { DocsLayout } from 'fumadocs-ui/layouts/docs';
import type { ReactNode } from 'react';
import { source } from '@/lib/source';
import { baseOptions } from '@/lib/layout.shared';
import { buildDocsVersionCatalog } from '@/lib/docs-versions';
import type { Locale } from '@/lib/i18n';

export function DocsShell({ children, locale }: { children: ReactNode; locale: Locale }) {
  const versions = buildDocsVersionCatalog(
    source.getPages(locale).map((page) => page.slugs),
  );

  return (
    <DocsLayout tree={source.getPageTree(locale)} {...baseOptions(locale, versions)}>
      {children}
    </DocsLayout>
  );
}
