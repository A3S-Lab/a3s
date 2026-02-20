import { RootProvider } from 'fumadocs-ui/provider/next';
import type { ReactNode } from 'react';

interface LangLayoutProps {
  children: ReactNode;
  params: Promise<{ lang: string }>;
}

export default async function LangLayout({ children, params }: LangLayoutProps) {
  const { lang } = await params;
  return (
    <RootProvider
      locale={lang}
      i18n={{
        locale: lang,
        locales: [
          { locale: 'en', name: 'English' },
          { locale: 'cn', name: '中文' },
        ],
        translations: lang === 'cn' ? {
          search: '搜索文档',
          toc: '本页目录',
          lastUpdate: '最后更新',
          chooseLanguage: '选择语言',
          nextPage: '下一页',
          previousPage: '上一页',
          editOnGithub: '在 GitHub 上编辑',
        } : undefined,
      }}
    >
      {children}
    </RootProvider>
  );
}
