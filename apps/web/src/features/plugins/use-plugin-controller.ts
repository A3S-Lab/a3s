import { useMemoizedFn } from 'ahooks';
import { useEffect, useRef } from 'react';
import { codeApi } from '../../lib/api';
import { appendTaskInstruction, appState, formatApiError, navigateTask, showToast } from '../../state/app-state';
import type { PluginActivityCatalog, PluginActivityItem, PluginOperationRequest } from '../../types/api';
import { validateActivityCatalogDocuments } from './plugin-activity-document';
import type { PluginContextProposal } from './plugin-state';

const ACTIVITY_POLL_MS = 2_500;

export function usePluginController() {
  const activitySequence = useRef(0);
  const marketplaceSequence = useRef(0);
  const marketplaceAbort = useRef<AbortController | null>(null);

  const applyCatalog = useMemoizedFn((catalog: PluginActivityCatalog) => {
    validateActivityCatalogDocuments(catalog);
    const items = [...catalog.items].sort(
      (left, right) =>
        left.order - right.order || left.title.localeCompare(right.title) || left.key.localeCompare(right.key)
    );
    appState.pluginCatalog = { ...catalog, items };
    appState.pluginCatalogStatus = 'ready';
    appState.pluginCatalogError = null;
    if (appState.pluginContextProposal && !contributionForProposal(appState.pluginContextProposal)) {
      appState.pluginContextProposal = null;
    }
    if (
      appState.activeProduct === 'plugin' &&
      (!appState.activePluginKey || !items.some((item) => item.key === appState.activePluginKey && item.enabled))
    ) {
      navigateTask('conversation');
      showToast('该插件已卸载或停用，已返回工作台。', 'info');
    }
  });

  const refreshActivities = useMemoizedFn(async (silent = false) => {
    const sequence = ++activitySequence.current;
    if (!silent || appState.pluginCatalogStatus === 'idle') appState.pluginCatalogStatus = 'loading';
    try {
      const catalog = await codeApi.pluginActivities();
      if (sequence !== activitySequence.current) return;
      applyCatalog(catalog);
    } catch (error) {
      if (sequence !== activitySequence.current) return;
      appState.pluginCatalogStatus = 'error';
      appState.pluginCatalogError = formatApiError(error);
    }
  });

  const refreshMarketplace = useMemoizedFn(async () => {
    marketplaceAbort.current?.abort();
    const controller = new AbortController();
    marketplaceAbort.current = controller;
    const sequence = ++marketplaceSequence.current;
    appState.pluginMarketplaceStatus = 'loading';
    appState.pluginMarketplaceError = null;
    try {
      const marketplace = await codeApi.pluginMarketplace(controller.signal);
      if (sequence !== marketplaceSequence.current) return;
      appState.pluginMarketplace = marketplace;
      appState.pluginMarketplaceStatus = 'ready';
    } catch (error) {
      if (controller.signal.aborted || sequence !== marketplaceSequence.current) return;
      appState.pluginMarketplaceStatus = 'error';
      appState.pluginMarketplaceError = formatApiError(error);
    }
  });

  const planOperation = useMemoizedFn(async (request: PluginOperationRequest) => {
    appState.pluginOperationStatus = 'loading';
    appState.pluginOperationError = null;
    appState.pluginOperationReview = null;
    try {
      const plan = await codeApi.planPluginOperation(request);
      appState.pluginOperationReview = { request, plan };
      appState.pluginOperationStatus = 'ready';
    } catch (error) {
      appState.pluginOperationStatus = 'error';
      appState.pluginOperationError = formatApiError(error);
    }
  });

  const applyReviewedOperation = useMemoizedFn(async () => {
    const review = appState.pluginOperationReview;
    if (!review) return;
    appState.pluginOperationStatus = 'loading';
    appState.pluginOperationError = null;
    try {
      await refreshActivities(true);
      const previousRevision = appState.pluginCatalog.revision;
      const result = await codeApi.applyPluginOperation({
        ...review.request,
        planDigest: review.plan.planDigest,
      });
      appState.pluginOperationReview = null;
      appState.pluginOperationStatus = 'ready';
      const changed = result.operations.some((operation) => operation.changed);
      await refreshActivities(true);
      if (changed) {
        await refreshMarketplaceAfterRevisionSettles(previousRevision, refreshActivities, refreshMarketplace);
      } else {
        await refreshMarketplace();
      }
      showToast(pluginOperationSuccessMessage(review.request, changed), 'success');
    } catch (error) {
      appState.pluginOperationStatus = 'error';
      appState.pluginOperationError = formatApiError(error);
    }
  });

  const setPackageEnabled = useMemoizedFn(async (componentId: string, enabled: boolean) => {
    appState.pluginOperationStatus = 'loading';
    appState.pluginOperationError = null;
    try {
      await refreshActivities(true);
      const previousRevision = appState.pluginCatalog.revision;
      await codeApi.setPluginPackageEnabled(componentId, enabled);
      appState.pluginOperationStatus = 'ready';
      await refreshActivities(true);
      await refreshMarketplaceAfterRevisionSettles(previousRevision, refreshActivities, refreshMarketplace);
      showToast(enabled ? '插件已启用。' : '插件已停用。', 'success');
    } catch (error) {
      appState.pluginOperationStatus = 'error';
      appState.pluginOperationError = formatApiError(error);
    }
  });

  const proposeContext = useMemoizedFn((proposal: PluginContextProposal) => {
    if (!contributionForProposal(proposal)) return;
    appState.pluginContextProposal = proposal;
  });

  const dismissContextProposal = useMemoizedFn(() => {
    appState.pluginContextProposal = null;
  });

  const acceptContextProposal = useMemoizedFn(() => {
    const proposal = appState.pluginContextProposal;
    if (!proposal) return;
    const contribution = contributionForProposal(proposal);
    if (!contribution) {
      appState.pluginContextProposal = null;
      return;
    }
    const fields = proposal.fields.map((field) => `- ${field.label}: ${field.value}`).join('\n');
    const context = [
      `[Reviewed plugin context: ${contribution.title}]`,
      proposal.summary,
      fields,
      '[/Reviewed plugin context]',
      proposal.prompt,
    ]
      .filter(Boolean)
      .join('\n\n');
    appendTaskInstruction(context);
    if (proposal.usePackageSkill && !appState.composerSkills.includes(contribution.skill)) {
      appState.composerSkills = [...appState.composerSkills, contribution.skill];
    }
    appState.pluginContextProposal = null;
    navigateTask('conversation');
    showToast(
      proposal.usePackageSkill
        ? `已将审核后的上下文和 ${contribution.skill} Skill 加入当前会话。`
        : '已将审核后的科研上下文加入当前会话。',
      'success'
    );
  });

  const dismissOperationReview = useMemoizedFn(() => {
    if (appState.pluginOperationStatus !== 'loading') appState.pluginOperationReview = null;
  });

  useEffect(() => {
    void refreshActivities();
    const interval = window.setInterval(() => {
      if (document.visibilityState === 'visible') void refreshActivities(true);
    }, ACTIVITY_POLL_MS);
    return () => {
      window.clearInterval(interval);
      marketplaceAbort.current?.abort();
    };
  }, [refreshActivities]);

  return {
    refreshActivities,
    refreshMarketplace,
    planOperation,
    applyReviewedOperation,
    dismissOperationReview,
    setPackageEnabled,
    proposeContext,
    dismissContextProposal,
    acceptContextProposal,
  };
}

