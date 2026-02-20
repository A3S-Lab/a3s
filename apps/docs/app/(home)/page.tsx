import type { Metadata } from 'next';
import HomePage from '@/components/home-page';

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev';

export const metadata: Metadata = {
  alternates: {
    canonical: siteUrl,
    languages: {
      en: siteUrl,
      'zh-Hans': `${siteUrl}/cn`,
    },
  },
};

const jsonLd = {
  '@context': 'https://schema.org',
  '@type': 'SoftwareApplication',
  name: 'A3S',
  description:
    'Autonomous Agent System — runtime, TEE security, memory, tooling, and orchestration for production AI agents.',
  applicationCategory: 'DeveloperApplication',
  operatingSystem: 'Linux, macOS',
  url: siteUrl,
  author: {
    '@type': 'Organization',
    name: 'A3S Lab',
    url: 'https://github.com/A3S-Lab',
  },
  license: 'https://opensource.org/licenses/MIT',
  programmingLanguage: ['Rust', 'Python', 'TypeScript'],
  offers: {
    '@type': 'Offer',
    price: '0',
    priceCurrency: 'USD',
  },
};

export default function Page() {
  return (
    <>
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
      />
      <HomePage lang="en" />
    </>
  );
}
