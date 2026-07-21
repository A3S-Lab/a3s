import { useMemoizedFn } from 'ahooks';
import { useEffect, useRef } from 'react';
import { codeApi } from '../../lib/api';
import { appState, formatApiError } from '../../state/app-state';

export function useMemoryController() {
  const requestRef = useRef<AbortController | null>(null);

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

  useEffect(
    () => () => {
      const request = requestRef.current;
      requestRef.current = null;
      request?.abort();
      appState.memoryRefreshing = false;
      if (appState.memoryPhase === 'loading' && appState.memoryData === null) {
        appState.memoryPhase = 'idle';
      }
    },
    []
  );

  return { loadMemory };
}
