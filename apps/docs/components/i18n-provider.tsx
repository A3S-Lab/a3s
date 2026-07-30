'use client';

import { RootProvider } from 'fumadocs-ui/provider/next';
import { usePathname, useRouter } from 'next/navigation';
import { useEffect, type ReactNode } from 'react';
import type { Translations } from 'fumadocs-ui/i18n';
import {
  htmlLanguage,
  localePath,
  resolveLocale,
  type Locale,
} from '@/lib/i18n';

const locales = [
  { locale: 'cn', name: '中文' },
  { locale: 'en', name: 'English' },
];

const translations: Record<Locale, Partial<Translations>> = {
  cn: {
    search: '搜索文档',
    searchNoResult: '没有找到结果',
    toc: '本页目录',
    tocNoHeadings: '本页没有目录',
    lastUpdate: '最后更新',
    chooseLanguage: '选择语言',
    nextPage: '下一页',
    previousPage: '上一页',
    chooseTheme: '选择主题',
    editOnGithub: '在 GitHub 上编辑',
  },
  en: {},
};

export function I18nProvider({
  children,
  locale,
}: {
  children: ReactNode;
  locale: string;
}) {
  const pathname = usePathname();
  const router = useRouter();
  const activeLocale = resolveLocale(locale);

  useEffect(() => {
    document.documentElement.lang = htmlLanguage(activeLocale);
  }, [activeLocale]);

  return (
    <RootProvider
      i18n={{
        locale: activeLocale,
        locales,
        translations: translations[activeLocale],
        onLocaleChange: (value) => {
          const nextLocale = resolveLocale(value);
          router.push(localePath(pathname, nextLocale));
        },
      }}
    >
      {children}
    </RootProvider>
  );
}
