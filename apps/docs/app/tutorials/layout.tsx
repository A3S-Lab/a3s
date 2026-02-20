import { I18nProvider } from '@/components/i18n-provider';
import type { ReactNode } from 'react';

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <I18nProvider locale="en">
      {children}
    </I18nProvider>
  );
}
