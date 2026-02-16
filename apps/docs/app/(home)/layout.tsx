import { HomeLayout } from 'fumadocs-ui/layouts/home';
import { baseOptions } from '@/lib/layout.shared';
import type { ReactNode } from 'react';

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <div className="term-home-wrapper">
      <HomeLayout {...baseOptions()}>{children}</HomeLayout>
    </div>
  );
}
