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
      <head>
        <link
          href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;700&display=swap"
          rel="stylesheet"
        />
        <script
          async
          src="https://www.googletagmanager.com/gtag/js?id=G-G1G5G0PTKD"
        />
        <script
          dangerouslySetInnerHTML={{
            __html: `
              window.dataLayer = window.dataLayer || [];
              function gtag(){dataLayer.push(arguments);}
              gtag('js', new Date());
              gtag('config', 'G-G1G5G0PTKD');
            `,
          }}
        />
      </head>
      <body className="flex flex-col min-h-screen">
        <RootProvider>{children}</RootProvider>
      </body>
    </html>
  );
}
