import { docs } from './.source/index.ts';
import { loader } from 'fumadocs-core/source';

console.log('docs:', docs);

const source = loader({
  source: docs,
  baseUrl: '/docs',
});

console.log('source:', source);
console.log('source.pageTree:', source.pageTree);
console.log('source keys:', Object.keys(source));
