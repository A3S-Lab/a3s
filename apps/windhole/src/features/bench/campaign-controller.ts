import { benchApi } from '../../lib/api';
import {
  createRunCampaignSnapshot,
  createRunSortieSnapshot,
  isBenchRunActive,
  isCampaignActive,
  type LabCampaignMemberRun,
  labState,
  type RunCampaignSnapshot,
  type RunSortieSnapshot,
  selectedTask,
} from '../../state/lab-state';
import type { BenchRunJob, BenchRunResult, StartBenchRunInput } from '../../types/bench';
import { campaignDeploymentStatus } from './campaign-deployment-status';
import { saveSortieManifest } from './sortie-manifest-store';

export const CAMPAIGN_CONCURRENCY = 2;

export interface CampaignBenchApi {
  startRun: (input: StartBenchRunInput, signal?: AbortSignal) => Promise<BenchRunJob>;
  run: (jobId: string, signal?: AbortSignal) => Promise<BenchRunJob>;
  result: (runId: string, signal?: AbortSignal) => Promise<BenchRunResult>;
}

export interface CampaignControllerDependencies {
  api: CampaignBenchApi;
  saveManifest: (runId: string, sortie: Readonly<RunSortieSnapshot>) => void;
  wait: (milliseconds: number, signal: AbortSignal) => Promise<void>;
  now: () => string;
  pollIntervalMs: number;
}

export interface CampaignController {
  startCampaign: () => Promise<boolean>;
  stopCampaignTracking: () => boolean;
}

const defaultDependencies: CampaignControllerDependencies = {
  api: benchApi,
  saveManifest: saveSortieManifest,
  wait: waitForPoll,
  now: () => new Date().toISOString(),
  pollIntervalMs: 900,
};

class CampaignControllerImpl implements CampaignController {
  private generation = 0;
  private execution?: Promise<void>;
  private abortController?: AbortController;

  constructor(private readonly dependencies: CampaignControllerDependencies) {}

  async startCampaign(): Promise<boolean> {
    if (this.execution || isCampaignActive() || isBenchRunActive()) {
      labState.notice = { tone: 'error', message: '已有评测正在运行，请等待当前出击结束。' };
      return false;
    }

    const task = selectedTask();
    const preparation = campaignDeploymentStatus({
      connectionMode: labState.connection.mode,
      doctor: labState.connection.doctor,
      locked: labState.runConfig.locked,
      roster: labState.hangar.roster,
      task,
    });
    if (!preparation.deployable) {
      labState.notice = { tone: 'error', message: preparation.message };
      return false;
    }

    if (!task) {
      labState.notice = { tone: 'error', message: '请先选择作战地图。' };
      return false;
    }

    const snapshot = createRunCampaignSnapshot(task, labState.hangar.roster);
    labState.runConfig.deploymentScope = 'campaign';
    const generation = Math.max(this.generation, labState.campaign.generation) + 1;
    this.generation = generation;
    const startedAt = this.dependencies.now();
    const abortController = new AbortController();
    this.abortController = abortController;
    labState.campaign = {
      generation,
      status: 'running',
      startedAt,
      snapshot,
      members: snapshot.roster.map((sortie) => ({
        rosterEntryId: sortie.rosterEntry.id,
        sortie,
        status: 'queued',
      })),
    };
    labState.notice = undefined;

    const execution = this.executeCampaign(snapshot, generation, abortController.signal);
    this.execution = execution;
    try {
      await execution;
    } catch (error) {
      if (this.isCurrent(generation)) this.failCampaign(readableError(error, '编队评测意外终止。'));
    } finally {
      if (this.execution === execution) {
        this.execution = undefined;
        this.abortController = undefined;
      }
    }
    return true;
  }

  stopCampaignTracking(): boolean {
    if (!isCampaignActive()) return false;

    this.generation = Math.max(this.generation, labState.campaign.generation) + 1;
    this.abortController?.abort();
    const completedAt = this.dependencies.now();
    labState.campaign.generation = this.generation;
    labState.campaign.status = 'tracking_stopped';
    labState.campaign.completedAt = completedAt;
    for (const member of labState.campaign.members) {
      if (member.status === 'queued' || member.status === 'starting' || member.status === 'running') {
        member.status = 'tracking_stopped';
        member.completedAt = completedAt;
      }
    }
    labState.notice = {
      tone: 'info',
      message: '已停止前端跟踪；已经提交到 Bench CLI 的 Job 仍可能继续运行。',
    };
    return true;
  }

