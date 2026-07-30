import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { resolveLocalizedMdxHref } from '@/lib/localized-mdx-link';

const pages = new Map([
  ['cloud/v0.1.0/recovery', { url: '/docs/cloud/v0.1.0/recovery' }],
  ['cloud/v0.1.0/interfaces', { url: '/docs/cloud/v0.1.0/interfaces' }],
]);

const source = {
  getPage(slugs: string[] | undefined) {
    return slugs ? pages.get(slugs.join('/')) : undefined;
  },
};

describe('localized MDX links', () => {
  test('keeps sibling links inside a versioned index directory', () => {
    assert.equal(
      resolveLocalizedMdxHref(
        source,
        { path: 'cloud/v0.1.0/index.mdx', slugs: ['cloud', 'v0.1.0'] },
        './recovery',
        'cn',
      ),
      '/docs/cloud/v0.1.0/recovery',
    );
  });

  test('resolves sibling links from a non-index page and preserves fragments', () => {
    assert.equal(
      resolveLocalizedMdxHref(
        source,
        {
          path: 'cloud/v0.1.0/recovery.mdx',
          slugs: ['cloud', 'v0.1.0', 'recovery'],
        },
        './interfaces#rest',
        'en',
      ),
      '/en/docs/cloud/v0.1.0/interfaces#rest',
    );
  });

  test('normalizes absolute links through the selected locale', () => {
    assert.equal(
      resolveLocalizedMdxHref(
        source,
        { path: 'cloud/index.mdx', slugs: ['cloud'] },
        '/cn/docs/cloud',
        'en',
      ),
      '/en/docs/cloud',
    );
  });
});
