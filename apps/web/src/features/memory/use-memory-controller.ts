import { useMemoizedFn } from 'ahooks';
import { useEffect, useRef } from 'react';
import { codeApi } from '../../lib/api';
import { appState, formatApiError, showToast } from '../../state/app-state';

export function useMemoryController() {
  const requestRef = useRef<AbortController | null>(null);
  const evolutionRequestRef = useRef<AbortController | null>(null);

  const loadMemory = useMemoizedFn(async (force = false) => {
    if (!force && (appState.memoryPhase === 'loading' || appState.memoryPhase === 'ready')) return;
    requestRef.current?.abort();
    const request = new AbortController();
    requestRef.current = request;
    const hasData = appState.memoryData !== null;
    appState.memoryError = null;
    appState.memoryRefreshing = hasData;
    if (!hasData) appState.memoryPhase = 'loading';
    try {
      const data = await codeApi.memory(request.signal);
      if (request.signal.aborted) return;
      appState.memoryData = data;
      appState.memoryPhase = 'ready';
      appState.memoryLastLoadedAt = Date.now();
      const selection = appState.memoryInspector;
      const selectionStillExists =
        selection?.kind === 'memory'
          ? data.entries.some((entry) => entry.id === selection.id)
          : selection?.kind === 'entity'
            ? data.graph.entities.some((entity) => entity.id === selection.id)
            : false;
      if (selection && !selectionStillExists) appState.memoryInspector = null;
    } catch (error) {
      if (request.signal.aborted) return;
      appState.memoryError = formatApiError(error);
      if (!hasData) appState.memoryPhase = 'error';
    } finally {
      if (requestRef.current === request) {
        requestRef.current = null;
        appState.memoryRefreshing = false;
      }
    }
  });

  const loadEvolution = useMemoizedFn(async (force = false) => {
    if (!force && (appState.evolutionPhase === 'loading' || appState.evolutionPhase === 'ready')) return;
    evolutionRequestRef.current?.abort();
    const request = new AbortController();
    evolutionRequestRef.current = request;
    const hasData = appState.evolutionData !== null;
    appState.evolutionError = null;
    appState.evolutionRefreshing = hasData;
    if (!hasData) appState.evolutionPhase = 'loading';
    try {
      const data = await codeApi.evolution(request.signal);
      if (request.signal.aborted) return;
      applyEvolutionOverview(data);
    } catch (error) {
      if (request.signal.aborted) return;
      appState.evolutionError = formatApiError(error);
      if (!hasData) appState.evolutionPhase = 'error';
    } finally {
      if (evolutionRequestRef.current === request) {
        evolutionRequestRef.current = null;
        appState.evolutionRefreshing = false;
      }
    }
  });

  const scanEvolution = useMemoizedFn(async () => {
    if (appState.evolutionRefreshing || appState.evolutionBusyId) return;
    evolutionRequestRef.current?.abort();
    const request = new AbortController();
    evolutionRequestRef.current = request;
    const hasData = appState.evolutionData !== null;
    appState.evolutionError = null;
    appState.evolutionRefreshing = hasData;
    if (!hasData) appState.evolutionPhase = 'loading';
    try {
      const response = await codeApi.scanEvolution(request.signal);
      if (request.signal.aborted) return;
      applyEvolutionOverview(response.overview);
      showToast(response.observed === 0 ? '没有发现新内容' : `找到 ${response.observed} 项内容`, 'success');
    } catch (error) {
      if (request.signal.aborted) return;
      appState.evolutionError = formatApiError(error);
      if (!hasData) appState.evolutionPhase = 'error';
    } finally {
      if (evolutionRequestRef.current === request) {
        evolutionRequestRef.current = null;
        appState.evolutionRefreshing = false;
      }
    }
  });

  const materializeEvolution = useMemoizedFn(async (id: string) => {
    await runEvolutionMutation(id, async () => {
      const response = await codeApi.materializeEvolution(id);
      const version = response.result.candidate.currentVersion ?? 0;
      const reload = response.result.requiresSessionReload ? '，当前对话已更新' : '';
      showToast(`已保存 v${version}${reload}`, 'success');
    });
  });

  const rejectEvolution = useMemoizedFn(async (id: string, reason?: string) => {
    await runEvolutionMutation(id, async () => {
      await codeApi.rejectEvolution(id, reason || 'Rejected during Web review');
      showToast('已忽略，可随时重新考虑', 'info');
    });
  });

  const reopenEvolution = useMemoizedFn(async (id: string) => {
    await runEvolutionMutation(id, async () => {
      await codeApi.reopenEvolution(id);
      showToast('已重新考虑', 'info');
    });
  });

  const rollbackEvolution = useMemoizedFn(async (id: string, targetVersion?: number) => {
    await runEvolutionMutation(id, async () => {
      const response = await codeApi.rollbackEvolution(id, targetVersion);
      const version = response.result.candidate.currentVersion;
      showToast(version == null ? '已撤销保存，需要时仍可恢复' : '已恢复所选版本', 'success');
    });
  });

  const runEvolutionMutation = useMemoizedFn(async (id: string, mutation: () => Promise<void>) => {
    if (appState.evolutionBusyId) return;
    appState.evolutionBusyId = id;
    appState.evolutionError = null;
    try {
      await mutation();
      await loadEvolution(true);
    } catch (error) {
      const message = formatApiError(error);
      appState.evolutionError = message;
      showToast(message, 'error');
    } finally {
      if (appState.evolutionBusyId === id) appState.evolutionBusyId = null;
    }
  });

  useEffect(
    () => () => {
      const request = requestRef.current;
      requestRef.current = null;
      request?.abort();
      const evolutionRequest = evolutionRequestRef.current;
      evolutionRequestRef.current = null;
      evolutionRequest?.abort();
      appState.memoryRefreshing = false;
      appState.evolutionRefreshing = false;
      if (appState.memoryPhase === 'loading' && appState.memoryData === null) {
        appState.memoryPhase = 'idle';
      }
      if (appState.evolutionPhase === 'loading' && appState.evolutionData === null) {
        appState.evolutionPhase = 'idle';
      }
    },
    []
  );

  return {
    loadMemory,
    loadEvolution,
    scanEvolution,
    materializeEvolution,
    rejectEvolution,
    reopenEvolution,
    rollbackEvolution,
  };
}

function applyEvolutionOverview(data: Awaited<ReturnType<typeof codeApi.evolution>>) {
  appState.evolutionData = data;
  appState.evolutionPhase = 'ready';
  appState.evolutionLastLoadedAt = Date.now();
  const selectedStillExists = data.candidates.some((candidate) => candidate.id === appState.evolutionSelectedId);
  if (!selectedStillExists) appState.evolutionSelectedId = data.candidates[0]?.id ?? null;
}
