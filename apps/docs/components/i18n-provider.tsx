'use client';

import { RootProvider } from 'fumadocs-ui/provider/next';
import { usePathname, useRouter } from 'next/navigation';
import type { ReactNode } from 'react';

const locales = [
  { locale: 'en', name: 'English' },
  { locale: 'cn', name: '中文' },
];

function onLocaleChange(locale: string, pathname: string, router: ReturnType<typeof useRouter>) {
  // Strip any existing /en or /cn prefix, then re-prefix with the target locale
  const stripped = pathname.replace(/^\/(en|cn)(\/|$)/, '/').replace(/\/$/, '') || '/';
  router.push(`/${locale}${stripped === '/' ? '' : stripped}`);
}

export function I18nProvider({
  children,
  locale,
  translations,
}: {
  children: ReactNode;
  locale: string;
  translations?: Record<string, string>;
}) {
  const pathname = usePathname();
  const router = useRouter();

  return (
    <RootProvider
      i18n={{
        locale,
        locales,
        translations: translations as never,
        onLocaleChange: (v) => onLocaleChange(v, pathname, router),
      }}
    >
      {children}
    </RootProvider>
  );
}
