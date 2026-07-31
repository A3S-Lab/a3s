import type { ReactNode } from 'react';
import type { Metadata } from 'next';
import { ScrollToTop } from '@/components/scroll-to-top';
import './global.css';
import '@/components/home/styles/home-base.css';
import '@/components/home/styles/home-sections.css';
import '@/components/home/styles/architecture-atlas.css';
import '@/components/home/styles/home-responsive.css';

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev';
const socialImage = `${siteUrl.replace(/\/$/, '')}/opengraph-image`;
const defaultTitle = 'A3S — Agent 工具、工作流与运行时';
const defaultDescription =
  'A3S 包含 CLI、Code、Browser、Office、Flow、Runtime、Cloud 等独立项目，可按需安装并通过公开接口组合。';

export const metadata: Metadata = {
  metadataBase: new URL(siteUrl),
  title: {
    default: defaultTitle,
    template: '%s | A3S',
  },
  description: defaultDescription,
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
    locale: 'zh_CN',
    url: siteUrl,
    siteName: 'A3S',
    title: defaultTitle,
    description: defaultDescription,
    images: [
      {
        url: socialImage,
        width: 1200,
        height: 630,
        alt: 'A3S — Agent tools, workflows, and runtimes',
      },
    ],
  },
  twitter: {
    card: 'summary_large_image',
    title: defaultTitle,
    description: defaultDescription,
    images: [socialImage],
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
    <html lang="zh-CN" suppressHydrationWarning>
      <head>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link
          rel="preconnect"
          href="https://fonts.gstatic.com"
          crossOrigin="anonymous"
        />
        <link
          href="https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;700&display=swap"
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
