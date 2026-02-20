import { tutorials as tutorialsCollection } from 'fumadocs-mdx:collections/server';
import { loader } from 'fumadocs-core/source';
import { i18n } from '@/lib/i18n';

export const tutorialsSource = loader({
  baseUrl: '/tutorials',
  source: tutorialsCollection.toFumadocsSource(),
  i18n,
});
