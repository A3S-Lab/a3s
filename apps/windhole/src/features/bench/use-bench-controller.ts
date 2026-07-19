import { useCallback, useEffect, useRef } from 'react';
import { demoTasks } from '../../data/demo-tasks';
import { benchApi } from '../../lib/api';
import { createBenchRunInput } from '../../lib/bench-run';
import {
  createRunSortieSnapshot,
  defaultTunnelParameters,
  isBenchRunActive,
  labState,
  selectedTask,
} from '../../state/lab-state';
import type {
  BenchDeploymentScope,
  BenchDoctorResult,
  BenchRunJob,
  BenchRunResult,
  BenchTask,
  StartBenchRunInput,
  WindTunnelParameters,
} from '../../types/bench';
import type { EvaluationEffort } from '../../types/evaluation';
import { candidateRunStatus } from '../hangar/hangar-configuration';
import { activeHangarRosterEntry, updateActiveHangarRosterEntry } from '../hangar/hangar-roster-state';
import { campaignController } from './campaign-controller';
import { replaceTaskCatalog } from './restored-task-catalog';
import { saveSortieManifest } from './sortie-manifest-store';
import { taskDeploymentStatus } from './task-deployment-status';

const JOB_POLL_INTERVAL_MS = 900;
const EXACT_RESULT_POLL_LIMIT = 5;

export interface BenchController {
  refresh: () => Promise<void>;
  selectTask: (taskId: string) => Promise<void>;
  setQuery: (query: string) => void;
  setCategory: (category: string) => void;
  setIncludeBlocked: (value: boolean) => void;
  setCandidate: (candidate: string) => void;
  setCandidateLock: (candidateLock: string) => void;
  setModel: (model: string) => void;
  setEffort: (effort: EvaluationEffort) => void;
  setTaskLock: (taskLock: string) => void;
  setLocked: (locked: boolean) => void;
  setDeploymentScope: (scope: BenchDeploymentScope) => void;
  setTunnelParameter: <Key extends keyof WindTunnelParameters>(key: Key, value: WindTunnelParameters[Key]) => void;
  resetTunnel: () => void;
  startRun: () => Promise<void>;
  startCampaign: () => Promise<boolean>;
  stopCampaignTracking: () => boolean;
  dismissNotice: () => void;
}

