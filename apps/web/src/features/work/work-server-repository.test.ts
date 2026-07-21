import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createWorkArtifact } from './work-templates';
import type { WorkArtifact } from './work-types';

describe('Work server repository', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.unstubAllGlobals();
    vi.stubGlobal('indexedDB', undefined);
    localStorage.clear();
  });

  it('migrates the legacy browser library once and then saves with the server revision', async () => {
    const legacy = createWorkArtifact('blank-document');
    localStorage.setItem('a3s-work.artifacts.v1', JSON.stringify([legacy]));
    let serverArtifact: WorkArtifact | null = null;
    const requests: Array<{ path: string; method: string; body?: unknown }> = [];
    const fetch = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const path = String(input);
      const method = init?.method ?? 'GET';
      const body = typeof init?.body === 'string' ? (JSON.parse(init.body) as unknown) : undefined;
      requests.push({ path, method, body });
      if (path.startsWith('/api/v1/work/library')) {
        return jsonResponse({
          artifacts: serverArtifact ? [serverArtifact] : [],
          folders: [],
          limits: { artifactBytes: 1024, sourceBytes: 2048, historyEntries: 50 },
          storage: 'server',
        });
      }
      if (method === 'PUT' && path.includes('/artifacts/')) {
        const request = body as { artifact: WorkArtifact; expectedRevision: number };
        expect(request.expectedRevision).toBe(serverArtifact?.revision ?? 0);
        serverArtifact = structuredClone(request.artifact);
        return jsonResponse(serverArtifact);
      }
      return new Response(null, { status: 404 });
    });
    vi.stubGlobal('fetch', fetch);
    const repository = await import('./work-repository');

    const library = await repository.loadWorkLibrary();
    expect(library.storage).toBe('server');
    expect(library.artifacts).toMatchObject([{ id: legacy.id }]);

    const changed = { ...library.artifacts[0], title: '服务器年度计划', revision: 2 };
    await repository.saveWorkArtifact(changed);

    const artifactWrites = requests.filter(
      (request) => request.method === 'PUT' && request.path.includes('/artifacts/')
    );
    expect(artifactWrites).toHaveLength(2);
    expect(artifactWrites.map((request) => (request.body as { expectedRevision: number }).expectedRevision)).toEqual([
      0, 1,
    ]);
  });

  it('uploads imported source bytes without JSON encoding', async () => {
    const artifact = createWorkArtifact('blank-document');
    let serverArtifact = artifact;
    const fetch = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const path = String(input);
      if (path.startsWith('/api/v1/work/library')) {
        return jsonResponse({
          artifacts: [serverArtifact],
          folders: [],
          limits: { artifactBytes: 1024, sourceBytes: 2048, historyEntries: 50 },
          storage: 'server',
        });
      }
      if (path.includes('/source')) {
        expect(init?.body).toBeInstanceOf(File);
        expect(new Headers(init?.headers).get('Content-Type')).toBe('text/plain');
        serverArtifact = {
          ...serverArtifact,
          revision: 2,
          source: { name: 'brief notes.txt', contentType: 'text/plain', size: 5, updatedAt: 2 },
        };
        return jsonResponse(serverArtifact);
      }
      return new Response(null, { status: 404 });
    });
    vi.stubGlobal('fetch', fetch);
    const repository = await import('./work-repository');
    await repository.loadWorkLibrary();

    const saved = await repository.saveWorkSource(
      artifact,
      new File(['notes'], 'brief notes.txt', { type: 'text/plain' })
    );

    expect(saved.source?.name).toBe('brief notes.txt');
    expect(fetch.mock.calls.at(-1)?.[0]).toBe(
      `/api/v1/work/artifacts/${artifact.id}/source?expectedRevision=1&fileName=brief%20notes.txt`
    );
  });

  it('reports a disconnected server save instead of claiming a local cache write succeeded', async () => {
    const artifact = createWorkArtifact('blank-document');
    const fetch = vi.fn(async (input: RequestInfo | URL) => {
      if (String(input).startsWith('/api/v1/work/library')) {
        return jsonResponse({
          artifacts: [artifact],
          folders: [],
          limits: { artifactBytes: 1024, sourceBytes: 2048, historyEntries: 50 },
          storage: 'server',
        });
      }
      throw new TypeError('connection closed');
    });
    vi.stubGlobal('fetch', fetch);
    const repository = await import('./work-repository');
    await repository.loadWorkLibrary();

    await expect(
      repository.saveWorkArtifact({ ...artifact, title: 'Unsaved server edit', revision: 2 })
    ).rejects.toThrow('connection closed');
    expect(JSON.parse(localStorage.getItem('a3s-work.artifacts.v1') ?? '[]')).toMatchObject([
      { title: artifact.title, revision: 1 },
    ]);
  });
});

function jsonResponse(value: unknown): Response {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { 'Content-Type': 'application/json' },
  });
}
