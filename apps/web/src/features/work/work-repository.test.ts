import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  deleteWorkArtifact,
  listWorkArtifacts,
  readWorkSourceBlob,
  saveWorkArtifact,
  saveWorkSource,
} from './work-repository';
import { createWorkArtifact } from './work-templates';

describe('Work artifact repository', () => {
  beforeEach(() => {
    vi.stubGlobal('indexedDB', undefined);
    localStorage.clear();
  });

  it('persists revisions and removes artifacts in the fallback store', async () => {
    const artifact = createWorkArtifact('blank-document');
    await saveWorkArtifact(artifact);

    artifact.title = '年度计划';
    artifact.revision = 2;
    artifact.lastOpenedAt += 100;
    await saveWorkArtifact(artifact);

    expect(await listWorkArtifacts()).toMatchObject([{ id: artifact.id, title: '年度计划', revision: 2 }]);
    await deleteWorkArtifact(artifact.id);
    expect(await listWorkArtifacts()).toEqual([]);
  });

  it('ignores malformed fallback records instead of breaking the library', async () => {
    localStorage.setItem('a3s-work.artifacts.v1', JSON.stringify([{ id: 1 }, null]));
    await expect(listWorkArtifacts()).resolves.toEqual([]);
  });

  it('keeps imported source bytes in the local compatibility cache', async () => {
    const artifact = await saveWorkArtifact(createWorkArtifact('blank-document'));
    const source = new File(['source bytes'], 'source.txt', { type: 'text/plain' });
    const saved = await saveWorkSource(artifact, source);

    expect(saved.source).toMatchObject({
      name: 'source.txt',
      contentType: 'text/plain',
      size: source.size,
    });
    await expect((await readWorkSourceBlob(saved)).text()).resolves.toBe('source bytes');
  });
});
