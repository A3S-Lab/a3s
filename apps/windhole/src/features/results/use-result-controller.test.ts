import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { demoTasks } from '../../data/demo-tasks';
import { benchApi } from '../../lib/api';
import { createRunCampaignSnapshot, createRunSortieSnapshot, labState } from '../../state/lab-state';
import type { BenchRunResult } from '../../types/bench';
import { loadSortieManifest } from '../bench/sortie-manifest-store';
import { DEFAULT_HANGAR_ROSTER } from '../hangar/hangar-configuration';
import { useResultController } from './use-result-controller';

beforeEach(() => {
  window.localStorage.clear();
  labState.connection = { mode: 'live', message: 'Connected' };
  labState.results = { runId: '', loading: false };
  labState.run = { stage: 'idle' };
  labState.campaign = { generation: 0, status: 'idle', members: [] };
  labState.catalog.tasks = demoTasks.map((task) => ({
    ...task,
    tags: task.tags ? [...task.tags] : undefined,
  }));
  labState.catalog.selectedTaskId = demoTasks[0].id;
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('useResultController', () => {
  it('loads the explicitly requested Run ID and never falls back to latest', async () => {
    const requested = completedResult('run-campaign-wing');
    const resultRequest = vi.spyOn(benchApi, 'result').mockResolvedValue(requested);
    const latestRequest = vi.spyOn(benchApi, 'latestResult').mockResolvedValue(completedResult('run-latest'));
    const controller = renderHook(() => useResultController());

    await act(async () => {
      await controller.result.current.loadResult(requested.run_id);
    });

    expect(resultRequest).toHaveBeenCalledOnce();
    expect(resultRequest).toHaveBeenCalledWith('run-campaign-wing');
    expect(latestRequest).not.toHaveBeenCalled();
    expect(labState.results.record).toEqual(requested);
    expect(labState.results.runId).toBe('run-campaign-wing');
  });

  it('keeps the explicit latest-result action on the Bench latest endpoint', async () => {
    const latest = completedResult('run-latest');
    const resultRequest = vi.spyOn(benchApi, 'result');
    const latestRequest = vi.spyOn(benchApi, 'latestResult').mockResolvedValue(latest);
    const controller = renderHook(() => useResultController());

    await act(async () => {
      await controller.result.current.loadLatest();
    });

    expect(latestRequest).toHaveBeenCalledOnce();
    expect(resultRequest).not.toHaveBeenCalled();
    expect(labState.results.record).toEqual(latest);
    expect(labState.results.runId).toBe('run-latest');
    expect(labState.results.loading).toBe(false);
  });

  it('rejects a response whose Run ID does not match the requested campaign member', async () => {
    vi.spyOn(benchApi, 'result').mockResolvedValue(completedResult('run-other-aircraft'));
    const controller = renderHook(() => useResultController());

    await act(async () => {
      await controller.result.current.loadResult('run-expected-aircraft');
    });

    expect(labState.results.record).toBeUndefined();
    expect(labState.results.runId).toBe('run-expected-aircraft');
    expect(labState.results.error).toBe(
      'Bench 战报 Run ID 不匹配：期望 run-expected-aircraft，实际 run-other-aircraft。'
    );
  });

  it('keeps the newer campaign member result when an older request returns last', async () => {
    const firstResponse = deferred<BenchRunResult>();
    const secondResponse = deferred<BenchRunResult>();
    vi.spyOn(benchApi, 'result').mockImplementation((runId) => {
      if (runId === 'run-first') return firstResponse.promise;
      if (runId === 'run-second') return secondResponse.promise;
      throw new Error(`Unexpected Run ID ${runId}`);
    });
    const latestRequest = vi.spyOn(benchApi, 'latestResult');
    const controller = renderHook(() => useResultController());

    let firstLoad!: Promise<void>;
    let secondLoad!: Promise<void>;
    act(() => {
      firstLoad = controller.result.current.loadResult('run-first');
      secondLoad = controller.result.current.loadResult('run-second');
    });
    expect(labState.results.runId).toBe('run-second');
    expect(labState.results.loading).toBe(true);

    secondResponse.resolve(completedResult('run-second'));
    await act(async () => {
      await secondLoad;
    });
    expect(labState.results.record?.run_id).toBe('run-second');
    expect(labState.results.runId).toBe('run-second');
    expect(labState.results.loading).toBe(false);

    firstResponse.resolve(completedResult('run-first'));
    await act(async () => {
      await firstLoad;
    });

    expect(labState.results.record?.run_id).toBe('run-second');
    expect(labState.results.runId).toBe('run-second');
    expect(labState.results.loading).toBe(false);
    expect(latestRequest).not.toHaveBeenCalled();
  });

  it('does not clear loading when a stale request finishes while the newer request is pending', async () => {
    const firstResponse = deferred<BenchRunResult>();
    const secondResponse = deferred<BenchRunResult>();
    vi.spyOn(benchApi, 'result').mockImplementation((runId) =>
      runId === 'run-first' ? firstResponse.promise : secondResponse.promise
    );
    const controller = renderHook(() => useResultController());

    let firstLoad!: Promise<void>;
    let secondLoad!: Promise<void>;
    act(() => {
      firstLoad = controller.result.current.loadResult('run-first');
      secondLoad = controller.result.current.loadResult('run-second');
    });

    firstResponse.resolve(completedResult('run-first'));
    await act(async () => {
      await firstLoad;
    });

    expect(labState.results.record).toBeUndefined();
    expect(labState.results.runId).toBe('run-second');
    expect(labState.results.loading).toBe(true);

    secondResponse.resolve(completedResult('run-second'));
    await act(async () => {
      await secondLoad;
    });
    expect(labState.results.record?.run_id).toBe('run-second');
    expect(labState.results.loading).toBe(false);
  });

  it('invalidates an in-flight result request when the user edits the Run ID', async () => {
    const response = deferred<BenchRunResult>();
    vi.spyOn(benchApi, 'result').mockReturnValue(response.promise);
    const controller = renderHook(() => useResultController());

    let load!: Promise<void>;
    act(() => {
      load = controller.result.current.loadResult('run-before-edit');
      controller.result.current.setRunId('run-user-is-typing');
    });
    expect(labState.results.runId).toBe('run-user-is-typing');
    expect(labState.results.loading).toBe(false);

    response.resolve(completedResult('run-before-edit'));
    await act(async () => {
      await load;
    });

    expect(labState.results.runId).toBe('run-user-is-typing');
    expect(labState.results.record).toBeUndefined();
  });

  it('opens an interrupted single run by its exact Run ID and never falls back to latest', async () => {
    const runId = 'run-restored-single';
    const record = completedResult(runId);
    setInterruptedSingleRun(runId, { completedAt: undefined });
    const resultRequest = vi.spyOn(benchApi, 'result').mockResolvedValue(record);
    const latestRequest = vi.spyOn(benchApi, 'latestResult');
    const controller = renderHook(() => useResultController());

    await act(async () => {
      await controller.result.current.openCurrentRun();
    });

    expect(resultRequest).toHaveBeenCalledOnce();
    expect(resultRequest).toHaveBeenCalledWith(runId);
    expect(latestRequest).not.toHaveBeenCalled();
    expect(labState.workspace).toBe('results');
    expect(labState.run).toMatchObject({
      runId,
      stage: 'completed',
      result: record,
    });
    expect(labState.run.completedAt).toEqual(expect.any(String));
    expect(labState.run.trackingStatus).toBeUndefined();
    expect(labState.run.trackingStoppedAt).toBeUndefined();
    expect(labState.run.error).toBeUndefined();
    expect(loadSortieManifest(runId)?.task.id).toBe(demoTasks[0].id);
  });

  it('rejects a terminal single-run result whose Task does not match the frozen sortie', async () => {
    const runId = 'run-single-wrong-task';
    const originalCompletedAt = '2026-07-17T00:01:00.000Z';
    setInterruptedSingleRun(runId, { completedAt: originalCompletedAt });
    vi.spyOn(benchApi, 'result').mockResolvedValue({
      ...completedResult(runId),
      task_id: demoTasks[1].id,
    });
    const controller = renderHook(() => useResultController());

    await act(async () => {
      await controller.result.current.loadResult(runId);
    });

    expect(labState.results.record).toBeUndefined();
    expect(labState.results.error).toContain('Task 归属不匹配');
    expect(labState.run).toMatchObject({
      runId,
      stage: 'judging',
      trackingStatus: 'tracking_stopped',
      trackingStoppedAt: '2026-07-17T00:02:00.000Z',
      completedAt: originalCompletedAt,
    });
    expect(labState.run.result).toBeUndefined();
    expect(loadSortieManifest(runId)).toBeUndefined();
  });

  it('rebinds a terminal Task Lock result to an exact task already present in the catalog', async () => {
    const runId = 'run-locked-catalog-task';
    const resolvedTask = demoTasks[1];
    setInterruptedSingleRun(runId, { locked: true });
    vi.spyOn(benchApi, 'result').mockResolvedValue({
      ...completedResult(runId),
      task_id: resolvedTask.id,
    });
    const taskRequest = vi.spyOn(benchApi, 'task');
    const controller = renderHook(() => useResultController());

    await act(async () => {
      await controller.result.current.loadResult(runId);
    });

    expect(taskRequest).not.toHaveBeenCalled();
    expect(labState.run).toMatchObject({
      runId,
      stage: 'completed',
      sortie: { task: { id: resolvedTask.id }, input: { locked: true } },
    });
    expect(labState.catalog.selectedTaskId).toBe(resolvedTask.id);
    expect(labState.results.sortie?.task.id).toBe(resolvedTask.id);
    expect(loadSortieManifest(runId)?.task.id).toBe(resolvedTask.id);
  });

  it('loads and binds an exact Task Lock task that is absent from the catalog', async () => {
    const runId = 'run-locked-fetched-task';
    const resolvedTask = {
      ...demoTasks[1],
      id: 'task-resolved-from-lock',
      path: '/bench/tasks/task-resolved-from-lock',
      name: 'Resolved Task Lock mission',
    };
    labState.catalog.tasks = [{ ...demoTasks[0] }];
    setInterruptedSingleRun(runId, { locked: true });
    vi.spyOn(benchApi, 'result').mockResolvedValue({
      ...completedResult(runId),
      task_id: resolvedTask.id,
    });
    const taskRequest = vi.spyOn(benchApi, 'task').mockResolvedValue({ task: resolvedTask });
    const controller = renderHook(() => useResultController());

    await act(async () => {
      await controller.result.current.loadResult(runId);
    });

    expect(taskRequest).toHaveBeenCalledWith(resolvedTask.id, true);
    expect(labState.catalog.tasks.some((task) => task.id === resolvedTask.id)).toBe(true);
    expect(labState.catalog.selectedTaskId).toBe(resolvedTask.id);
    expect(labState.run.sortie?.task.id).toBe(resolvedTask.id);
    expect(loadSortieManifest(runId)?.task.id).toBe(resolvedTask.id);
  });

  it('fails closed when an exact Task Lock task cannot be loaded', async () => {
    const runId = 'run-locked-task-unavailable';
    const resolvedTaskId = 'task-unavailable-after-lock';
    labState.catalog.tasks = [{ ...demoTasks[0] }];
    setInterruptedSingleRun(runId, { locked: true });
    vi.spyOn(benchApi, 'result').mockResolvedValue({
      ...completedResult(runId),
      task_id: resolvedTaskId,
    });
    vi.spyOn(benchApi, 'task').mockRejectedValue(new Error('Task catalog unavailable'));
    const controller = renderHook(() => useResultController());

    await act(async () => {
      await controller.result.current.loadResult(runId);
    });

    expect(labState.results.record).toBeUndefined();
    expect(labState.results.error).toBe('Task catalog unavailable');
    expect(labState.run).toMatchObject({
      runId,
      stage: 'judging',
      trackingStatus: 'tracking_stopped',
      sortie: { task: { id: demoTasks[0].id }, input: { locked: true } },
    });
    expect(labState.run.result).toBeUndefined();
    expect(labState.catalog.selectedTaskId).toBe(demoTasks[0].id);
    expect(loadSortieManifest(runId)).toBeUndefined();
  });

  it('keeps an interrupted single run tracking-stopped when the exact result is non-terminal', async () => {
    const runId = 'run-single-still-judging';
    const originalTrackingStoppedAt = '2026-07-17T00:02:00.000Z';
    setInterruptedSingleRun(runId);
    vi.spyOn(benchApi, 'result').mockResolvedValue({
      ...completedResult(runId),
      status: 'candidate_completed',
      score: undefined,
    });
    const controller = renderHook(() => useResultController());

    await act(async () => {
      await controller.result.current.loadResult(runId);
    });

    expect(labState.results.record?.status).toBe('candidate_completed');
    expect(labState.run).toMatchObject({
      runId,
      stage: 'candidate_completed',
      trackingStatus: 'tracking_stopped',
      trackingStoppedAt: originalTrackingStoppedAt,
    });
    expect(labState.run.result).toBeUndefined();
    expect(labState.run.error).toBeUndefined();
    expect(loadSortieManifest(runId)).toBeUndefined();
  });

  it('reconciles and archives an exact failed result for an interrupted single run', async () => {
    const runId = 'run-single-failed';
    const record: BenchRunResult = {
      ...completedResult(runId),
      status: 'failed',
      score: undefined,
    };
    setInterruptedSingleRun(runId);
    vi.spyOn(benchApi, 'result').mockResolvedValue(record);
    const controller = renderHook(() => useResultController());

    await act(async () => {
      await controller.result.current.loadResult(runId);
    });

    expect(labState.run).toMatchObject({
      runId,
      stage: 'failed',
      result: record,
      error: 'Bench 战报标记该次评测为失败。',
    });
    expect(labState.run.trackingStatus).toBeUndefined();
    expect(labState.run.trackingStoppedAt).toBeUndefined();
    expect(loadSortieManifest(runId)?.task.id).toBe(demoTasks[0].id);
  });

  it('does not project an older exact result onto a replacement single run', async () => {
    const olderRunId = 'run-single-older';
    const newerRunId = 'run-single-newer';
    const response = deferred<BenchRunResult>();
    setInterruptedSingleRun(olderRunId);
    vi.spyOn(benchApi, 'result').mockReturnValue(response.promise);
    const controller = renderHook(() => useResultController());

    let load!: Promise<void>;
    act(() => {
      load = controller.result.current.loadResult(olderRunId);
    });
    setInterruptedSingleRun(newerRunId);

    response.resolve(completedResult(olderRunId));
    await act(async () => {
      await load;
    });

    expect(labState.results.record?.run_id).toBe(olderRunId);
    expect(labState.run).toMatchObject({
      runId: newerRunId,
      stage: 'judging',
      trackingStatus: 'tracking_stopped',
    });
    expect(labState.run.result).toBeUndefined();
    expect(loadSortieManifest(olderRunId)).toBeUndefined();
  });

  it('uses an already returned campaign result by exact Run ID when Bench is temporarily disconnected', async () => {
    const roster = [DEFAULT_HANGAR_ROSTER[0]];
    const snapshot = createRunCampaignSnapshot(demoTasks[0], roster);
    const record = completedResult('run-cached-campaign');
    labState.connection = { mode: 'checking', message: 'Reconnecting' };
    labState.campaign = {
      generation: 1,
      status: 'completed',
      snapshot,
      members: [
        {
          rosterEntryId: roster[0].id,
          sortie: snapshot.roster[0],
          status: 'completed',
          runId: record.run_id,
          result: record,
        },
      ],
    };
    const resultRequest = vi.spyOn(benchApi, 'result');
    const controller = renderHook(() => useResultController());

    await act(async () => {
      await controller.result.current.loadResult(record.run_id);
    });

    expect(resultRequest).not.toHaveBeenCalled();
    expect(labState.results.record).toEqual(record);
    expect(labState.results.sortie).toMatchObject({
      task: { id: demoTasks[0].id },
      rosterEntry: { id: roster[0].id },
    });
  });

  it('re-verifies an interrupted campaign member by its exact Run ID and reconciles the terminal state', async () => {
    const roster = [DEFAULT_HANGAR_ROSTER[0]];
    const snapshot = createRunCampaignSnapshot(demoTasks[0], roster);
    const record = completedResult('run-interrupted');
    labState.campaign = {
      generation: 2,
      status: 'tracking_stopped',
      snapshot,
      members: [
        {
          rosterEntryId: roster[0].id,
          sortie: snapshot.roster[0],
          status: 'tracking_stopped',
          jobId: 'job-interrupted',
          runId: record.run_id,
        },
      ],
    };
    const resultRequest = vi.spyOn(benchApi, 'result').mockResolvedValue(record);
    const controller = renderHook(() => useResultController());

    await act(async () => {
      await controller.result.current.loadResult(record.run_id);
    });

    expect(resultRequest).toHaveBeenCalledWith('run-interrupted');
    expect(labState.campaign.status).toBe('completed');
    expect(labState.campaign.members[0]).toMatchObject({
      status: 'completed',
      stage: 'completed',
      runId: 'run-interrupted',
      result: record,
    });
    expect(labState.results.sortie?.task.id).toBe(demoTasks[0].id);
  });

  it('rejects an exact result whose Task does not match the frozen campaign map', async () => {
    const roster = [DEFAULT_HANGAR_ROSTER[0]];
    const snapshot = createRunCampaignSnapshot(demoTasks[0], roster);
    labState.campaign = {
      generation: 3,
      status: 'tracking_stopped',
      snapshot,
      members: [
        {
          rosterEntryId: roster[0].id,
          sortie: snapshot.roster[0],
          status: 'tracking_stopped',
          runId: 'run-wrong-task',
        },
      ],
    };
    vi.spyOn(benchApi, 'result').mockResolvedValue({
      ...completedResult('run-wrong-task'),
      task_id: demoTasks[1].id,
    });
    const controller = renderHook(() => useResultController());

    await act(async () => {
      await controller.result.current.loadResult('run-wrong-task');
    });

    expect(labState.results.record).toBeUndefined();
    expect(labState.results.sortie).toBeUndefined();
    expect(labState.results.error).toContain('Task 归属不匹配');
    expect(labState.campaign.members[0].status).toBe('tracking_stopped');
  });
});

function completedResult(runId: string): BenchRunResult {
  return {
    status: 'completed',
    governance_status: 'local_unofficial',
    run_id: runId,
    task_id: demoTasks[0].id,
    score: '87.50',
    primary_metric: 'score',
  };
}

function setInterruptedSingleRun(
  runId: string,
  options: { completedAt?: string; locked?: boolean } = { completedAt: '2026-07-17T00:01:00.000Z' }
): void {
  const sortie = createRunSortieSnapshot(demoTasks[0], DEFAULT_HANGAR_ROSTER[0], {
    task: options.locked ? './task.lock.json' : demoTasks[0].id,
    candidate: DEFAULT_HANGAR_ROSTER[0].candidate,
    model: options.locked ? undefined : DEFAULT_HANGAR_ROSTER[0].model,
    locked: options.locked ?? false,
  });
  labState.run = {
    mode: 'live',
    stage: 'judging',
    trackingStatus: 'tracking_stopped',
    trackingStoppedAt: '2026-07-17T00:02:00.000Z',
    jobId: `job-${runId}`,
    runId,
    startedAt: '2026-07-17T00:00:00.000Z',
    completedAt: options.completedAt,
    sortie,
  };
}

function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void } {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}
