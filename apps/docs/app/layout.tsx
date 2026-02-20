import type { ReactNode } from 'react';
import type { Metadata } from 'next';
import './global.css';

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev';

export const metadata: Metadata = {
  metadataBase: new URL(siteUrl),
  title: {
    default: 'A3S – Autonomous Agent System',
    template: '%s | A3S',
  },
  description:
    'A3S is an open-source Autonomous Agent System — runtime, TEE security, memory, tooling, and orchestration for production AI agents.',
  keywords: [
    'A3S',
    'autonomous agents',
    'AI agent runtime',
    'TEE',
    'Trusted Execution Environment',
    'secure AI',
    'agent framework',
    'Rust',
    'SafeClaw',
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
    title: 'A3S – Autonomous Agent System',
    description:
      'Open-source runtime, TEE security, memory, and orchestration for production AI agents.',
    images: [{ url: '/og.png', width: 1200, height: 630, alt: 'A3S – Autonomous Agent System' }],
  },
  twitter: {
    card: 'summary_large_image',
    title: 'A3S – Autonomous Agent System',
    description:
      'Open-source runtime, TEE security, memory, and orchestration for production AI agents.',
    images: ['/og.png'],
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
      </body>
    </html>
  );
}
