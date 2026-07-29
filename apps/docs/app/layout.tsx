import type { ReactNode } from 'react';
import type { Metadata } from 'next';
import { ScrollToTop } from '@/components/scroll-to-top';
import './global.css';
import '@/components/home/styles/home-base.css';
import '@/components/home/styles/home-sections.css';
import '@/components/home/styles/home-responsive.css';

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev';

export const metadata: Metadata = {
  metadataBase: new URL(siteUrl),
  title: {
    default: 'A3S — Governed Agents. Composable Infrastructure.',
    template: '%s | A3S',
  },
  description:
    'A Rust-native platform for governed agents, local AI work, and composable infrastructure.',
  keywords: [
    'A3S',
    'agentic agents',
    'AI agent runtime',
    'TEE',
    'Trusted Execution Environment',
    'secure AI',
    'agent framework',
    'Rust',
    'Box',
    'MCP',
    'model context protocol',
    'LLM',
    'agent orchestration',
    'open source',
  ],
  authors: [{ name: 'A3S Lab', url: 'https://github.com/A3S-Lab' }],
  openGraph: {
    type: 'website',
    locale: 'en_US',
    url: siteUrl,
    siteName: 'A3S',
    title: 'A3S — Governed Agents. Composable Infrastructure.',
    description:
      'A Rust-native platform for governed agents, local AI work, and composable infrastructure.',
  },
  twitter: {
    card: 'summary_large_image',
    title: 'A3S — Governed Agents. Composable Infrastructure.',
    description:
      'A Rust-native platform for governed agents, local AI work, and composable infrastructure.',
  },
  robots: {
    index: true,
    follow: true,
    googleBot: {
      index: true,
      follow: true,
      'max-video-preview': -1,
      'max-image-preview': 'large',
      'max-snippet': -1,
    },
  },
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="anonymous" />
        <link
          href="https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500;700&display=swap"
          rel="stylesheet"
        />
        <script
          async
          src="https://www.googletagmanager.com/gtag/js?id=G-VWVNY8DWPG"
        />
        <script
          dangerouslySetInnerHTML={{
            __html: `
              window.dataLayer = window.dataLayer || [];
              function gtag(){dataLayer.push(arguments);}
              gtag('js', new Date());
              gtag('config', 'G-VWVNY8DWPG');
            `,
          }}
        />
      </head>
      <body className="flex flex-col min-h-screen">
        {children}
        <ScrollToTop />
      </body>
    </html>
  );
}
