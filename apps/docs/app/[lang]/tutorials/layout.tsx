import { I18nProvider } from '@/components/i18n-provider';
import type { ReactNode } from 'react';

interface LayoutProps {
  children: ReactNode;
  params: Promise<{ lang: string }>;
}

export default async function Layout({ children, params }: LayoutProps) {
  const { lang } = await params;
  return (
    <I18nProvider locale={lang}>
      {children}
    </I18nProvider>
  );
}
