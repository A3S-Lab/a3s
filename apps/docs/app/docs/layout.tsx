import { source } from '@/lib/source';
import { DocsLayout } from 'fumadocs-ui/layouts/docs';
import { I18nProvider } from '@/components/i18n-provider';
import { baseOptions } from '@/lib/layout.shared';
import type { ReactNode } from 'react';

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <I18nProvider locale="en">
      <DocsLayout tree={source.getPageTree('en')} {...baseOptions('en')}>
        {children}
      </DocsLayout>
    </I18nProvider>
  );
}
