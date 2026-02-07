import { docs } from '@/.source';
import { loader } from 'fumadocs-core/source';
import { createMDXSource } from 'fumadocs-mdx';

export const source = loader({
  source: createMDXSource(docs, []),
  baseUrl: '/docs',
});
