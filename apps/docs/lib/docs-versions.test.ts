import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  buildDocsVersionCatalog,
  docsVersionPath,
  matchDocsVersion,
} from '@/lib/docs-versions';

const catalog = buildDocsVersionCatalog([
  ['cloud'],
  ['cloud', 'architecture'],
  ['cloud', 'v0.1.0'],
  ['cloud', 'v0.1.0', 'architecture'],
  ['cloud', 'v0.1.0', 'recovery'],
  ['cloud', 'v0.2.0'],
  ['cloud', 'v0.2.0', 'architecture'],
  ['code'],
]);

describe('documentation version routing', () => {
  test('discovers versioned products directly from page slugs', () => {
    assert.deepEqual(catalog, [
      {
        product: 'cloud',
        versions: [
          { id: 'latest', topics: ['', 'architecture'] },
          { id: 'v0.2.0', topics: ['', 'architecture'] },
          { id: 'v0.1.0', topics: ['', 'architecture', 'recovery'] },
        ],
      },
    ]);
  });

  test('matches canonical and legacy localized version paths', () => {
    const release = matchDocsVersion('/docs/cloud/v0.1.0/recovery', catalog);
    assert.equal(release?.version.id, 'v0.1.0');
    assert.equal(release?.topic, 'recovery');

    const latest = matchDocsVersion('/cn/docs/cloud/architecture', catalog);
    assert.equal(latest?.version.id, 'latest');
    assert.equal(latest?.topic, 'architecture');
  });

  test('preserves a topic when the target version contains it', () => {
    assert.equal(
      docsVersionPath('/docs/cloud/v0.1.0/architecture', 'v0.2.0', catalog, 'cn'),
      '/docs/cloud/v0.2.0/architecture',
    );
  });

  test('falls back to the target version root when the topic is unavailable', () => {
    assert.equal(docsVersionPath('/docs/cloud/v0.1.0/recovery', 'latest', catalog, 'cn'),
      '/docs/cloud',
    );
  });

  test('keeps the selected language while switching versions', () => {
    assert.equal(docsVersionPath('/en/docs/cloud', 'v0.1.0', catalog, 'en'),
      '/en/docs/cloud/v0.1.0',
    );
  });
});
