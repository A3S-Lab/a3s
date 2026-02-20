import HomePage from '@/components/home-page';

export default async function Page({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params;
  return <HomePage lang={lang === 'cn' ? 'cn' : 'en'} />;
}
