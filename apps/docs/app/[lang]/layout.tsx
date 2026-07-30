import type { ReactNode } from 'react';
import { I18nProvider } from '@/components/i18n-provider';
import { notFound } from 'next/navigation';
import { isLocale, locales } from '@/lib/i18n';

interface LangLayoutProps {
  children: ReactNode;
  params: Promise<{ lang: string }>;
}

export function generateStaticParams() {
  return locales.map((lang) => ({ lang }));
}

export default async function LangLayout({ children, params }: LangLayoutProps) {
  const { lang } = await params;
  if (!isLocale(lang)) notFound();
  return <I18nProvider locale={lang}>{children}</I18nProvider>;
}
