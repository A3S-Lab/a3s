import { describe, expect, it } from 'vitest';
import type { PluginActivityCatalog, PluginActivityItem } from '../../types/api';
import { resolveActivityDocument } from './plugin-activity-document';

const revision = 'b'.repeat(64);
const documentUrl = `/api/v1/plugins/activities/science%3Aresearch/document?generation=2&revision=${revision}`;

const contribution: PluginActivityItem = {
  key: 'science:research',
  packageId: 'use/a3s/science',
  route: 'science',
  version: '1.2.3',
  enabled: true,
  id: 'research',
  title: '科研',
  description: 'Explore scientific sources.',
  icon: 'flask-conical',
  skill: 'a3s-use-science',
  order: 120,
  sha256: 'a'.repeat(64),
  mediaType: 'text/html',
  documentUrl,
};

const catalog: PluginActivityCatalog = {
  schemaVersion: 1,
  available: true,
  generation: 2,
  revision,
  items: [contribution],
};

describe('plugin Activity document identity', () => {
  it('accepts only the exact same-origin URL emitted for the catalog identity', () => {
    expect(resolveActivityDocument(catalog, contribution)).toEqual({
      key: contribution.key,
      generation: catalog.generation,
      revision: catalog.revision,
      url: documentUrl,
      token: `${contribution.key}:${catalog.generation}:${catalog.revision}`,
    });
  });

  it.each([
    ['missing URL', undefined],
    ['raw key', documentUrl.replace('science%3Aresearch', 'science:research')],
    ['wrong key', documentUrl.replace('science%3Aresearch', 'search%3Afind')],
    ['wrong generation', documentUrl.replace('generation=2', 'generation=3')],
    ['wrong revision', documentUrl.replace(revision, 'c'.repeat(64))],
    ['extra authority', `${documentUrl}&redirect=https://example.test`],
    ['absolute authority', `https://example.test${documentUrl}`],
  ])('rejects %s', (_case, candidate) => {
    expect(() => resolveActivityDocument(catalog, { ...contribution, documentUrl: candidate })).toThrow(
      'Activity document identity'
    );
  });

  it('rejects a document URL on a disabled contribution', () => {
    expect(() => resolveActivityDocument(catalog, { ...contribution, enabled: false })).toThrow(
      'disabled Activity contribution'
    );
  });
});