export function useBenchController(): BenchController {
  const connectionGeneration = useRef(0);
  const taskSelectionGenerationRef = useRef(0);
  const runGenerationRef = useRef(0);
  const startRunInFlightRef = useRef(false);

  const refresh = useCallback(async () => {
    if (isBenchRunActive()) {
      labState.notice = { tone: 'error', message: '评测运行中，完成后才能重新检测 Bench 连接。' };
      return;
    }
    const requestGeneration = ++connectionGeneration.current;
    taskSelectionGenerationRef.current += 1;
    labState.connection.mode = 'checking';
    labState.connection.message = '正在连接本机 A3S Bench…';
    try {
      const health = await benchApi.health();
      const [catalogResult, doctorResult] = await Promise.allSettled([benchApi.tasks(true), benchApi.doctor()]);
      if (requestGeneration !== connectionGeneration.current) return;
      if (catalogResult.status === 'rejected') throw catalogResult.reason;
      replaceTaskCatalog(catalogResult.value.tasks);
      labState.connection.health = health;
      if (doctorResult.status === 'fulfilled') {
        labState.connection.doctor = doctorResult.value;
        if (isBenchDoctorReady(doctorResult.value)) {
          labState.connection.mode = 'live';
          labState.connection.message = `A3S Bench ${health.version} · ${doctorResult.value.runtime.provider}`;
        } else {
          labState.connection.mode = 'preview';
          labState.connection.message = `A3S Bench ${health.version} · Runtime 未就绪：${doctorResult.value.runtime.detail}`;
        }
      } else {
        labState.connection.doctor = undefined;
        labState.connection.mode = 'preview';
        labState.connection.message = readableError(doctorResult.reason, 'A3S Bench Doctor 未就绪，无法部署评测');
      }
    } catch (error) {
      if (requestGeneration !== connectionGeneration.current) return;
      replaceTaskCatalog(demoTasks);
      labState.connection.health = undefined;
      labState.connection.doctor = undefined;
      labState.connection.mode = 'preview';
      labState.connection.message = readableError(error, 'CLI 桥接未连接，当前展示预览数据');
    }
  }, []);

  useEffect(() => {
    void refresh();
    return () => {
      connectionGeneration.current += 1;
      taskSelectionGenerationRef.current += 1;
      runGenerationRef.current += 1;
    };
  }, [refresh]);

  const selectTask = useCallback(async (taskId: string) => {
    if (isBenchRunActive()) {
      labState.notice = { tone: 'error', message: '评测运行中，不能切换作战地图。' };
      return;
    }
    const selectionGeneration = ++taskSelectionGenerationRef.current;
    labState.catalog.selectedTaskId = taskId;
    if (labState.connection.mode !== 'live') return;
    try {
      const detail = await benchApi.task(taskId, true);
      if (selectionGeneration !== taskSelectionGenerationRef.current || labState.catalog.selectedTaskId !== taskId) {
        return;
      }
      const index = labState.catalog.tasks.findIndex((task) => task.id === taskId);
      if (index >= 0) labState.catalog.tasks[index] = { ...labState.catalog.tasks[index], ...detail.task };
    } catch (error) {
      if (selectionGeneration !== taskSelectionGenerationRef.current || labState.catalog.selectedTaskId !== taskId) {
        return;
      }
      labState.notice = { tone: 'error', message: readableError(error, '无法读取任务详情') };
    }
  }, []);

  const bindExactResultTask = useCallback(
    async (result: Readonly<BenchRunResult>, runGeneration: number): Promise<string | undefined> => {
      const sortie = labState.run.sortie;
      if (!sortie) return 'Bench 返回了战报，但当前没有可核验的冻结出击快照。';

      const resultTaskId = normalizedIdentifier(result.task_id);
      if (!sortie.input.locked) {
        if (resultTaskId && resultTaskId !== sortie.task.id) {
          return `Bench 战报 Task 归属不匹配：期望 ${sortie.task.id}，实际 ${resultTaskId}。`;
        }
        if (result.status === 'completed' && !resultTaskId) {
          return 'Bench 完成战报没有返回可核验的 Task ID。';
        }
        return undefined;
      }

      if (!resultTaskId) {
        return 'Task Lock 战报没有返回可核验的真实 Task ID，不能把场景预览归档为评测地图。';
      }

      let resolvedTask = labState.catalog.tasks.find((task) => task.id === resultTaskId);
      if (!resolvedTask) {
        try {
          const detail = await benchApi.task(resultTaskId, true);
          if (runGeneration !== runGenerationRef.current) return undefined;
          const detailTaskId = normalizedIdentifier(detail.task.id);
          if (detailTaskId !== resultTaskId) {
            return `Bench Task 详情归属不匹配：期望 ${resultTaskId}，实际 ${detailTaskId || '空值'}。`;
          }
          resolvedTask = { ...detail.task };
          labState.catalog.tasks.push(resolvedTask);
        } catch (error) {
          return readableError(error, `无法核验 Task Lock 解析出的任务 ${resultTaskId}`);
        }
      }

      if (runGeneration !== runGenerationRef.current) return undefined;
      labState.run.sortie = createRunSortieSnapshot(resolvedTask, sortie.rosterEntry, sortie.input);
      taskSelectionGenerationRef.current += 1;
      labState.catalog.selectedTaskId = resolvedTask.id;
      return undefined;
    },
    []
  );

  const pollExactResult = useCallback(
    async (runId: string, jobCompletedAt: string | undefined, runGeneration: number) => {
      for (let attempt = 0; attempt < EXACT_RESULT_POLL_LIMIT; attempt += 1) {
        if (runGeneration !== runGenerationRef.current) return;
        const result = await benchApi.result(runId);
        if (runGeneration !== runGenerationRef.current) return;
        const resultRunId = normalizedIdentifier(result.run_id);
        if (resultRunId !== runId) {
          failCurrentRun(`Bench 战报 Run ID 不匹配：期望 ${runId}，实际 ${resultRunId || '空值'}。`);
          return;
        }

        labState.run.stage = result.status;
        labState.run.completedAt = jobCompletedAt;
        if (result.status === 'completed' || result.status === 'failed') {
          const taskIntegrityError = await bindExactResultTask(result, runGeneration);
          if (runGeneration !== runGenerationRef.current) return;
          if (taskIntegrityError) {
            failCurrentRun(taskIntegrityError);
            return;
          }
        }
        if (result.status === 'completed') {
          labState.run.result = result;
          labState.run.error = undefined;
          if (labState.run.sortie) saveSortieManifest(runId, labState.run.sortie);
          labState.notice = { tone: 'success', message: `试验完成，得分 ${result.score ?? '—'}。` };
          return;
        }
        if (result.status === 'failed') {
          labState.run.result = result;
          if (labState.run.sortie) saveSortieManifest(runId, labState.run.sortie);
          failCurrentRun('Bench 战报标记该次评测为失败。');
          return;
        }
        if (attempt === EXACT_RESULT_POLL_LIMIT - 1) {
          failCurrentRun(`Bench Job 已结束，但精确战报仍处于非终态 ${result.status}。`);
          return;
        }
        await wait(JOB_POLL_INTERVAL_MS);
      }
    },
    [bindExactResultTask]
  );

  const pollRun = useCallback(
    async (initialJob: BenchRunJob, input: Readonly<StartBenchRunInput>, runGeneration: number) => {
      const initialIntegrityError = benchJobIntegrityError(initialJob, input);
      if (initialIntegrityError) {
        failCurrentRun(initialIntegrityError);
        return;
      }
      const jobId = initialJob.jobId.trim();
      labState.run.jobId = jobId;
      let job = initialJob;

      while (runGeneration === runGenerationRef.current) {
        const integrityError = benchJobIntegrityError(job, input, jobId);
        if (integrityError) {
          failCurrentRun(integrityError);
          return;
        }
        labState.run.completedAt = job.completedAt;
        labState.run.error = job.error;
        if (job.status === 'completed') {
          const runId = normalizedIdentifier(job.result?.run_id);
          if (!runId) {
            failCurrentRun('Bench 已结束任务，但没有返回可核验的 Run ID。');
            return;
          }
          labState.run.runId = runId;
          await pollExactResult(runId, job.completedAt, runGeneration);
          return;
        }
        if (job.status === 'failed') {
          const failedRunId = normalizedLocalRunId(job.runId);
          if (failedRunId) labState.run.runId = failedRunId;
          failCurrentRun(job.error ?? 'A3S Bench 试验失败');
          return;
        }
        labState.run.stage = job.stage;
        await wait(JOB_POLL_INTERVAL_MS);
        if (runGeneration !== runGenerationRef.current) return;
        job = await benchApi.run(jobId);
      }
    },
    [pollExactResult]
  );

  const startRun = useCallback(async () => {
    if (startRunInFlightRef.current || isBenchRunActive()) {
      labState.notice = { tone: 'error', message: '已有评测正在运行，请等待当前出击结束。' };
      return;
    }
    if (labState.connection.mode === 'checking') {
      labState.notice = { tone: 'error', message: 'Bench 连接与 Runtime 自检尚未完成，暂不能部署评测。' };
      return;
    }
    if (labState.connection.mode !== 'live') {
      labState.notice = { tone: 'error', message: '当前仅可查看预览数据；连接可用的 A3S Bench 后才能部署评测。' };
      return;
    }
    if (!isBenchDoctorReady(labState.connection.doctor)) {
      labState.notice = { tone: 'error', message: 'Bench Runtime Doctor 未就绪，不能部署评测。' };
      return;
    }
    const task = selectedTask();
    if (!task) return;
    const activeEntry = activeHangarRosterEntry();
    if (!activeEntry) {
      labState.notice = { tone: 'error', message: '当前没有可部署的机库组合。' };
      return;
    }
    if (!labState.runConfig.locked) {
      const taskStatus = taskDeploymentStatus(task, labState.connection.doctor);
      if (!taskStatus.deployable) {
        labState.notice = { tone: 'error', message: taskStatus.message };
        return;
      }
    }
    const input = createBenchRunInput({
      taskId: task.id,
      candidate: activeEntry.candidate,
      candidateLock: labState.runConfig.candidateLock,
      model: activeEntry.model,
      taskLock: labState.runConfig.taskLock,
      locked: labState.runConfig.locked,
    });
    if (!input.task) {
      labState.notice = { tone: 'error', message: '请先填写 Task Lock 文件。' };
      return;
    }
    if (labState.runConfig.locked && !input.candidate) {
      labState.notice = {
        tone: 'error',
        message: '请先填写 Candidate Lock 文件。',
      };
      return;
    }
    if (!labState.runConfig.locked) {
      const candidateStatus = candidateRunStatus(input.candidate, input.model ?? '');
      if (!candidateStatus.deployable) {
        labState.notice = { tone: 'error', message: candidateStatus.message };
        return;
      }
    }
    const sortie = createRunSortieSnapshot(task, activeEntry, input);
    labState.runConfig.deploymentScope = 'single';
    startRunInFlightRef.current = true;
    taskSelectionGenerationRef.current += 1;
    const currentRunGeneration = ++runGenerationRef.current;
    labState.run = {
      mode: 'live',
      stage: 'planned',
      startedAt: new Date().toISOString(),
      sortie,
    };
    labState.notice = undefined;

    try {
      const job = await benchApi.startRun(sortie.input);
      if (currentRunGeneration !== runGenerationRef.current) return;
      await pollRun(job, sortie.input, currentRunGeneration);
    } catch (error) {
      if (currentRunGeneration !== runGenerationRef.current) return;
      failCurrentRun(readableError(error, '无法启动 A3S Bench 试验'));
    } finally {
      startRunInFlightRef.current = false;
    }
  }, [pollRun]);

  return {
    refresh,
    selectTask,
    setQuery: (query) => {
      labState.catalog.query = query;
    },
    setCategory: (category) => {
      labState.catalog.category = category;
    },
    setIncludeBlocked: (value) => {
      labState.catalog.includeBlocked = value;
    },
    setCandidate: (candidate) => {
      changeRunPreparation(() => updateActiveHangarRosterEntry({ candidate }));
    },
    setCandidateLock: (candidateLock) => {
      changeRunPreparation(() => {
        labState.runConfig.candidateLock = candidateLock;
      });
    },
    setModel: (model) => {
      changeRunPreparation(() => updateActiveHangarRosterEntry({ model }));
    },
    setEffort: (effort) => {
      changeRunPreparation(() => updateActiveHangarRosterEntry({ effort }));
    },
    setTaskLock: (taskLock) => {
      changeRunPreparation(() => {
        labState.runConfig.taskLock = taskLock;
      });
    },
    setLocked: (locked) => {
      changeRunPreparation(() => {
        labState.runConfig.locked = locked;
        if (locked) labState.runConfig.deploymentScope = 'single';
      });
    },
    setDeploymentScope: (scope) => {
      changeRunPreparation(() => {
        if (scope === 'campaign' && labState.runConfig.locked) {
          labState.notice = { tone: 'error', message: '锁文件模式仅支持单机出击。' };
          return;
        }
        labState.runConfig.deploymentScope = scope;
      });
    },
    setTunnelParameter: (key, value) => {
      labState.tunnel[key] = value;
    },
    resetTunnel: () => {
      Object.assign(labState.tunnel, defaultTunnelParameters);
    },
    startRun,
    startCampaign: () => campaignController.startCampaign(),
    stopCampaignTracking: () => campaignController.stopCampaignTracking(),
    dismissNotice: () => {
      labState.notice = undefined;
    },
  };
}

