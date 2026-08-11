import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { codeApi } from '../../lib/api';
import { appState } from '../../state/app-state';
import type { PluginActivityCatalog, PluginActivityItem } from '../../types/api';
import { createPluginsState } from './plugin-state';
import { usePluginController } from './use-plugin-controller';

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
  documentUrl: `/api/v1/plugins/activities/science%3Aresearch/document?generation=2&revision=${'b'.repeat(64)}`,
};

const catalog: PluginActivityCatalog = {
  schemaVersion: 1,
  available: true,
  generation: 2,
  revision: 'b'.repeat(64),
  items: [contribution],
};

describe('usePluginController', () => {
  beforeEach(() => {
    window.location.hash = '';
    Object.assign(appState, createPluginsState(), {
      activeProduct: 'work',
      pluginCatalog: catalog,
      pluginCatalogStatus: 'ready',
      composerValue: '',
      composerSkills: [],
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    Object.assign(appState, createPluginsState());
  });

  it('rejects an invalid executable document identity without replacing the current catalog', async () => {
    const invalidCatalog: PluginActivityCatalog = {
      ...catalog,
      generation: 3,
      revision: 'c'.repeat(64),
      items: [contribution],
    };
    vi.spyOn(codeApi, 'pluginActivities').mockResolvedValue(invalidCatalog);
    const hook = renderHook(() => usePluginController());

    await waitFor(() => expect(appState.pluginCatalogStatus).toBe('error'));
    expect(appState.pluginCatalog).toEqual(catalog);
    expect(appState.pluginCatalogError).toContain('Activity document identity');
    hook.unmount();
  });

  it('hands reviewed context and the same-package Skill back to the unified work session', () => {
    vi.spyOn(codeApi, 'pluginActivities').mockResolvedValue(catalog);
    appState.activeProduct = 'plugin';
    appState.activePluginKey = contribution.key;
    const hook = renderHook(() => usePluginController());

    act(() => {
      hook.result.current.proposeContext({
        sourceKey: contribution.key,
        sourceGeneration: catalog.generation,
        sourceRevision: catalog.revision,
        sourceDocumentUrl: contribution.documentUrl!,
        title: 'Review research context',
        summary: 'Compare recent CRISPR evidence.',
        prompt: 'Compare the selected studies and identify uncertainty.',
        fields: [{ label: 'Source', value: 'PubMed' }],
        usePackageSkill: true,
      });
    });
    act(() => hook.result.current.acceptContextProposal());

    expect(appState.activeProduct).toBe('work');
    expect(window.location.hash).toBe('#home');
    expect(appState.composerValue).toContain('[Reviewed plugin context: 科研]');
    expect(appState.composerValue).toContain('- Source: PubMed');
    expect(appState.composerValue).toContain('Compare the selected studies and identify uncertainty.');
    expect(appState.composerSkills).toEqual(['a3s-use-science']);
    expect(appState.pluginContextProposal).toBeNull();
    hook.unmount();
  });

  it('hands general-discipline context to the current session without attaching the biomedical package Skill', () => {
    vi.spyOn(codeApi, 'pluginActivities').mockResolvedValue(catalog);
    appState.activeProduct = 'plugin';
    appState.activePluginKey = contribution.key;
    const hook = renderHook(() => usePluginController());

    act(() => {
      hook.result.current.proposeContext({
        sourceKey: contribution.key,
        sourceGeneration: catalog.generation,
        sourceRevision: catalog.revision,
        sourceDocumentUrl: contribution.documentUrl!,
        title: 'Review research context',
        summary: 'Assess a software engineering question.',
        prompt: 'Compare the selected software engineering evidence.',
        fields: [{ label: 'Discipline', value: 'Computer Science' }],
        usePackageSkill: false,
      });
    });
    act(() => hook.result.current.acceptContextProposal());

    expect(appState.composerValue).toContain('- Discipline: Computer Science');
    expect(appState.composerSkills).toEqual([]);
    expect(appState.pluginContextProposal).toBeNull();
    hook.unmount();
  });

  it('drops a pending proposal if its package is disabled before review completes', () => {
    vi.spyOn(codeApi, 'pluginActivities').mockResolvedValue(catalog);
    const hook = renderHook(() => usePluginController());

    act(() => {
      hook.result.current.proposeContext({
        sourceKey: contribution.key,
        sourceGeneration: catalog.generation,
        sourceRevision: catalog.revision,
        sourceDocumentUrl: contribution.documentUrl!,
        title: 'Review research context',
        summary: 'Pending context.',
        prompt: 'This must not reach Code.',
        fields: [],
        usePackageSkill: true,
      });
      appState.pluginCatalog = {
        ...catalog,
        items: [{ ...contribution, enabled: false, documentUrl: undefined }],
      };
      hook.result.current.acceptContextProposal();
    });

    expect(appState.composerValue).toBe('');
    expect(appState.composerSkills).toEqual([]);
    expect(appState.pluginContextProposal).toBeNull();
    hook.unmount();
  });

  it('drains a pending proposal when the same Activity advances to a new Registry generation', async () => {
    const activities = vi.spyOn(codeApi, 'pluginActivities').mockResolvedValue(catalog);
    const hook = renderHook(() => usePluginController());
    await waitFor(() => expect(activities).toHaveBeenCalledOnce());

    act(() => {
      hook.result.current.proposeContext({
        sourceKey: contribution.key,
        sourceGeneration: catalog.generation,
        sourceRevision: catalog.revision,
        sourceDocumentUrl: contribution.documentUrl!,
        title: 'Review research context',
        summary: 'Pending context from the old document.',
        prompt: 'This must be drained on upgrade.',
        fields: [],
        usePackageSkill: true,
      });
    });
    expect(appState.pluginContextProposal).not.toBeNull();

    const nextRevision = 'c'.repeat(64);
    const nextCatalog: PluginActivityCatalog = {
      ...catalog,
      generation: 3,
      revision: nextRevision,
      items: [
        {
          ...contribution,
          version: '2.0.0',
          documentUrl: `/api/v1/plugins/activities/science%3Aresearch/document?generation=3&revision=${nextRevision}`,
        },
      ],
    };
    activities.mockResolvedValue(nextCatalog);
    await act(() => hook.result.current.refreshActivities(true));

    expect(appState.pluginCatalog).toEqual(nextCatalog);
    expect(appState.pluginContextProposal).toBeNull();
    hook.unmount();
  });

  it('captures a registry baseline and waits for an installed contribution before reporting success', async () => {
    const emptyCatalog: PluginActivityCatalog = {
      ...catalog,
      generation: 1,
      revision: 'a'.repeat(64),
      items: [],
    };
    const activities = vi
      .spyOn(codeApi, 'pluginActivities')
      .mockResolvedValueOnce(emptyCatalog)
      .mockResolvedValueOnce(emptyCatalog)
      .mockResolvedValue(catalog);
    const marketplace = vi.spyOn(codeApi, 'pluginMarketplace').mockResolvedValue({
      schemaVersion: 1,
      verifiedAt: '2026-07-22T00:00:00Z',
      registries: [],
      items: [],
    });
    vi.spyOn(codeApi, 'applyPluginOperation').mockResolvedValue({
      planDigest: 'd'.repeat(64),
      operations: [{ component: contribution.packageId, changed: true, message: 'Installed.' }],
    });
    appState.pluginCatalog = { ...emptyCatalog, revision: '' };
    appState.pluginOperationReview = {
      request: { action: 'install', componentId: contribution.packageId, version: contribution.version },
      plan: {
        dryRun: true,
        planSchemaVersion: 1,
        planCommand: `a3s install ${contribution.packageId} --dry-run`,
        planDigest: 'd'.repeat(64),
        plans: [
          {
            component: contribution.packageId,
            action: 'install',
            source: 'registry:replacement',
            mutates: true,
            message: 'Install the research plugin.',
          },
        ],
      },
    };
    const hook = renderHook(() => usePluginController());
    await waitFor(() => expect(activities).toHaveBeenCalledOnce());

    await act(() => hook.result.current.applyReviewedOperation());

    expect(activities.mock.calls.length).toBeGreaterThanOrEqual(3);
    expect(marketplace).toHaveBeenCalledOnce();
    expect(appState.pluginCatalog).toEqual(catalog);
    expect(appState.pluginOperationReview).toBeNull();
    expect(appState.toast?.message).toBe('科研插件已安装并启用，可从市场或活动栏打开。');
    hook.unmount();
  });

  it('converges Marketplace state after uninstall removes the live contribution', async () => {
    const removedCatalog: PluginActivityCatalog = {
      ...catalog,
      generation: 3,
      revision: 'c'.repeat(64),
      items: [],
    };
    const activities = vi
      .spyOn(codeApi, 'pluginActivities')
      .mockResolvedValueOnce(catalog)
      .mockResolvedValueOnce(catalog)
      .mockResolvedValue(removedCatalog);
    const marketplace = vi.spyOn(codeApi, 'pluginMarketplace').mockResolvedValue({
      schemaVersion: 1,
      verifiedAt: '2026-07-22T00:00:00Z',
      registries: [],
      items: [],
    });
    vi.spyOn(codeApi, 'applyPluginOperation').mockResolvedValue({
      planDigest: 'd'.repeat(64),
      operations: [{ component: contribution.packageId, changed: true, message: 'Uninstalled.' }],
    });
    appState.activeProduct = 'plugin';
    appState.activePluginKey = contribution.key;
    appState.pluginContextProposal = {
      sourceKey: contribution.key,
      sourceGeneration: catalog.generation,
      sourceRevision: catalog.revision,
      sourceDocumentUrl: contribution.documentUrl!,
      title: 'Pending research context',
      summary: 'Pending.',
      prompt: 'Must be discarded.',
      fields: [],
      usePackageSkill: true,
    };
    appState.pluginOperationReview = {
      request: { action: 'uninstall', componentId: contribution.packageId },
      plan: {
        dryRun: true,
        planSchemaVersion: 1,
        planCommand: `a3s uninstall ${contribution.packageId} --dry-run`,
        planDigest: 'd'.repeat(64),
        plans: [
          {
            component: contribution.packageId,
            action: 'uninstall',
            source: 'installed',
            mutates: true,
            message: 'Uninstall the research plugin.',
          },
        ],
      },
    };
    const hook = renderHook(() => usePluginController());
    await waitFor(() => expect(activities).toHaveBeenCalledOnce());

    await act(() => hook.result.current.applyReviewedOperation());

    expect(marketplace).toHaveBeenCalledOnce();
    expect(appState.pluginCatalog).toEqual(removedCatalog);
    expect(appState.pluginContextProposal).toBeNull();
    expect(appState.pluginOperationReview).toBeNull();
    expect(appState.activeProduct).toBe('work');
    expect(appState.toast?.message).toBe('科研插件已卸载。');
    hook.unmount();
  });
});
