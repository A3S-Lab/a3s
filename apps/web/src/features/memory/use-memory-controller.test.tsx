import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { codeApi } from '../../lib/api';
import { appState } from '../../state/app-state';
import { evolutionTestData } from './evolution-test-data';
import { createMemoryState } from './memory-state';
import { memoryTestData } from './memory-test-data';
import { useMemoryController } from './use-memory-controller';

describe('useMemoryController', () => {
  beforeEach(() => {
    Object.assign(appState, createMemoryState());
  });

  afterEach(() => {
    vi.restoreAllMocks();
    Object.assign(appState, createMemoryState());
  });

  it('loads the initial overview and removes a selection that no longer exists', async () => {
    const deferred = createDeferred<ReturnType<typeof memoryTestData>>();
    vi.spyOn(codeApi, 'memory').mockReturnValue(deferred.promise);
    appState.memoryInspector = { kind: 'memory', id: 'removed-memory' };
    const hook = renderHook(() => useMemoryController());

    let request!: Promise<void>;
    act(() => {
      request = hook.result.current.loadMemory();
    });
    expect(appState.memoryPhase).toBe('loading');
    expect(appState.memoryError).toBeNull();

    deferred.resolve(memoryTestData());
    await act(() => request);

    expect(appState.memoryPhase).toBe('ready');
    expect(appState.memoryData?.entries).toHaveLength(3);
    expect(appState.memoryLastLoadedAt).not.toBeNull();
    expect(appState.memoryInspector).toBeNull();
    hook.unmount();
  });

  it('preserves stale data and exposes a retryable error when refresh fails', async () => {
    const current = memoryTestData();
    Object.assign(appState, {
      memoryPhase: 'ready',
      memoryData: current,
      memoryLastLoadedAt: 100,
    });
    vi.spyOn(codeApi, 'memory').mockRejectedValue(new Error('memory service unavailable'));
    const hook = renderHook(() => useMemoryController());

    await act(() => hook.result.current.loadMemory(true));

    expect(appState.memoryPhase).toBe('ready');
    expect(appState.memoryData).toEqual(current);
    expect(appState.memoryError).toBe('memory service unavailable');
    expect(appState.memoryLastLoadedAt).toBe(100);
    expect(appState.memoryRefreshing).toBe(false);
    hook.unmount();
  });

  it('lets only the newest forced refresh settle shared loading state', async () => {
    Object.assign(appState, {
      memoryPhase: 'ready',
      memoryData: memoryTestData(),
    });
    const second = createDeferred<ReturnType<typeof memoryTestData>>();
    const requests: AbortSignal[] = [];
    vi.spyOn(codeApi, 'memory')
      .mockImplementationOnce((signal) => {
        requests.push(signal as AbortSignal);
        return rejectWhenAborted(signal as AbortSignal);
      })
      .mockImplementationOnce((signal) => {
        requests.push(signal as AbortSignal);
        return second.promise;
      });
    const hook = renderHook(() => useMemoryController());

    let firstRequest!: Promise<void>;
    let secondRequest!: Promise<void>;
    act(() => {
      firstRequest = hook.result.current.loadMemory(true);
      secondRequest = hook.result.current.loadMemory(true);
    });
    await act(() => firstRequest);

    expect(requests[0].aborted).toBe(true);
    expect(appState.memoryRefreshing).toBe(true);

    second.resolve(memoryTestData());
    await act(() => secondRequest);
    expect(appState.memoryRefreshing).toBe(false);
    expect(appState.memoryPhase).toBe('ready');
    hook.unmount();
  });

  it('aborts an in-flight initial load and restores an idle phase on unmount', async () => {
    let signal: AbortSignal | undefined;
    vi.spyOn(codeApi, 'memory').mockImplementation((requestSignal) => {
      signal = requestSignal;
      return rejectWhenAborted(requestSignal as AbortSignal);
    });
    const hook = renderHook(() => useMemoryController());

    let request!: Promise<void>;
    act(() => {
      request = hook.result.current.loadMemory();
    });
    expect(appState.memoryPhase).toBe('loading');

    hook.unmount();
    await request;

    expect(signal?.aborted).toBe(true);
    expect(appState.memoryPhase).toBe('idle');
    expect(appState.memoryRefreshing).toBe(false);
  });

  it('loads the evolution catalog and selects the first available candidate', async () => {
    const data = evolutionTestData();
    vi.spyOn(codeApi, 'evolution').mockResolvedValue(data);
    appState.evolutionSelectedId = 'missing-candidate';
    const hook = renderHook(() => useMemoryController());

    await act(() => hook.result.current.loadEvolution());

    expect(appState.evolutionPhase).toBe('ready');
    expect(appState.evolutionData?.revision).toBe(8);
    expect(appState.evolutionSelectedId).toBe(data.candidates[0].id);
    expect(appState.evolutionLastLoadedAt).not.toBeNull();
    hook.unmount();
  });

  it('rescans memory signals and applies the returned catalog without a second request', async () => {
    const data = evolutionTestData();
    vi.spyOn(codeApi, 'scanEvolution').mockResolvedValue({ observed: 7, overview: data });
    const evolution = vi.spyOn(codeApi, 'evolution');
    const hook = renderHook(() => useMemoryController());

    await act(() => hook.result.current.scanEvolution());

    expect(appState.evolutionPhase).toBe('ready');
    expect(appState.evolutionData?.candidates).toHaveLength(3);
    expect(appState.evolutionRefreshing).toBe(false);
    expect(appState.toast?.message).toBe('找到 7 项内容');
    expect(evolution).not.toHaveBeenCalled();
    hook.unmount();
  });

  it('materializes a candidate, then refreshes the shared catalog', async () => {
    const data = evolutionTestData();
    const candidate = data.candidates[1];
    vi.spyOn(codeApi, 'materializeEvolution').mockResolvedValue({
      result: { candidate, requiresSessionReload: true },
      rebuiltSessions: [
        {
          sessionId: 'session-one',
          workspace: '/tmp/a3s',
          skillDirCount: 2,
          builtinSkillActive: true,
          capabilitySkillActive: false,
          runtimeToolActive: false,
        },
      ],
    });
    vi.spyOn(codeApi, 'evolution').mockResolvedValue(data);
    const hook = renderHook(() => useMemoryController());

    await act(() => hook.result.current.materializeEvolution(candidate.id));

    expect(codeApi.materializeEvolution).toHaveBeenCalledWith(candidate.id);
    expect(codeApi.evolution).toHaveBeenCalled();
    expect(appState.evolutionBusyId).toBeNull();
    expect(appState.toast?.message).toBe('已保存，当前对话已更新');
    hook.unmount();
  });

  it('passes the review reason when rejecting and exposes mutation errors without losing data', async () => {
    const data = evolutionTestData();
    Object.assign(appState, {
      evolutionPhase: 'ready',
      evolutionData: data,
    });
    vi.spyOn(codeApi, 'rejectEvolution').mockRejectedValue(new Error('candidate is already materialized'));
    const hook = renderHook(() => useMemoryController());

    await act(() => hook.result.current.rejectEvolution('candidate-one', 'Not reusable'));

    expect(codeApi.rejectEvolution).toHaveBeenCalledWith('candidate-one', 'Not reusable');
    expect(appState.evolutionData).toEqual(data);
    expect(appState.evolutionError).toBe('candidate is already materialized');
    expect(appState.evolutionBusyId).toBeNull();
    hook.unmount();
  });
});

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function rejectWhenAborted(signal: AbortSignal): Promise<never> {
  return new Promise((_, reject) => {
    signal.addEventListener('abort', () => reject(new DOMException('Aborted', 'AbortError')), { once: true });
  });
}