  private async executeCampaign(snapshot: RunCampaignSnapshot, generation: number, signal: AbortSignal): Promise<void> {
    let nextSortieIndex = 0;
    const worker = async (): Promise<void> => {
      while (this.isCurrent(generation)) {
        const sortieIndex = nextSortieIndex;
        nextSortieIndex += 1;
        if (sortieIndex >= snapshot.roster.length) return;
        await this.executeSortie(sortieIndex, snapshot, generation, signal);
      }
    };

    await Promise.all(Array.from({ length: Math.min(CAMPAIGN_CONCURRENCY, snapshot.roster.length) }, () => worker()));
    if (!this.isCurrent(generation)) return;
    this.finishCampaign();
  }

  private async executeSortie(
    sortieIndex: number,
    snapshot: RunCampaignSnapshot,
    generation: number,
    signal: AbortSignal
  ): Promise<void> {
    const member = this.currentMember(sortieIndex, generation);
    if (!member) return;
    member.status = 'starting';
    member.startedAt = this.dependencies.now();

    try {
      const job = await this.dependencies.api.startRun(member.sortie.input, signal);
      const currentMember = this.currentMember(sortieIndex, generation);
      if (!currentMember) return;
      const ownershipError = campaignJobOwnershipError(job, currentMember.sortie.input);
      if (ownershipError) {
        this.failMember(currentMember, ownershipError, job.completedAt);
        return;
      }
      currentMember.stage = job.stage;
      const jobId = job.jobId.trim();
      if (!jobId) {
        this.failMember(currentMember, 'Bench 未返回可轮询的 Job ID。');
        return;
      }
      const duplicateOwner = labState.campaign.members.find(
        (candidate, index) => index !== sortieIndex && candidate.jobId === jobId
      );
      if (duplicateOwner) {
        this.failMember(currentMember, `Bench 返回了重复的 Job ID：${jobId}。`);
        return;
      }
      currentMember.jobId = jobId;
      await this.consumeJob(sortieIndex, job, snapshot, generation, signal);
    } catch (error) {
      const currentMember = this.currentMember(sortieIndex, generation);
      if (!currentMember) return;
      this.failMember(currentMember, readableError(error, '无法启动或轮询 A3S Bench 评测。'));
    }
  }

  private async consumeJob(
    sortieIndex: number,
    initialJob: BenchRunJob,
    snapshot: RunCampaignSnapshot,
    generation: number,
    signal: AbortSignal
  ): Promise<void> {
    let job = initialJob;
    while (this.isCurrent(generation)) {
      const member = this.currentMember(sortieIndex, generation);
      if (!member) return;
      member.stage = job.stage;
      member.completedAt = job.completedAt;
      if (job.status === 'failed') {
        this.failMember(member, job.error ?? 'A3S Bench 评测失败。', job.completedAt);
        return;
      }
      if (job.status === 'completed') {
        await this.completeMember(sortieIndex, job, snapshot, generation, signal);
        return;
      }

      member.status = 'running';
      await this.dependencies.wait(this.dependencies.pollIntervalMs, signal);
      if (!this.isCurrent(generation)) return;
      job = await this.dependencies.api.run(member.jobId as string, signal);
      if (job.jobId.trim() !== member.jobId) {
        this.failMember(member, `Bench Job 归属不匹配：期望 ${member.jobId}，实际 ${job.jobId || '空值'}。`);
        return;
      }
      const ownershipError = campaignJobOwnershipError(job, member.sortie.input);
      if (ownershipError) {
        this.failMember(member, ownershipError, job.completedAt);
        return;
      }
    }
  }

  private async completeMember(
    sortieIndex: number,
    job: BenchRunJob,
    snapshot: RunCampaignSnapshot,
    generation: number,
    signal: AbortSignal
  ): Promise<void> {
    const member = this.currentMember(sortieIndex, generation);
    if (!member) return;
    const runId = job.result?.run_id.trim();
    if (!runId) {
      this.failMember(member, 'Bench 已结束任务，但没有返回可核验的 Run ID。', job.completedAt);
      return;
    }
    const duplicateOwner = labState.campaign.members.find(
      (candidate, index) => index !== sortieIndex && candidate.runId === runId
    );
    if (duplicateOwner) {
      this.failMember(member, `Bench 返回了重复的 Run ID：${runId}。`, job.completedAt);
      return;
    }
    member.runId = runId;

    let result: BenchRunResult | undefined;
    while (this.isCurrent(generation)) {
      result = await this.dependencies.api.result(runId, signal);
      const currentMember = this.currentMember(sortieIndex, generation);
      if (!currentMember) return;
      const resultOwnershipError = campaignResultOwnershipError(result, runId, snapshot.task.id);
      if (resultOwnershipError) {
        this.failMember(currentMember, resultOwnershipError, job.completedAt);
        return;
      }
      currentMember.result = result;
      currentMember.stage = result.status;
      if (result.status === 'failed') {
        this.failMember(currentMember, 'Bench 战报标记该次评测为失败。', job.completedAt);
        return;
      }
      if (result.status === 'completed') break;
      currentMember.status = 'running';
      await this.dependencies.wait(this.dependencies.pollIntervalMs, signal);
    }
    if (!this.isCurrent(generation) || !result) return;

    const currentMember = this.currentMember(sortieIndex, generation);
    if (!currentMember || result.status !== 'completed') return;
    currentMember.completedAt = job.completedAt ?? this.dependencies.now();
    const sortie = snapshot.roster[sortieIndex];
    try {
      this.dependencies.saveManifest(runId, createRunSortieSnapshot(snapshot.task, sortie.rosterEntry, sortie.input));
    } catch {
      // Bench results remain authoritative when optional browser metadata cannot be persisted.
    }
    currentMember.status = 'completed';
    currentMember.error = undefined;
  }

