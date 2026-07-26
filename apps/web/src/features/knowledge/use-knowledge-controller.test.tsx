import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { codeApi } from '../../lib/api';
import { appState } from '../../state/app-state';
import type { PersonalKnowledgeBase, PersonalKnowledgeBaseCatalog } from '../../types/api';
import { createKnowledgeState } from './knowledge-state';
import { useKnowledgeController } from './use-knowledge-controller';

const personal: PersonalKnowledgeBaseCatalog = {
  schemaVersion: 1,
  workspaceRoot: '/workspace',
  root: '/workspace/.a3s/kb/bases',
  items: [],
  total: 0,
  warnings: [],
};

describe('useKnowledgeController', () => {
  beforeEach(() => {
    Object.assign(appState, createKnowledgeState());
  });

  afterEach(() => {
    vi.restoreAllMocks();
    Object.assign(appState, createKnowledgeState());
  });

  it('loads only the user local knowledge-base catalog', async () => {
    vi.spyOn(codeApi, 'personalKnowledgeBases').mockResolvedValue(personal);
    const marketplace = vi.spyOn(codeApi, 'knowledgeMarketplace');
    const hook = renderHook(() => useKnowledgeController());

    await act(() => hook.result.current.refreshKnowledge());

    expect(appState.knowledgeStatus).toBe('ready');
    expect(appState.personalKnowledgeBases?.total).toBe(0);
    expect(marketplace).not.toHaveBeenCalled();
    hook.unmount();
  });

  it('picks and imports an Obsidian vault into the local catalog', async () => {
    const imported = importedBase();
    Object.assign(appState, { personalKnowledgeBases: personal, knowledgeStatus: 'ready' });
    vi.spyOn(codeApi, 'pickWorkspaceDirectory').mockResolvedValue({
      cancelled: false,
      path: '/Users/me/Research Vault',
    });
    vi.spyOn(codeApi, 'importPersonalKnowledgeBase').mockResolvedValue({ changed: true, knowledgeBase: imported });
    vi.spyOn(codeApi, 'personalKnowledgeBases').mockResolvedValue({ ...personal, items: [imported], total: 1 });
    const hook = renderHook(() => useKnowledgeController());

    let path: string | null = null;
    await act(async () => {
      path = await hook.result.current.pickKnowledgeBaseDirectory();
    });
    expect(path).toBe('/Users/me/Research Vault');

    const importedResults: PersonalKnowledgeBase[] = [];
    await act(async () => {
      const result = await hook.result.current.importKnowledgeBase({ path: path! });
      if (result) importedResults.push(result);
    });

    expect(importedResults[0].origin).toBe('imported');
    expect(codeApi.importPersonalKnowledgeBase).toHaveBeenCalledWith({
      path: '/Users/me/Research Vault',
      workspace: '/workspace',
    });
    expect(appState.personalKnowledgeBases?.items[0].id).toBe('research-vault');
    expect(appState.toast?.message).toBe('知识库已导入。');
    hook.unmount();
  });

  it('keeps the inline creator retryable when the service rejects a duplicate name', async () => {
    vi.spyOn(codeApi, 'createPersonalKnowledgeBase').mockRejectedValue(new Error('knowledge base already exists'));
    const hook = renderHook(() => useKnowledgeController());

    let result = true;
    await act(async () => {
      result = await hook.result.current.createKnowledgeBase({ name: 'Project Notes' });
    });

    expect(result).toBe(false);
    expect(appState.knowledgeOperationStatus).toBe('error');
    expect(appState.knowledgeOperationError).toBe('knowledge base already exists');
    hook.unmount();
  });

  it('keeps create and pin mutations in the active catalog workspace', async () => {
    const created = importedBase();
    Object.assign(appState, { personalKnowledgeBases: personal, knowledgeStatus: 'ready' });
    const create = vi
      .spyOn(codeApi, 'createPersonalKnowledgeBase')
      .mockResolvedValue({ changed: true, knowledgeBase: created });
    const pin = vi
      .spyOn(codeApi, 'setPersonalKnowledgeBasePinned')
      .mockResolvedValue({ changed: true, knowledgeBase: { ...created, pinned: false } });
    vi.spyOn(codeApi, 'personalKnowledgeBases').mockResolvedValue(personal);
    const hook = renderHook(() => useKnowledgeController());

    await act(async () => {
      expect(await hook.result.current.createKnowledgeBase({ name: 'Research Vault' })).toBe(true);
      expect(await hook.result.current.setPinned(created.id, false)).toBe(true);
    });

    expect(create).toHaveBeenCalledWith({ name: 'Research Vault', workspace: '/workspace' });
    expect(pin).toHaveBeenCalledWith(created.id, false, '/workspace');
    hook.unmount();
  });

  it('requests compilation in the catalog workspace and applies the queued state inline', async () => {
    const sourceReady = selectionBase();
    const queued = {
      ...sourceReady,
      compilation: { ...sourceReady.compilation!, phase: 'queued' as const, activeJobId: 'job-1' },
    };
    Object.assign(appState, {
      personalKnowledgeBases: { ...personal, items: [sourceReady], total: 1 },
      knowledgeStatus: 'ready',
      knowledgeWorkspace: '/fallback-workspace',
    });
    const requestCompilation = vi
      .spyOn(codeApi, 'requestKnowledgeCompilation')
      .mockResolvedValue({ changed: true, knowledgeBase: queued, job: null });
    vi.spyOn(codeApi, 'personalKnowledgeBases').mockResolvedValue({ ...personal, items: [queued], total: 1 });
    const hook = renderHook(() => useKnowledgeController());

    await act(async () => {
      expect(await hook.result.current.requestCompilation(sourceReady.id)).toBe(true);
    });

    expect(requestCompilation).toHaveBeenCalledWith(sourceReady.id, '/workspace');
    expect(appState.personalKnowledgeBases?.items[0].compilation?.phase).toBe('queued');
    hook.unmount();
  });

  it('updates smart-auto policy in the active knowledge workspace', async () => {
    const sourceReady = selectionBase();
    const automatic = {
      ...sourceReady,
      compilation: { ...sourceReady.compilation!, policy: 'smart_auto' as const },
    };
    Object.assign(appState, {
      personalKnowledgeBases: { ...personal, items: [sourceReady], total: 1 },
      knowledgeStatus: 'ready',
    });
    const setPolicy = vi
      .spyOn(codeApi, 'setKnowledgeCompilationPolicy')
      .mockResolvedValue({ changed: true, knowledgeBase: automatic, job: null });
    vi.spyOn(codeApi, 'personalKnowledgeBases').mockResolvedValue({ ...personal, items: [automatic], total: 1 });
    const hook = renderHook(() => useKnowledgeController());

    await act(async () => {
      expect(await hook.result.current.setCompilationPolicy(sourceReady.id, 'smart_auto')).toBe(true);
    });

    expect(setPolicy).toHaveBeenCalledWith(sourceReady.id, 'smart_auto', '/workspace');
    expect(appState.personalKnowledgeBases?.items[0].compilation?.policy).toBe('smart_auto');
    hook.unmount();
  });
});