function readableError(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback;
}

export function isBenchDoctorReady(doctor: BenchDoctorResult | undefined): boolean {
  return (
    doctor?.runtime.ready === true &&
    doctor.runtime.provider.trim().length > 0 &&
    doctor.runtime.detail.trim().length > 0
  );
}

function changeRunPreparation(change: () => unknown): void {
  if (isBenchRunActive()) {
    labState.notice = { tone: 'error', message: '评测运行中，不能修改当前出击配置。' };
    return;
  }
  change();
}

function failCurrentRun(message: string): void {
  labState.run.stage = 'failed';
  labState.run.completedAt ??= new Date().toISOString();
  labState.run.error = message;
  labState.notice = { tone: 'error', message };
}

export function benchJobIntegrityError(
  job: Readonly<BenchRunJob>,
  expectedInput: Readonly<StartBenchRunInput>,
  expectedJobId?: string
): string | undefined {
  const jobId = normalizedIdentifier(job.jobId);
  if (!jobId) return 'Bench 未返回可轮询的 Job ID。';
  if (expectedJobId !== undefined && jobId !== expectedJobId) {
    return `Bench Job 归属不匹配：期望 ${expectedJobId}，实际 ${jobId}。`;
  }
  if (job.task !== expectedInput.task) return 'Bench Job 的 task 归属与冻结出击输入不匹配。';
  if (job.candidate !== expectedInput.candidate) return 'Bench Job 的 candidate 归属与冻结出击输入不匹配。';
  if (job.model !== expectedInput.model) return 'Bench Job 的 model 归属与冻结出击输入不匹配。';
  if (job.locked !== expectedInput.locked) return 'Bench Job 的 locked 归属与冻结出击输入不匹配。';
  if (job.runId !== undefined && !normalizedLocalRunId(job.runId)) return 'Bench Job 返回了无效的失败 Run ID。';
  return undefined;
}

function normalizedIdentifier(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

function normalizedLocalRunId(value: unknown): string {
  const runId = normalizedIdentifier(value);
  return /^local-[A-Za-z0-9-]{1,122}$/u.test(runId) ? runId : '';
}

function wait(milliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

export function filteredTasks(
  tasks: readonly BenchTask[],
  query: string,
  category: string,
  includeBlocked: boolean
): BenchTask[] {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  return tasks.filter((task) => {
    if (!includeBlocked && task.availability !== 'ready') return false;
    if (category !== 'all' && task.category !== category) return false;
    if (!normalizedQuery) return true;
    return `${task.name} ${task.id} ${task.category}`.toLocaleLowerCase().includes(normalizedQuery);
  });
}
