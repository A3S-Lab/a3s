import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { demoTasks } from '../../data/demo-tasks';
import { benchApi } from '../../lib/api';
import { labState } from '../../state/lab-state';
import type { BenchHealth, BenchRunJob, BenchRunResult, BenchTask, StartBenchRunInput } from '../../types/bench';
import { createHangarDraft, DEFAULT_HANGAR_ROSTER } from '../hangar/hangar-configuration';
import { useHangarController } from '../hangar/use-hangar-controller';
import { saveSortieManifest } from './sortie-manifest-store';
import { benchJobIntegrityError, filteredTasks, useBenchController } from './use-bench-controller';

vi.mock('../../lib/api', () => ({
  benchApi: {
    health: vi.fn(),
    tasks: vi.fn(),
    task: vi.fn(),
    doctor: vi.fn(),
    startRun: vi.fn(),
    run: vi.fn(),
    result: vi.fn(),
  },
}));

vi.mock('./sortie-manifest-store', () => ({
  saveSortieManifest: vi.fn(),
}));

const liveJob: BenchRunJob = {
  jobId: 'job-integration-1',
  task: demoTasks[0].id,
  candidate: DEFAULT_HANGAR_ROSTER[0].candidate,
  model: DEFAULT_HANGAR_ROSTER[0].model,
  locked: false,
  status: 'running',
  stage: 'running',
  startedAt: '2026-07-17T00:00:00.000Z',
};

let acceptedJob = liveJob;

const fullRunResult: BenchRunResult = {
  status: 'completed',
  governance_status: 'local_unofficial',
  run_id: 'run-integration-1',
  task_reference: 'quick_file_edit',
  task_id: 'quick_file_edit',
  score: '0.9625',
  primary_metric: 'score',
  runtime_provider: 'test',
  model: 'openai/gpt-5.6',
  result_digest: 'sha256:complete-public-result',
};