function importedBase(): PersonalKnowledgeBase {
  return {
    id: 'research-vault',
    name: 'Research Vault',
    description: 'Imported from Obsidian.',
    origin: 'imported',
    marketplaceId: null,
    version: '1.0.0',
    pinned: true,
    createdAt: '2026-07-22T00:00:00Z',
    updatedAt: '2026-07-22T00:00:00Z',
    path: '/workspace/.a3s/kb/bases/research-vault',
    sourceCount: 2,
    conceptCount: 0,
    bytes: 1024,
  };
}

function selectionBase(): PersonalKnowledgeBase {
  return {
    ...importedBase(),
    id: 'research-pack',
    name: 'Research Pack',
    origin: 'selection',
    compilation: {
      policy: 'manual',
      phase: 'source_ready',
      sourceDigest: 'sha256:source',
      lastCompiledDigest: null,
      pendingChanges: true,
      activeJobId: null,
      lastRequestedAt: null,
      lastSucceededAt: null,
      lastFailedAt: null,
      lastError: null,
      pausedReason: null,
      compilerVersion: null,
      recompileRecommended: false,
      nextAutoCompileAt: null,
      stableWindowSeconds: 5,
      quietWindowSeconds: 30,
      minimumIntervalSeconds: 600,
    },
  };
}
