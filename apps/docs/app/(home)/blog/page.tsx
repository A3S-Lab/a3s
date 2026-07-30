import type { Metadata } from 'next';
import { BlogList } from '@/components/blog-list';
import { localizedUrl } from '@/lib/i18n';

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev';

export const metadata: Metadata = {
  title: '博客',
  description: 'A3S 团队的技术文章、版本说明与深度解析。',
  alternates: {
    canonical: localizedUrl(siteUrl, '/blog', 'cn'),
    languages: {
      'zh-CN': localizedUrl(siteUrl, '/blog', 'cn'),
      en: localizedUrl(siteUrl, '/blog', 'en'),
    },
  },
};

export default function BlogPage() {
  return <BlogList locale="cn" />;
}
