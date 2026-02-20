import { source } from '@/lib/source';
import { DocsLayout } from 'fumadocs-ui/layouts/docs';
import { I18nProvider } from '@/components/i18n-provider';
import { baseOptions } from '@/lib/layout.shared';
import type { ReactNode } from 'react';

interface LayoutProps {
  children: ReactNode;
  params: Promise<{ lang: string }>;
}

export default async function Layout({ children, params }: LayoutProps) {
  const { lang } = await params;
  return (
    <I18nProvider locale={lang}>
      <DocsLayout tree={source.getPageTree(lang)} {...baseOptions(lang)}>
        {children}
      </DocsLayout>
    </I18nProvider>
  );
}
