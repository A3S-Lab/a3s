import { describe, expect, it, vi } from 'vitest';
import { WorkLocalFileExistsError } from './work-local-file-binding';
import {
  createWorkLocalArtifact,
  workLocalArtifactFileName,
  type WorkLocalArtifactCreateDependencies,
} from './work-local-artifact-create';

function dependencies(): WorkLocalArtifactCreateDependencies {
  return {
    createArtifact: vi.fn((templateId: string) => ({
      id: 'artifact-new',
      kind: templateId === 'blank-spreadsheet' ? 'spreadsheet' : 'document',
      title: 'Untitled',
      favorite: false,
      createdAt: 1,
      updatedAt: 1,
      lastOpenedAt: 1,
      revision: 1,
      content:
        templateId === 'blank-spreadsheet'
          ? { type: 'spreadsheet', sheets: [] }
          : { type: 'document', pageSize: 'a4', html: '<p></p>' },
    })),
    createBlob: vi.fn().mockResolvedValue(new Blob([Uint8Array.from([1, 2, 3])])),
    saveArtifact: vi.fn(async (artifact) => ({ ...artifact, revision: 2 })),
    purgeArtifact: vi.fn().mockResolvedValue(undefined),
    writeFile: vi.fn().mockResolvedValue({ fingerprint: 'sha256:new', size: 3 }),
  } as WorkLocalArtifactCreateDependencies;
}

describe('Work local Office artifact creation', () => {
  it('creates, serializes, persists, and binds a native file in the selected directory', async () => {
    const deps = dependencies();

    const result = await createWorkLocalArtifact('blank-spreadsheet', '/docs', 'Budget.xlsx', deps);

    expect(result.artifact).toMatchObject({ id: 'artifact-new', kind: 'spreadsheet', title: 'Budget', revision: 2 });
    expect(deps.saveArtifact).toHaveBeenCalledWith(expect.objectContaining({ title: 'Budget' }));
    expect(deps.writeFile).toHaveBeenCalledWith('/docs/Budget.xlsx', Uint8Array.from([1, 2, 3]));
    expect(result.binding).toEqual({
      artifactId: 'artifact-new',
      path: '/docs/Budget.xlsx',
      fingerprint: 'sha256:new',
      size: 3,
      updatedAt: expect.any(Number),
    });
  });

  it('purges the managed staging artifact when the local destination already exists', async () => {
    const deps = dependencies();
    vi.mocked(deps.writeFile).mockRejectedValue(new WorkLocalFileExistsError('/docs/Plan.docx'));

    await expect(createWorkLocalArtifact('blank-document', '/docs', 'Plan.docx', deps)).rejects.toBeInstanceOf(
      WorkLocalFileExistsError
    );

    expect(deps.purgeArtifact).toHaveBeenCalledWith('artifact-new');
  });

  it('requires a valid filename with the editor-native extension', () => {
    expect(workLocalArtifactFileName('Plan', 'document')).toBe('Plan.docx');
    expect(() => workLocalArtifactFileName('Plan.xlsx', 'document')).toThrow('必须保存为 .docx');
    expect(() => workLocalArtifactFileName('../Plan', 'document')).toThrow('有效的文件名');
  });
});
