import { RootProvider } from 'fumadocs-ui/provider/next';
import type { ReactNode } from 'react';
import './global.css';

export const metadata = {
  title: {
    default: 'A3S Documentation',
    template: '%s | A3S Docs',
  },
  description: 'Documentation for the A3S ecosystem — Box, Code, Gateway, SafeClaw',
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className="flex flex-col min-h-screen">
        <RootProvider>{children}</RootProvider>
      </body>
    </html>
  );
}
