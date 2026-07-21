import { beforeEach, describe, expect, it, vi } from 'vitest';
import { demoTasks } from '../../data/demo-tasks';
import { isCampaignActive, isEvaluationActive, labState } from '../../state/lab-state';
import type { BenchRunJob, BenchRunResult, StartBenchRunInput } from '../../types/bench';
import { createHangarDraft, type HangarRosterEntry } from '../hangar/hangar-configuration';
import { CAMPAIGN_CONCURRENCY, type CampaignBenchApi, createCampaignController } from './campaign-controller';

beforeEach(() => {
  const roster = createRoster(3);
  labState.connection = {
    mode: 'live',
    message: 'Ready',
    doctor: {
      runtime: { provider: 'test', ready: true, detail: 'ready' },
      judge_model: 'test/judge',
    },
  };
  labState.catalog = {
    tasks: demoTasks.map((task) => ({ ...task, tags: task.tags ? [...task.tags] : undefined })),
    selectedTaskId: demoTasks[0].id,
    query: '',
    category: 'all',
    includeBlocked: false,
  };
  labState.hangar = {
    draft: createHangarDraft('generic'),
    roster,
    activeEntryId: roster[0].id,
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
});

describe('campaign controller', () => {
  it('runs a frozen formation snapshot through a real two-worker queue and preserves result ownership', async () => {
    labState.hangar.roster = createRoster(5);
    labState.hangar.activeEntryId = labState.hangar.roster[0].id;
    let activeJobs = 0;
    let maximumActiveJobs = 0;
    const runRequests = new Map<string, Deferred<BenchRunJob>>();
    const startRun = vi.fn(async (input: StartBenchRunInput): Promise<BenchRunJob> => {
      const index = candidateIndex(input.candidate);
      activeJobs += 1;
      maximumActiveJobs = Math.max(maximumActiveJobs, activeJobs);
      return runningJob(index, input);
    });
    const run = vi.fn((jobId: string): Promise<BenchRunJob> => {
      const request = deferred<BenchRunJob>();
      runRequests.set(jobId, request);
      return request.promise;
    });
    const result = vi.fn(async (runId: string): Promise<BenchRunResult> => {
      activeJobs -= 1;
      return completedResult(runId);
    });
    const saveManifest = vi.fn();
    const campaign = createCampaignController({
      api: { startRun, run, result },
      saveManifest,
      wait: async () => undefined,
      now: fixedClock(),
    });

    const execution = campaign.startCampaign();
    await vi.waitFor(() => expect(runRequests.size).toBe(CAMPAIGN_CONCURRENCY));

    expect(startRun).toHaveBeenCalledTimes(2);
    expect(labState.campaign.members.map((member) => member.status)).toEqual([
      'running',
      'running',
      'queued',
      'queued',
      'queued',
    ]);
    expect(labState.campaign.members[2].stage).toBeUndefined();

    const snapshot = labState.campaign.snapshot;
    expect(snapshot).toBeDefined();
    if (!snapshot) return;
    expect(Object.isFrozen(snapshot)).toBe(true);
    expect(Object.isFrozen(snapshot.task)).toBe(true);
    expect(Object.isFrozen(snapshot.task.tags)).toBe(true);
    expect(Object.isFrozen(snapshot.roster)).toBe(true);
    expect(snapshot.roster).toHaveLength(5);
    for (const sortie of snapshot.roster) {
      expect(Object.isFrozen(sortie)).toBe(true);
      expect(Object.isFrozen(sortie.rosterEntry)).toBe(true);
      expect(Object.isFrozen(sortie.input)).toBe(true);
      expect(sortie.input.task).toBe(snapshot.task.id);
      expect(sortie.input.locked).toBe(false);
    }

    const frozenTaskName = snapshot.task.name;
    const frozenLastCandidate = snapshot.roster[4].input.candidate;
    labState.catalog.tasks[0].name = 'Mutated after launch';
    labState.hangar.roster[4].candidate = './agents/mutated-after-launch';
    expect(snapshot.task.name).toBe(frozenTaskName);
    expect(snapshot.roster[4].input.candidate).toBe(frozenLastCandidate);

    completePendingJob(runRequests, 0);
    await vi.waitFor(() => expect(startRun).toHaveBeenCalledTimes(3));
    completePendingJob(runRequests, 1);
    await vi.waitFor(() => expect(startRun).toHaveBeenCalledTimes(4));
    completePendingJob(runRequests, 2);
    await vi.waitFor(() => expect(startRun).toHaveBeenCalledTimes(5));
    completePendingJob(runRequests, 3);
    completePendingJob(runRequests, 4);
    await expect(execution).resolves.toBe(true);

    expect(maximumActiveJobs).toBe(CAMPAIGN_CONCURRENCY);
    expect(activeJobs).toBe(0);
    expect(labState.campaign.status).toBe('completed');
    expect(labState.runConfig.deploymentScope).toBe('campaign');
    expect(startRun.mock.calls.map(([input]) => input.candidate)).toEqual(
      Array.from({ length: 5 }, (_, index) => `./agents/candidate-${index}`)
    );
    expect(result.mock.calls.map(([runId]) => runId).sort()).toEqual(
      Array.from({ length: 5 }, (_, index) => `run-${index}`)
    );
    expect(saveManifest.mock.calls.map(([runId]) => runId).sort()).toEqual(
      Array.from({ length: 5 }, (_, index) => `run-${index}`)
    );
    expect(labState.campaign.members).toHaveLength(5);
    for (const [index, member] of labState.campaign.members.entries()) {
      expect(member).toMatchObject({
        rosterEntryId: `roster-${index}`,
        status: 'completed',
        jobId: `job-${index}`,
        runId: `run-${index}`,
        result: { run_id: `run-${index}`, score: `0.${index + 5}` },
      });
      expect(member.sortie.input.candidate).toBe(`./agents/candidate-${index}`);
    }
  });

  it('isolates member failures and completes the remaining formation', async () => {
    const startRun = vi.fn(async (input: StartBenchRunInput): Promise<BenchRunJob> => {
      const index = candidateIndex(input.candidate);
      if (index === 0) throw new Error('candidate adapter failed to start');
      return completedJob(index);
    });
    const run = vi.fn<(jobId: string) => Promise<BenchRunJob>>();
    const result = vi.fn(async (runId: string) => completedResult(runId));
    const saveManifest = vi.fn();
    const campaign = createCampaignController({
      api: { startRun, run, result },
      saveManifest,
      wait: async () => undefined,
      now: fixedClock(),
    });

    await expect(campaign.startCampaign()).resolves.toBe(true);

    expect(startRun).toHaveBeenCalledTimes(3);
    expect(run).not.toHaveBeenCalled();
    expect(labState.campaign.status).toBe('completed_with_failures');
    expect(labState.campaign.members.map((member) => member.status)).toEqual(['failed', 'completed', 'completed']);
    expect(labState.campaign.members[0].error).toBe('candidate adapter failed to start');
    expect(labState.campaign.members[0].jobId).toBeUndefined();
    expect(labState.campaign.members[1]).toMatchObject({ jobId: 'job-1', runId: 'run-1' });
    expect(labState.campaign.members[2]).toMatchObject({ jobId: 'job-2', runId: 'run-2' });
    expect(result.mock.calls.map(([runId]) => runId).sort()).toEqual(['run-1', 'run-2']);
    expect(saveManifest.mock.calls.map(([runId]) => runId).sort()).toEqual(['run-1', 'run-2']);
  });

  it('rejects a Bench Job whose frozen input ownership does not match the roster sortie', async () => {
    labState.hangar.roster = createRoster(1);
    labState.hangar.activeEntryId = labState.hangar.roster[0].id;
    const wrongJob = { ...completedJob(0), task: 'another_task' };
    const api = {
      startRun: vi.fn(async () => wrongJob),
      run: vi.fn<(jobId: string) => Promise<BenchRunJob>>(),
      result: vi.fn<(runId: string) => Promise<BenchRunResult>>(),
    };
    const saveManifest = vi.fn();
    const campaign = createCampaignController({ api, saveManifest, now: fixedClock() });

    await expect(campaign.startCampaign()).resolves.toBe(true);

    expect(labState.campaign.status).toBe('failed');
    expect(labState.campaign.members[0]).toMatchObject({
      status: 'failed',
      error: expect.stringContaining('Task 归属不匹配'),
    });
    expect(api.result).not.toHaveBeenCalled();
    expect(saveManifest).not.toHaveBeenCalled();
  });

  it('requires the exact frozen Task and a terminal completed result before archiving a member', async () => {
    labState.hangar.roster = createRoster(1);
    labState.hangar.activeEntryId = labState.hangar.roster[0].id;
    const wrongTaskResult = { ...completedResult('run-0'), task_id: 'another_task' };
    const firstApi = {
      startRun: vi.fn(async () => completedJob(0)),
      run: vi.fn<(jobId: string) => Promise<BenchRunJob>>(),
      result: vi.fn(async () => wrongTaskResult),
    };
    const firstSaveManifest = vi.fn();
    const firstCampaign = createCampaignController({
      api: firstApi,
      saveManifest: firstSaveManifest,
      now: fixedClock(),
    });

    await expect(firstCampaign.startCampaign()).resolves.toBe(true);

    expect(labState.campaign.status).toBe('failed');
    expect(labState.campaign.members[0].error).toContain('Task 归属不匹配');
    expect(firstSaveManifest).not.toHaveBeenCalled();

    labState.campaign = { generation: 0, status: 'idle', members: [] };
    const judgingResult: BenchRunResult = { ...completedResult('run-0'), status: 'judging' };
    const secondApi = {
      startRun: vi.fn(async () => completedJob(0)),
      run: vi.fn<(jobId: string) => Promise<BenchRunJob>>(),
      result: vi.fn().mockResolvedValueOnce(judgingResult).mockResolvedValueOnce(completedResult('run-0')),
    };
    const secondSaveManifest = vi.fn();
    const secondCampaign = createCampaignController({
      api: secondApi,
      saveManifest: secondSaveManifest,
      wait: async () => undefined,
      now: fixedClock(),
    });

    await expect(secondCampaign.startCampaign()).resolves.toBe(true);

    expect(secondApi.result).toHaveBeenCalledTimes(2);
    expect(secondApi.result).toHaveBeenNthCalledWith(1, 'run-0', expect.any(AbortSignal));
    expect(secondApi.result).toHaveBeenNthCalledWith(2, 'run-0', expect.any(AbortSignal));
    expect(labState.campaign.members[0]).toMatchObject({ status: 'completed', stage: 'completed', runId: 'run-0' });
    expect(secondSaveManifest).toHaveBeenCalledOnce();
  });

  it('rejects the whole formation before any API request when one member is not deployable', async () => {
    const api = emptyApi();
    const campaign = createCampaignController({ api });
    labState.hangar.roster[1].candidate = 'codex';

    await expect(campaign.startCampaign()).resolves.toBe(false);

    expect(api.startRun).not.toHaveBeenCalled();
    expect(labState.campaign).toEqual({ generation: 0, status: 'idle', members: [] });
    expect(labState.notice?.message).toContain('CALL-1');
    expect(labState.notice?.message).toContain('a3s-code');

    labState.hangar.roster[1].candidate = 'a3s-code';
    labState.hangar.roster[1].model = '';
    await expect(campaign.startCampaign()).resolves.toBe(false);

    expect(api.startRun).not.toHaveBeenCalled();
    expect(labState.notice?.message).toContain('provider/model');
  });

  it('requires live Doctor readiness, an ordinary run, and a deployable task', async () => {
    const api = emptyApi();
    const campaign = createCampaignController({ api });

    labState.connection.mode = 'preview';
    await expect(campaign.startCampaign()).resolves.toBe(false);
    expect(labState.notice?.message).toContain('预览数据');

    labState.connection.mode = 'live';
    if (labState.connection.doctor) labState.connection.doctor.runtime.ready = false;
    await expect(campaign.startCampaign()).resolves.toBe(false);
    expect(labState.notice?.message).toContain('Doctor 未就绪');

    if (labState.connection.doctor) {
      labState.connection.doctor.runtime.ready = true;
      labState.connection.doctor.judge_model = null;
    }
    labState.catalog.tasks[0].availability_reason = 'requires_configured_judge_model';
    await expect(campaign.startCampaign()).resolves.toBe(false);
    expect(labState.notice?.message).toContain('Judge 模型');

    labState.catalog.tasks[0].availability_reason = 'bundled_offline_task';
    labState.runConfig.locked = true;
    await expect(campaign.startCampaign()).resolves.toBe(false);
    expect(labState.notice?.message).toContain('仅支持普通 Candidate 模式');
    expect(api.startRun).not.toHaveBeenCalled();
  });

  it('blocks duplicate launches and uses generation invalidation to stop only frontend tracking', async () => {
    labState.hangar.roster = createRoster(1);
    labState.hangar.activeEntryId = labState.hangar.roster[0].id;
    const startRequest = deferred<BenchRunJob>();
    const startRun = vi.fn((_input: StartBenchRunInput, _signal?: AbortSignal) => startRequest.promise);
    const run = vi.fn<(jobId: string) => Promise<BenchRunJob>>();
    const result = vi.fn<(runId: string) => Promise<BenchRunResult>>();
    const campaign = createCampaignController({
      api: { startRun, run, result },
      wait: async () => undefined,
      now: fixedClock(),
    });

    const firstLaunch = campaign.startCampaign();
    await vi.waitFor(() => expect(startRun).toHaveBeenCalledTimes(1));
    await expect(campaign.startCampaign()).resolves.toBe(false);
    expect(startRun).toHaveBeenCalledTimes(1);

    const firstGeneration = labState.campaign.generation;
    expect(campaign.stopCampaignTracking()).toBe(true);
    expect(labState.campaign.generation).toBeGreaterThan(firstGeneration);
    expect(labState.campaign.status).toBe('tracking_stopped');
    expect(labState.campaign.members[0].status).toBe('tracking_stopped');
    expect(labState.notice?.message).toContain('Job 仍可能继续运行');
    const signal = startRun.mock.calls[0][1];
    expect(signal?.aborted).toBe(true);

    startRequest.resolve(completedJob(0));
    await expect(firstLaunch).resolves.toBe(true);
    expect(result).not.toHaveBeenCalled();
    expect(labState.campaign.status).toBe('tracking_stopped');
  });

  it('exposes campaign-aware guards without treating queued state as a Bench stage', () => {
    expect(isCampaignActive('running')).toBe(true);
    expect(isCampaignActive('completed')).toBe(false);
    expect(isEvaluationActive('idle', 'running')).toBe(true);
    expect(isEvaluationActive('candidate_running', 'idle')).toBe(true);
    expect(isEvaluationActive('completed', 'completed_with_failures')).toBe(false);
  });
});

function createRoster(size: number): HangarRosterEntry[] {
  return Array.from({ length: size }, (_, index) => ({
    id: `roster-${index}`,
    airframeId: index % 2 === 0 ? 'j-35' : 'f-35',
    pilotId: index % 2 === 0 ? 'a3s' : 'codex',
    candidate: `./agents/candidate-${index}`,
    model: `provider/model-${index}`,
    effort: index % 2 === 0 ? 'high' : 'medium',
    callsign: `CALL-${index}`,
  }));
}

function candidateIndex(candidate: string): number {
  return Number(candidate.match(/(\d+)$/u)?.[1]);
}

function runningJob(index: number, input: StartBenchRunInput): BenchRunJob {
  return {
    jobId: `job-${index}`,
    task: input.task,
    candidate: input.candidate,
    model: input.model,
    locked: false,
    status: 'running',
    stage: 'candidate_running',
    startedAt: `2026-07-17T00:00:0${index}.000Z`,
  };
}

function completedJob(index: number): BenchRunJob {
  return {
    jobId: `job-${index}`,
    task: demoTasks[0].id,
    candidate: `./agents/candidate-${index}`,
    model: `provider/model-${index}`,
    locked: false,
    status: 'completed',
    stage: 'completed',
    startedAt: `2026-07-17T00:00:0${index}.000Z`,
    completedAt: `2026-07-17T00:01:0${index}.000Z`,
    result: completedResult(`run-${index}`),
  };
}

function completedResult(runId: string): BenchRunResult {
  const index = Number(runId.match(/(\d+)$/u)?.[1]);
  return {
    status: 'completed',
    run_id: runId,
    task_id: demoTasks[0].id,
    score: `0.${index + 5}`,
  };
}

function completePendingJob(requests: Map<string, Deferred<BenchRunJob>>, index: number): void {
  const request = requests.get(`job-${index}`);
  if (!request) throw new Error(`Missing pending job-${index}`);
  request.resolve(completedJob(index));
}

function emptyApi() {
  const startRun = vi.fn(async (_input: StartBenchRunInput, _signal?: AbortSignal): Promise<BenchRunJob> => {
    throw new Error('Unexpected startRun call');
  });
  const run = vi.fn(async (_jobId: string, _signal?: AbortSignal): Promise<BenchRunJob> => {
    throw new Error('Unexpected run call');
  });
  const result = vi.fn(async (_runId: string, _signal?: AbortSignal): Promise<BenchRunResult> => {
    throw new Error('Unexpected result call');
  });
  return { startRun, run, result } satisfies CampaignBenchApi;
}

function fixedClock(): () => string {
  let tick = 0;
  return () => new Date(Date.UTC(2026, 6, 17, 0, 0, tick++)).toISOString();
}

interface Deferred<Value> {
  promise: Promise<Value>;
  resolve: (value: Value) => void;
}

function deferred<Value>(): Deferred<Value> {
  let resolve!: (value: Value) => void;
  const promise = new Promise<Value>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}