beforeEach(() => {
  vi.clearAllMocks();
  labState.catalog = {
    tasks: demoTasks.map((task) => ({ ...task })),
    selectedTaskId: demoTasks[0].id,
    query: '',
    category: 'all',
    includeBlocked: false,
  };
  labState.connection = {
    mode: 'checking',
    message: 'Connecting',
  };
  labState.hangar = {
    draft: createHangarDraft('a3s'),
    roster: DEFAULT_HANGAR_ROSTER.map((entry) => ({ ...entry })),
    activeEntryId: DEFAULT_HANGAR_ROSTER[0].id,
  };
  labState.runConfig = {
    candidateLock: './candidate.lock.json',
    deploymentScope: 'single',
    locked: false,
    taskLock: './task.lock.json',
  };
  labState.run = { stage: 'idle' };
  labState.campaign = { generation: 0, status: 'idle', members: [] };
  labState.notice = undefined;

  vi.mocked(benchApi.health).mockResolvedValue({
    connected: true,
    component: 'a3s-bench',
    version: 'test',
    target: 'test',
    cliProtocol: 'test',
    workingDirectory: '/tmp/a3s-bench',
  });
  vi.mocked(benchApi.tasks).mockResolvedValue({ tasks: demoTasks.map((task) => ({ ...task })) });
  vi.mocked(benchApi.task).mockImplementation(async (taskId) => {
    const task = demoTasks.find((candidate) => candidate.id === taskId);
    if (!task) throw new Error(`Unknown test task: ${taskId}`);
    return { task: { ...task } };
  });
  vi.mocked(benchApi.doctor).mockResolvedValue({ runtime: { provider: 'test', ready: true, detail: 'ready' } });
  acceptedJob = liveJob;
  vi.mocked(benchApi.startRun).mockImplementation(async (input: StartBenchRunInput) => {
    acceptedJob = { ...liveJob, ...input };
    return acceptedJob;
  });
  vi.mocked(benchApi.run).mockImplementation(async (jobId) => ({
    ...acceptedJob,
    jobId,
    status: 'completed' as const,
    stage: 'completed' as const,
    completedAt: '2026-07-17T00:00:01.000Z',
    result: {
      status: 'completed' as const,
      run_id: fullRunResult.run_id,
      score: 'partial-job-score',
    },
  }));
  vi.mocked(benchApi.result).mockResolvedValue(fullRunResult);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe('filteredTasks', () => {
  it('matches task names, ids, and categories', () => {
    expect(filteredTasks(demoTasks, 'rust_multicrate', 'all', false).map((task) => task.id)).toEqual([
      'rust_multicrate_reconstruction',
    ]);
    expect(filteredTasks(demoTasks, 'scientific', 'all', true)).toHaveLength(2);
  });

  it('keeps blocked tasks behind the explicit filter', () => {
    expect(filteredTasks(demoTasks, 'wireless', 'all', false)).toEqual([]);
    expect(filteredTasks(demoTasks, 'wireless', 'all', true)).toHaveLength(1);
  });
});

describe('deployment scope', () => {
  it('stores the map deployment choice and coerces Campaign back to single when Lock mode is enabled', () => {
    const { result } = renderHook(() => useBenchController());

    act(() => result.current.setDeploymentScope('campaign'));
    expect(labState.runConfig.deploymentScope).toBe('campaign');

    act(() => result.current.setLocked(true));
    expect(labState.runConfig.locked).toBe(true);
    expect(labState.runConfig.deploymentScope).toBe('single');

    act(() => result.current.setDeploymentScope('campaign'));
    expect(labState.runConfig.deploymentScope).toBe('single');
    expect(labState.notice?.message).toContain('锁文件模式仅支持单机');
  });

  it('exposes a frontend-only Campaign tracking stop without claiming to cancel Bench work', () => {
    const { result } = renderHook(() => useBenchController());
    labState.campaign = {
      generation: 4,
      status: 'running',
      members: [],
    };

    let stopped = false;
    act(() => {
      stopped = result.current.stopCampaignTracking();
    });

    expect(stopped).toBe(true);
    expect(labState.campaign.status).toBe('tracking_stopped');
    expect(labState.notice?.message).toContain('已停止前端跟踪');
    expect(labState.notice?.message).toContain('Job 仍可能继续运行');
  });
});

describe('Bench run configuration integration', () => {
  it('sends the selected map task with the active hangar Candidate and model', async () => {
    const annRunResult = {
      ...fullRunResult,
      task_reference: 'ann_vector_search_qps',
      task_id: 'ann_vector_search_qps',
    };
    vi.mocked(benchApi.result).mockResolvedValue(annRunResult);
    const { result } = renderHook(() => ({ bench: useBenchController(), hangar: useHangarController() }));

    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    const codexEntry = labState.hangar.roster.find((entry) => entry.pilotId === 'codex');
    expect(codexEntry).toBeDefined();
    if (!codexEntry) return;

    await act(async () => {
      result.current.hangar.activateRosterEntry(codexEntry.id);
      result.current.bench.setCandidate('./agents/codex-gpt-5.6');
      result.current.bench.setModel('openai/gpt-5.6');
      await result.current.bench.selectTask('ann_vector_search_qps');
    });

    vi.useFakeTimers();
    const runPromise = result.current.bench.startRun();
    await act(async () => {
      await vi.runAllTimersAsync();
      await runPromise;
    });

    expect(labState.catalog.selectedTaskId).toBe('ann_vector_search_qps');
    expect(labState.hangar.activeEntryId).toBe(codexEntry.id);
    expect(benchApi.startRun).toHaveBeenCalledTimes(1);
    expect(benchApi.startRun).toHaveBeenCalledWith({
      task: 'ann_vector_search_qps',
      candidate: './agents/codex-gpt-5.6',
      model: 'openai/gpt-5.6',
      locked: false,
    });
    expect(benchApi.result).toHaveBeenCalledWith(fullRunResult.run_id);
    expect(labState.run.result).toEqual(annRunResult);
    expect(labState.run.result?.score).not.toBe('partial-job-score');
    expect(labState.run.runId).toBe(fullRunResult.run_id);

    const sortie = labState.run.sortie;
    expect(sortie).toMatchObject({
      task: { id: 'ann_vector_search_qps', name: 'ANN Vector Search QPS' },
      rosterEntry: {
        id: codexEntry.id,
        candidate: './agents/codex-gpt-5.6',
        model: 'openai/gpt-5.6',
        effort: codexEntry.effort,
        airframeId: codexEntry.airframeId,
        pilotId: codexEntry.pilotId,
        callsign: codexEntry.callsign,
      },
      input: {
        task: 'ann_vector_search_qps',
        candidate: './agents/codex-gpt-5.6',
        model: 'openai/gpt-5.6',
        locked: false,
      },
    });
    expect(Object.isFrozen(sortie)).toBe(true);
    expect(Object.isFrozen(sortie?.task)).toBe(true);
    expect(Object.isFrozen(sortie?.task.tags)).toBe(true);
    expect(Object.isFrozen(sortie?.rosterEntry)).toBe(true);
    expect(Object.isFrozen(sortie?.input)).toBe(true);

    const originalTaskName = sortie?.task.name;
    const originalCallsign = sortie?.rosterEntry.callsign;
    const selectedCatalogTask = labState.catalog.tasks.find((task) => task.id === 'ann_vector_search_qps');
    if (selectedCatalogTask) selectedCatalogTask.name = 'Mutated map name';
    codexEntry.callsign = 'MUTATED-99';
    expect(sortie?.task.name).toBe(originalTaskName);
    expect(sortie?.rosterEntry.callsign).toBe(originalCallsign);
  });

  it('sends lock files and binds the sortie to the Task ID resolved by Bench instead of the preview map', async () => {
    vi.mocked(benchApi.result).mockResolvedValue({
      ...fullRunResult,
      task_reference: 'ann_vector_search_qps',
      task_id: 'ann_vector_search_qps',
    });
    const { result } = renderHook(() => useBenchController());

    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    await act(async () => {
      await result.current.selectTask('portfolio_risk_calibration');
      result.current.setCandidate('claude-code');
      result.current.setModel('anthropic/claude-opus-4.6');
      result.current.setCandidateLock('  ./locks/claude.candidate.lock.json  ');
      result.current.setTaskLock('  ./locks/risk-map.task.lock.json  ');
      result.current.setLocked(true);
    });

    vi.useFakeTimers();
    const runPromise = result.current.startRun();
    await act(async () => {
      await vi.runAllTimersAsync();
      await runPromise;
    });

    expect(labState.catalog.selectedTaskId).toBe('ann_vector_search_qps');
    expect(benchApi.startRun).toHaveBeenCalledTimes(1);
    expect(benchApi.startRun).toHaveBeenCalledWith({
      task: './locks/risk-map.task.lock.json',
      candidate: './locks/claude.candidate.lock.json',
      model: undefined,
      locked: true,
    });
    expect(labState.run.sortie).toMatchObject({
      task: { id: 'ann_vector_search_qps', name: 'ANN Vector Search QPS' },
      rosterEntry: {
        candidate: 'claude-code',
        model: 'anthropic/claude-opus-4.6',
      },
      input: {
        task: './locks/risk-map.task.lock.json',
        candidate: './locks/claude.candidate.lock.json',
        model: undefined,
        locked: true,
      },
    });
    expect(labState.run.sortie?.task.id).not.toBe('portfolio_risk_calibration');
  });
});

describe('Bench run integrity', () => {
  it.each([
    ['task', { task: 'different-task' }],
    ['candidate', { candidate: './different-candidate' }],
    ['model', { model: 'different/model' }],
    ['locked', { locked: true }],
  ] satisfies Array<[string, Partial<BenchRunJob>]>)('rejects a Job whose %s ownership changed', (field, change) => {
    const input: StartBenchRunInput = {
      task: liveJob.task,
      candidate: liveJob.candidate,
      model: liveJob.model,
      locked: liveJob.locked,
    };

    expect(benchJobIntegrityError({ ...liveJob, ...change }, input)).toContain(field);
  });

  it('rejects empty and mismatched Job IDs', () => {
    const input: StartBenchRunInput = {
      task: liveJob.task,
      candidate: liveJob.candidate,
      model: liveJob.model,
      locked: liveJob.locked,
    };

    expect(benchJobIntegrityError({ ...liveJob, jobId: '   ' }, input)).toContain('Job ID');
    expect(benchJobIntegrityError(liveJob, input, 'different-job')).toContain('Job 归属不匹配');
  });

  it('rejects an unstructured failure Run ID', () => {
    const input: StartBenchRunInput = {
      task: liveJob.task,
      candidate: liveJob.candidate,
      model: liveJob.model,
      locked: liveJob.locked,
    };

    expect(
      benchJobIntegrityError({ ...liveJob, status: 'failed', stage: 'failed', runId: 'invented-run' }, input)
    ).toContain('Run ID');
  });

  it('fails an accepted run with an empty Job ID before attempting to poll it', async () => {
    vi.mocked(benchApi.startRun).mockResolvedValue({ ...liveJob, jobId: '   ' });
    const { result } = renderHook(() => useBenchController());
    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    await act(async () => result.current.startRun());

    expect(benchApi.run).not.toHaveBeenCalled();
    expect(benchApi.result).not.toHaveBeenCalled();
    expect(labState.run.jobId).toBeUndefined();
    expect(labState.run.stage).toBe('failed');
    expect(labState.notice).toMatchObject({ tone: 'error', message: expect.stringContaining('Job ID') });
  });

  it.each([
    ['a different Job ID', { jobId: 'job-owned-by-another-run' }],
    ['different frozen ownership', { candidate: './candidate-owned-by-another-run' }],
  ] satisfies Array<[string, Partial<BenchRunJob>]>)('rejects a poll response with %s', async (_case, change) => {
    vi.mocked(benchApi.run).mockResolvedValue({ ...liveJob, ...change });
    const { result } = renderHook(() => useBenchController());
    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    vi.useFakeTimers();
    const runPromise = result.current.startRun();
    await act(async () => {
      await vi.runAllTimersAsync();
      await runPromise;
    });

    expect(benchApi.run).toHaveBeenCalledWith(liveJob.jobId);
    expect(benchApi.result).not.toHaveBeenCalled();
    expect(labState.run.stage).toBe('failed');
    expect(labState.notice?.tone).toBe('error');
    expect(labState.notice?.message).toContain('归属');
  });

  it('preserves a structured failed Run ID in the terminal run state', async () => {
    const runId = 'local-1721188800000-42-0';
    vi.mocked(benchApi.run).mockResolvedValue({
      ...liveJob,
      status: 'failed',
      stage: 'failed',
      runId,
      completedAt: '2026-07-17T00:00:01.000Z',
      error: `run ${runId} failed: Candidate Adapter exited`,
    });
    const { result } = renderHook(() => useBenchController());
    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    vi.useFakeTimers();
    const runPromise = result.current.startRun();
    await act(async () => {
      await vi.runAllTimersAsync();
      await runPromise;
    });

    expect(labState.run).toMatchObject({
      stage: 'failed',
      jobId: liveJob.jobId,
      runId,
      error: expect.stringContaining(`run ${runId} failed`),
    });
    expect(benchApi.result).not.toHaveBeenCalled();
  });

  it('keeps an exact nonterminal result unarchived and unannounced until it becomes completed', async () => {
    const judgingResult: BenchRunResult = {
      status: 'judging',
      run_id: fullRunResult.run_id,
      task_reference: fullRunResult.task_reference,
    };
    vi.mocked(benchApi.result).mockResolvedValueOnce(judgingResult).mockResolvedValueOnce(fullRunResult);
    const { result } = renderHook(() => useBenchController());
    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    vi.useFakeTimers();
    const runPromise = result.current.startRun();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(900);
    });

    expect(benchApi.result).toHaveBeenCalledTimes(1);
    expect(labState.run.stage).toBe('judging');
    expect(labState.run.result).toBeUndefined();
    expect(saveSortieManifest).not.toHaveBeenCalled();
    expect(labState.notice?.tone).not.toBe('success');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(900);
      await runPromise;
    });

    expect(benchApi.result).toHaveBeenCalledTimes(2);
    expect(labState.run.stage).toBe('completed');
    expect(labState.run.result).toEqual(fullRunResult);
    expect(saveSortieManifest).toHaveBeenCalledOnce();
    expect(labState.notice?.tone).toBe('success');
  });

  it('rejects an unlocked result attributed to a different Task', async () => {
    vi.mocked(benchApi.result).mockResolvedValue({
      ...fullRunResult,
      task_id: demoTasks[1].id,
    });
    const { result } = renderHook(() => useBenchController());
    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    vi.useFakeTimers();
    const runPromise = result.current.startRun();
    await act(async () => {
      await vi.runAllTimersAsync();
      await runPromise;
    });

    expect(labState.run.stage).toBe('failed');
    expect(labState.run.result).toBeUndefined();
    expect(saveSortieManifest).not.toHaveBeenCalled();
    expect(labState.notice?.message).toContain('Task 归属不匹配');
  });

  it('fails closed when a Task Lock result cannot be resolved against the Bench catalog', async () => {
    const unknownTaskId = 'task-only-inside-lock';
    vi.mocked(benchApi.result).mockResolvedValue({
      ...fullRunResult,
      task_id: unknownTaskId,
    });
    vi.mocked(benchApi.task).mockRejectedValueOnce(new Error(`Unknown test task: ${unknownTaskId}`));
    const { result } = renderHook(() => useBenchController());
    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    act(() => result.current.setLocked(true));
    vi.useFakeTimers();
    const runPromise = result.current.startRun();
    await act(async () => {
      await vi.runAllTimersAsync();
      await runPromise;
    });

    expect(benchApi.task).toHaveBeenCalledWith(unknownTaskId, true);
    expect(labState.run.stage).toBe('failed');
    expect(labState.run.result).toBeUndefined();
    expect(saveSortieManifest).not.toHaveBeenCalled();
    expect(labState.notice?.message).toContain(`Unknown test task: ${unknownTaskId}`);
  });

  it('fails truthfully when an exact result never reaches a terminal status', async () => {
    vi.mocked(benchApi.result).mockResolvedValue({
      status: 'judging',
      run_id: fullRunResult.run_id,
      task_reference: fullRunResult.task_reference,
    });
    const { result } = renderHook(() => useBenchController());
    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    vi.useFakeTimers();
    const runPromise = result.current.startRun();
    await act(async () => {
      await vi.runAllTimersAsync();
      await runPromise;
    });

    expect(benchApi.result).toHaveBeenCalledTimes(5);
    expect(labState.run.stage).toBe('failed');
    expect(labState.run.result).toBeUndefined();
    expect(saveSortieManifest).not.toHaveBeenCalled();
    expect(labState.notice).toMatchObject({ tone: 'error', message: expect.stringContaining('非终态 judging') });
  });

  it('treats an exact failed result as failure rather than success', async () => {
    const failedResult: BenchRunResult = {
      status: 'failed',
      run_id: fullRunResult.run_id,
      task_reference: fullRunResult.task_reference,
    };
    vi.mocked(benchApi.result).mockResolvedValue(failedResult);
    const { result } = renderHook(() => useBenchController());
    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    vi.useFakeTimers();
    const runPromise = result.current.startRun();
    await act(async () => {
      await vi.runAllTimersAsync();
      await runPromise;
    });

    expect(labState.run.stage).toBe('failed');
    expect(labState.run.result).toEqual(failedResult);
    expect(saveSortieManifest).toHaveBeenCalledOnce();
    expect(labState.notice).toMatchObject({ tone: 'error', message: expect.stringContaining('标记该次评测为失败') });
  });

  it('rejects missing and unsupported mutable Candidate references before calling Bench', async () => {
    const { result } = renderHook(() => useBenchController());
    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    act(() => result.current.setCandidate(''));
    await act(async () => result.current.startRun());

    expect(benchApi.startRun).not.toHaveBeenCalled();
    expect(labState.run).toEqual({ stage: 'idle' });
    expect(labState.notice?.message).toContain('Candidate Adapter');

    act(() => result.current.setCandidate('codex'));
    await act(async () => result.current.startRun());

    expect(benchApi.startRun).not.toHaveBeenCalled();
    expect(labState.run).toEqual({ stage: 'idle' });
    expect(labState.notice?.message).toContain('a3s-code');
  });

  it('rejects bundled A3S Code without a model route before calling Bench', async () => {
    const { result } = renderHook(() => useBenchController());
    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    act(() => result.current.setModel(''));
    await act(async () => result.current.startRun());

    expect(benchApi.startRun).not.toHaveBeenCalled();
    expect(labState.notice?.message).toContain('provider/model');
  });

  it('rejects a map whose Judge dependency is absent from Doctor', async () => {
    const { result } = renderHook(() => useBenchController());
    await waitFor(() => expect(labState.connection.mode).toBe('live'));
    const task = labState.catalog.tasks[0];
    task.availability_reason = 'requires_configured_judge_model';

    await act(async () => result.current.startRun());

    expect(benchApi.startRun).not.toHaveBeenCalled();
    expect(labState.notice?.message).toContain('Judge 模型');
  });

  it('does not deploy or synthesize a result while connection checking or previewing', async () => {
    const healthRequest = deferred<BenchHealth>();
    vi.mocked(benchApi.health).mockReturnValueOnce(healthRequest.promise);
    const { result } = renderHook(() => useBenchController());

    expect(labState.connection.mode).toBe('checking');
    await act(async () => result.current.startRun());
    expect(benchApi.startRun).not.toHaveBeenCalled();
    expect(labState.run).toEqual({ stage: 'idle' });
    expect(labState.run.result).toBeUndefined();

    await act(async () => healthRequest.reject(new Error('bridge offline')));
    await waitFor(() => expect(labState.connection.mode).toBe('preview'));
    await act(async () => result.current.startRun());

    expect(benchApi.startRun).not.toHaveBeenCalled();
    expect(labState.run).toEqual({ stage: 'idle' });
    expect(labState.run.result).toBeUndefined();
    expect(labState.notice?.message).toContain('预览数据');
  });

  it('keeps the connection non-deployable when Doctor reports an unready Runtime', async () => {
    vi.mocked(benchApi.doctor).mockResolvedValue({
      runtime: { provider: 'test', ready: false, detail: 'Docker daemon is unavailable' },
    });
    const { result } = renderHook(() => useBenchController());

    await waitFor(() => expect(labState.connection.mode).toBe('preview'));
    expect(labState.connection.health?.connected).toBe(true);
    expect(labState.connection.doctor?.runtime.ready).toBe(false);
    expect(labState.connection.message).toContain('Runtime 未就绪');

    await act(async () => result.current.startRun());
    expect(benchApi.startRun).not.toHaveBeenCalled();
    expect(labState.run).toEqual({ stage: 'idle' });
  });

  it('prevents concurrent start requests and preserves one sortie-to-result association', async () => {
    const startRequest = deferred<BenchRunJob>();
    vi.mocked(benchApi.startRun).mockReturnValueOnce(startRequest.promise);
    const { result } = renderHook(() => useBenchController());
    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    vi.useFakeTimers();
    const firstRun = result.current.startRun();
    const firstSortie = labState.run.sortie;
    const duplicateRun = result.current.startRun();
    await duplicateRun;

    expect(benchApi.startRun).toHaveBeenCalledTimes(1);
    expect(labState.notice?.message).toContain('已有评测正在运行');
    expect(firstSortie).toBeDefined();

    startRequest.resolve(liveJob);
    await act(async () => {
      await vi.runAllTimersAsync();
      await firstRun;
    });

    expect(benchApi.startRun).toHaveBeenCalledTimes(1);
    expect(labState.run.sortie).toBe(firstSortie);
    expect(labState.notice?.tone).toBe('success');
    expect(labState.run.runId).toBe(fullRunResult.run_id);
    expect(labState.run.result?.run_id).toBe(fullRunResult.run_id);
  });

  it('rejects map and controller configuration changes during a run while the snapshot stays stable', async () => {
    const startRequest = deferred<BenchRunJob>();
    vi.mocked(benchApi.startRun).mockReturnValueOnce(startRequest.promise);
    const { result } = renderHook(() => useBenchController());
    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    const activeEntry = labState.hangar.roster.find((entry) => entry.id === labState.hangar.activeEntryId);
    expect(activeEntry).toBeDefined();
    if (!activeEntry) return;
    const originalTaskId = labState.catalog.selectedTaskId;
    const originalCandidate = activeEntry.candidate;
    const originalModel = activeEntry.model;
    const originalEffort = activeEntry.effort;

    vi.useFakeTimers();
    const runPromise = result.current.startRun();
    const sortie = labState.run.sortie;
    expect(sortie).toBeDefined();

    await act(async () => {
      await result.current.selectTask('portfolio_risk_calibration');
      result.current.setCandidate('./agents/replacement');
      result.current.setModel('replacement/model');
      result.current.setEffort('minimal');
      result.current.setCandidateLock('./replacement.candidate.lock.json');
      result.current.setTaskLock('./replacement.task.lock.json');
      result.current.setLocked(true);
    });

    expect(labState.catalog.selectedTaskId).toBe(originalTaskId);
    expect(activeEntry.candidate).toBe(originalCandidate);
    expect(activeEntry.model).toBe(originalModel);
    expect(activeEntry.effort).toBe(originalEffort);
    expect(labState.runConfig).toEqual({
      candidateLock: './candidate.lock.json',
      deploymentScope: 'single',
      locked: false,
      taskLock: './task.lock.json',
    });
    expect(benchApi.task).not.toHaveBeenCalled();

    labState.catalog.selectedTaskId = 'portfolio_risk_calibration';
    activeEntry.callsign = 'DIRECT-MUTATION';
    expect(sortie?.task.id).toBe(originalTaskId);
    expect(sortie?.rosterEntry.callsign).not.toBe('DIRECT-MUTATION');

    startRequest.resolve(liveJob);
    await act(async () => {
      await vi.runAllTimersAsync();
      await runPromise;
    });
    expect(labState.run.sortie).toBe(sortie);
  });

  it('ignores stale task detail responses after a newer map selection', async () => {
    const firstDetail = deferred<{ task: BenchTask }>();
    const secondDetail = deferred<{ task: BenchTask }>();
    vi.mocked(benchApi.task).mockReturnValueOnce(firstDetail.promise).mockReturnValueOnce(secondDetail.promise);
    const { result } = renderHook(() => useBenchController());
    await waitFor(() => expect(labState.connection.mode).toBe('live'));

    const firstTask = demoTasks.find((task) => task.id === 'ann_vector_search_qps');
    const secondTask = demoTasks.find((task) => task.id === 'portfolio_risk_calibration');
    expect(firstTask).toBeDefined();
    expect(secondTask).toBeDefined();
    if (!firstTask || !secondTask) return;

    const firstSelection = result.current.selectTask(firstTask.id);
    const secondSelection = result.current.selectTask(secondTask.id);
    secondDetail.resolve({ task: { ...secondTask, name: 'Newest map detail' } });
    await secondSelection;
    firstDetail.resolve({ task: { ...firstTask, name: 'Stale map detail' } });
    await firstSelection;

    expect(labState.catalog.selectedTaskId).toBe(secondTask.id);
    expect(labState.catalog.tasks.find((task) => task.id === secondTask.id)?.name).toBe('Newest map detail');
    expect(labState.catalog.tasks.find((task) => task.id === firstTask.id)?.name).toBe(firstTask.name);
    expect(labState.notice).toBeUndefined();
  });
});

interface Deferred<Value> {
  promise: Promise<Value>;
  resolve: (value: Value) => void;
  reject: (reason?: unknown) => void;
}

function deferred<Value>(): Deferred<Value> {
  let resolve!: (value: Value) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<Value>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}
