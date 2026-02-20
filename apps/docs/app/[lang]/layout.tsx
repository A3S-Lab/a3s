import type { ReactNode } from 'react';
import { I18nProvider } from '@/components/i18n-provider';

interface LangLayoutProps {
  children: ReactNode;
  params: Promise<{ lang: string }>;
}

export function generateStaticParams() {
  return [{ lang: 'cn' }];
}

const cnTranslations = {
  search: '搜索文档',
  toc: '本页目录',
  lastUpdate: '最后更新',
  chooseLanguage: '选择语言',
  nextPage: '下一页',
  previousPage: '上一页',
  editOnGithub: '在 GitHub 上编辑',
};

export default async function LangLayout({ children, params }: LangLayoutProps) {
  const { lang } = await params;
  return (
    <I18nProvider locale={lang} translations={lang === 'cn' ? cnTranslations : undefined}>
      {children}
    </I18nProvider>
  );
}
