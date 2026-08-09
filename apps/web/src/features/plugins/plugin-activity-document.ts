import type { PluginActivityCatalog, PluginActivityItem } from '../../types/api';

export interface PluginActivityDocumentIdentity {
  key: string;
  generation: number;
  revision: string;
  url: string;
  token: string;
}

const LOWER_SHA256 = /^[0-9a-f]{64}$/;

export function resolveActivityDocument(
  catalog: PluginActivityCatalog,
  contribution: PluginActivityItem
): PluginActivityDocumentIdentity {
  if (!contribution.enabled) {
    throw new Error('A disabled Activity contribution cannot publish a document URL.');
  }
  if (catalog.generation <= 0 || !LOWER_SHA256.test(catalog.revision)) {
    throw new Error('Activity document identity has an invalid Registry generation or revision.');
  }
  const expected =
    `/api/v1/plugins/activities/${encodeURIComponent(contribution.key)}/document` +
    `?generation=${catalog.generation}&revision=${catalog.revision}`;
  if (contribution.documentUrl !== expected) {
    throw new Error('Activity document identity does not match the current Registry catalog.');
  }
  return {
    key: contribution.key,
    generation: catalog.generation,
    revision: catalog.revision,
    url: expected,
    token: `${contribution.key}:${catalog.generation}:${catalog.revision}`,
  };
}

export function validateActivityCatalogDocuments(catalog: PluginActivityCatalog): void {
  for (const contribution of catalog.items) {
    if (contribution.enabled) {
      resolveActivityDocument(catalog, contribution);
    } else if (contribution.documentUrl !== undefined) {
      throw new Error('A disabled Activity contribution cannot publish a document URL.');
    }
  }
}