function contributionForProposal(proposal: PluginContextProposal): PluginActivityItem | undefined {
  const catalog = appState.pluginCatalog;
  if (catalog.generation !== proposal.sourceGeneration || catalog.revision !== proposal.sourceRevision) {
    return undefined;
  }
  return catalog.items.find(
    (item) => item.key === proposal.sourceKey && item.enabled && item.documentUrl === proposal.sourceDocumentUrl
  );
}

async function refreshUntilRevisionChanges(
  previousRevision: string,
  refresh: (silent?: boolean) => Promise<void>
): Promise<void> {
  for (let attempt = 0; attempt < 6; attempt += 1) {
    if (appState.pluginCatalog.revision !== previousRevision) return;
    await delay(400 * (attempt + 1));
    await refresh(true);
  }
}

async function refreshMarketplaceAfterRevisionSettles(
  previousRevision: string,
  refreshActivities: (silent?: boolean) => Promise<void>,
  refreshMarketplace: () => Promise<void>
): Promise<void> {
  await refreshUntilRevisionChanges(previousRevision, refreshActivities);
  await refreshMarketplace();
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

function pluginOperationSuccessMessage(request: PluginOperationRequest, changed: boolean): string {
  if (!changed) return '插件已处于目标状态。';
  const isScience = request.componentId === 'use/a3s/science';
  if (request.action === 'install') {
    return isScience ? '科研插件已安装并启用，可从市场或活动栏打开。' : '插件已安装并启用。';
  }
  if (request.action === 'uninstall') return isScience ? '科研插件已卸载。' : '插件已卸载。';
  return isScience ? '科研插件已升级。' : '插件已升级。';
}

export type PluginActions = ReturnType<typeof usePluginController>;