  private currentMember(sortieIndex: number, generation: number): LabCampaignMemberRun | undefined {
    if (!this.isCurrent(generation)) return undefined;
    return labState.campaign.members[sortieIndex];
  }

  private isCurrent(generation: number): boolean {
    return (
      this.generation === generation &&
      labState.campaign.generation === generation &&
      labState.campaign.status === 'running'
    );
  }

  private finishCampaign(): void {
    const completed = labState.campaign.members.filter((member) => member.status === 'completed').length;
    const failed = labState.campaign.members.filter((member) => member.status === 'failed').length;
    labState.campaign.completedAt = this.dependencies.now();
    if (failed === 0) {
      labState.campaign.status = 'completed';
      labState.notice = { tone: 'success', message: `编队评测完成：${completed} 架全部返回有效战报。` };
      return;
    }
    if (completed === 0) {
      labState.campaign.status = 'failed';
      labState.campaign.error = `编队中的 ${failed} 架评测均失败。`;
      labState.notice = { tone: 'error', message: labState.campaign.error };
      return;
    }
    labState.campaign.status = 'completed_with_failures';
    labState.campaign.error = `${failed} 架评测失败，${completed} 架返回有效战报。`;
    labState.notice = { tone: 'error', message: labState.campaign.error };
  }

  private failCampaign(message: string): void {
    const completedAt = this.dependencies.now();
    labState.campaign.status = 'failed';
    labState.campaign.completedAt = completedAt;
    labState.campaign.error = message;
    for (const member of labState.campaign.members) {
      if (member.status === 'queued' || member.status === 'starting' || member.status === 'running') {
        this.failMember(member, message, completedAt);
      }
    }
    labState.notice = { tone: 'error', message };
  }

  private failMember(member: LabCampaignMemberRun, message: string, completedAt?: string): void {
    member.status = 'failed';
    member.completedAt = completedAt ?? this.dependencies.now();
    member.error = message;
  }
}

export function createCampaignController(overrides: Partial<CampaignControllerDependencies> = {}): CampaignController {
  return new CampaignControllerImpl({ ...defaultDependencies, ...overrides });
}

export const campaignController = createCampaignController();

function readableError(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback;
}

function campaignJobOwnershipError(job: BenchRunJob, input: Readonly<StartBenchRunInput>): string | undefined {
  if (job.task !== input.task) return `Bench Job Task 归属不匹配：期望 ${input.task}，实际 ${job.task || '空值'}。`;
  if (job.candidate !== input.candidate) {
    return `Bench Job Candidate 归属不匹配：期望 ${input.candidate}，实际 ${job.candidate || '空值'}。`;
  }
  if (job.locked !== input.locked) {
    return `Bench Job 锁定模式归属不匹配：期望 ${String(input.locked)}，实际 ${String(job.locked)}。`;
  }
  if ((job.model ?? undefined) !== (input.model ?? undefined)) {
    return `Bench Job Model 归属不匹配：期望 ${input.model ?? '未指定'}，实际 ${job.model ?? '未指定'}。`;
  }
  return undefined;
}

function campaignResultOwnershipError(
  result: Readonly<BenchRunResult>,
  runId: string,
  taskId: string
): string | undefined {
  if (result.run_id !== runId) {
    return `Bench 战报 Run ID 不匹配：期望 ${runId}，实际 ${result.run_id || '空值'}。`;
  }
  if (result.task_id !== taskId) {
    return `Bench 战报 Task 归属不匹配：期望 ${taskId}，实际 ${result.task_id || '空值'}。`;
  }
  return undefined;
}

function waitForPoll(milliseconds: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const onAbort = (): void => {
      window.clearTimeout(timeout);
      reject(new DOMException('Campaign polling tracking stopped', 'AbortError'));
    };
    const timeout = window.setTimeout(() => {
      signal.removeEventListener('abort', onAbort);
      resolve();
    }, milliseconds);
    if (signal.aborted) {
      onAbort();
      return;
    }
    signal.addEventListener('abort', onAbort, { once: true });
  });
}
