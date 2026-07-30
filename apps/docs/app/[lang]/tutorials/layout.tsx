import type { ReactNode } from 'react';
import { notFound } from 'next/navigation';
import { isLocale } from '@/lib/i18n';

interface LayoutProps {
  children: ReactNode;
  params: Promise<{ lang: string }>;
}

export default async function Layout({ children, params }: LayoutProps) {
  const { lang } = await params;
  if (!isLocale(lang)) notFound();
  return children;
}
