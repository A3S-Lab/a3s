import { useCallback, useRef } from 'react';
import { benchApi } from '../../lib/api';
import { createRunSortieSnapshot, labState, type RunSortieSnapshot } from '../../state/lab-state';
import type { BenchRunResult, BenchTask } from '../../types/bench';
import { loadSortieManifest, saveSortieManifest } from '../bench/sortie-manifest-store';

export interface ResultController {
  setRunId: (runId: string) => void;
  loadResult: (runId?: string) => Promise<void>;
  loadLatest: () => Promise<void>;
  openCurrentRun: () => Promise<void>;
}

export function useResultController(): ResultController {
  const requestGeneration = useRef(0);

  const loadResult = useCallback(async (runId?: string) => {
    const generation = ++requestGeneration.current;
    const normalized = (runId ?? labState.results.runId).trim();
    if (!normalized) {
      labState.results.loading = false;
      labState.results.error = '请输入运行 ID，或读取最近一次结果。';
      return;
    }
    if (labState.connection.mode !== 'live') {
      labState.results.loading = false;
      const cached = cachedResult(normalized);
      if (cached) {
        labState.results.runId = normalized;
        labState.results.record = cached.record;
        labState.results.sortie = cached.sortie;
        labState.results.error = undefined;
      } else {
        labState.results.error = '连接本机 A3S Bench 后才能读取正式结果。';
      }
      return;
    }
    labState.results.loading = true;
    labState.results.runId = normalized;
    labState.results.error = undefined;
    try {
      const expectedSortie = expectedSortieForRunId(normalized);
      const result = await benchApi.result(normalized);
      if (generation !== requestGeneration.current) return;
      if (result.run_id !== normalized) {
        throw new Error(`Bench 战报 Run ID 不匹配：期望 ${normalized}，实际 ${result.run_id || '空值'}。`);
      }
      let verifiedSortie = expectedSortie ?? loadSortieManifest(result.run_id);
      const lockedSingleRun = isInterruptedLockedSingleRun(normalized, expectedSortie);
      if (lockedSingleRun && (result.status === 'completed' || result.status === 'failed')) {
        const resolution = await resolveLockedSingleRunSortie(result, expectedSortie);
        if (generation !== requestGeneration.current) return;
        verifiedSortie = resolution.sortie;
        if (isInterruptedLockedSingleRun(normalized, expectedSortie)) {
          if (!labState.catalog.tasks.some((task) => task.id === resolution.task.id)) {
            labState.catalog.tasks.push({
              ...resolution.task,
              tags: resolution.task.tags ? [...resolution.task.tags] : undefined,
            });
          }
          labState.run.sortie = resolution.sortie;
          labState.catalog.selectedTaskId = resolution.task.id;
        }
      } else if (!lockedSingleRun) {
        assertResultMatchesSortie(result, expectedSortie);
      }
      labState.results.record = result;
      labState.results.runId = result.run_id;
      labState.results.sortie = verifiedSortie;
      reconcileSingleRun(result, verifiedSortie);
      reconcileCampaignMember(result, verifiedSortie);
    } catch (error) {
      if (generation !== requestGeneration.current) return;
      labState.results.record = undefined;
      labState.results.sortie = undefined;
      labState.results.error = readableError(error, '无法读取该结果');
    } finally {
      if (generation === requestGeneration.current) labState.results.loading = false;
    }
  }, []);

  const loadLatest = useCallback(async () => {
    const generation = ++requestGeneration.current;
    if (labState.connection.mode !== 'live') {
      labState.results.loading = false;
      if (labState.run.result) {
        labState.results.record = labState.run.result;
        labState.results.runId = labState.run.result.run_id;
        labState.results.sortie = labState.run.sortie ?? loadSortieManifest(labState.run.result.run_id);
        labState.results.error = undefined;
      } else {
        labState.results.error = '当前没有可用战报；请先完成一次地图评测。';
      }
      return;
    }
    labState.results.loading = true;
    labState.results.error = undefined;
    try {
      const result = await benchApi.latestResult();
      if (generation !== requestGeneration.current) return;
      const sortie = expectedSortieForRunId(result.run_id);
      assertResultMatchesSortie(result, sortie);
      labState.results.record = result;
      labState.results.runId = result.run_id;
      labState.results.sortie = sortie;
    } catch (error) {
      if (generation !== requestGeneration.current) return;
      labState.results.record = undefined;
      labState.results.sortie = undefined;
      labState.results.error = readableError(error, '没有可读取的本地结果');
    } finally {
      if (generation === requestGeneration.current) labState.results.loading = false;
    }
  }, []);

  const openCurrentRun = useCallback(async () => {
    labState.workspace = 'results';
    const result = labState.run.result;
    if (!result) {
      const runId = labState.run.runId?.trim();
      if (runId) {
        await loadResult(runId);
        return;
      }
      await loadLatest();
      return;
    }
    labState.results.runId = result.run_id;
    labState.results.record = result;
    labState.results.sortie = labState.run.sortie ?? loadSortieManifest(result.run_id);
    labState.results.error = undefined;
    if (labState.connection.mode === 'live') await loadResult(result.run_id);
  }, [loadLatest, loadResult]);

  return {
    setRunId: (runId) => {
      requestGeneration.current += 1;
      labState.results.runId = runId;
      labState.results.loading = false;
    },
    loadResult,
    loadLatest,
    openCurrentRun,
  };
}

function expectedSortieForRunId(runId: string): RunSortieSnapshot | undefined {
  if (labState.run.runId?.trim() === runId && labState.run.sortie) return labState.run.sortie;

  const member = labState.campaign.members.find((candidate) => candidate.runId === runId);
  const campaignTask = labState.campaign.snapshot?.task;
  if (member && campaignTask) {
    return createRunSortieSnapshot(campaignTask, member.sortie.rosterEntry, member.sortie.input);
  }

  return loadSortieManifest(runId);
}

function assertResultMatchesSortie(result: Readonly<BenchRunResult>, sortie: RunSortieSnapshot | undefined): void {
  if (!sortie) return;
  if (result.task_id !== sortie.task.id) {
    throw new Error(`Bench 战报 Task 归属不匹配：期望 ${sortie.task.id}，实际 ${result.task_id || '空值'}。`);
  }
}

function isInterruptedLockedSingleRun(
  runId: string,
  sortie: RunSortieSnapshot | undefined
): sortie is RunSortieSnapshot {
  const currentSortie = labState.run.sortie;
  return Boolean(
    sortie?.input.locked &&
      currentSortie?.input.locked &&
      labState.run.trackingStatus === 'tracking_stopped' &&
      labState.run.runId?.trim() === runId &&
      currentSortie.task.id === sortie.task.id
  );
}

async function resolveLockedSingleRunSortie(
  result: Readonly<BenchRunResult>,
  previewSortie: RunSortieSnapshot
): Promise<{ sortie: RunSortieSnapshot; task: BenchTask }> {
  const taskId = result.task_id?.trim();
  if (!taskId) {
    throw new Error('Task Lock 战报没有返回可核验的真实 Task ID，不能把场景预览归档为评测地图。');
  }

  let resolvedTask = labState.catalog.tasks.find((task) => task.id === taskId);
  if (!resolvedTask) {
    try {
      const detail = await benchApi.task(taskId, true);
      const detailTaskId = detail.task.id.trim();
      if (detailTaskId !== taskId) {
        throw new Error(`Bench Task 详情归属不匹配：期望 ${taskId}，实际 ${detailTaskId || '空值'}。`);
      }
      resolvedTask = { ...detail.task };
    } catch (error) {
      throw new Error(readableError(error, `无法核验 Task Lock 解析出的任务 ${taskId}`));
    }
  }

  return {
    task: resolvedTask,
    sortie: createRunSortieSnapshot(resolvedTask, previewSortie.rosterEntry, previewSortie.input),
  };
}

function reconcileSingleRun(result: BenchRunResult, sortie: RunSortieSnapshot | undefined): void {
  const currentSortie = labState.run.sortie;
  if (
    labState.run.trackingStatus !== 'tracking_stopped' ||
    labState.run.runId?.trim() !== result.run_id ||
    !currentSortie ||
    !sortie ||
    currentSortie.task.id !== sortie.task.id
  ) {
    return;
  }

  if (result.status !== 'completed' && result.status !== 'failed') {
    if (result.status !== 'idle') labState.run.stage = result.status;
    labState.run.result = undefined;
    labState.run.error = undefined;
    return;
  }

  labState.run.result = result;
  labState.run.stage = result.status;
  labState.run.trackingStatus = undefined;
  labState.run.trackingStoppedAt = undefined;
  labState.run.completedAt ??= new Date().toISOString();
  labState.run.error = result.status === 'failed' ? 'Bench 战报标记该次评测为失败。' : undefined;
  try {
    saveSortieManifest(result.run_id, sortie);
  } catch {
    // The exact Bench result remains authoritative when optional browser metadata cannot be persisted.
  }
}

function reconcileCampaignMember(result: BenchRunResult, sortie: RunSortieSnapshot | undefined): void {
  const member = labState.campaign.members.find((candidate) => candidate.runId === result.run_id);
  if (!member || !sortie) return;

  member.result = result;
  member.stage = result.status;
  if (result.status === 'completed') {
    member.status = 'completed';
    member.completedAt ??= new Date().toISOString();
    member.error = undefined;
    try {
      saveSortieManifest(result.run_id, sortie);
    } catch {
      // The exact Bench result remains authoritative when optional browser metadata cannot be persisted.
    }
  } else if (result.status === 'failed') {
    member.status = 'failed';
    member.completedAt ??= new Date().toISOString();
    member.error = member.error?.trim() || 'Bench 战报标记该次评测为失败。';
  } else if (member.status !== 'running') {
    member.status = 'tracking_stopped';
  }

  reconcileCampaignStatus();
}

function reconcileCampaignStatus(): void {
  if (labState.campaign.status === 'running' || labState.campaign.members.length === 0) return;
  const completed = labState.campaign.members.filter((member) => member.status === 'completed').length;
  const failed = labState.campaign.members.filter((member) => member.status === 'failed').length;
  if (completed + failed !== labState.campaign.members.length) {
    labState.campaign.status = 'tracking_stopped';
    return;
  }
  labState.campaign.completedAt ??= new Date().toISOString();
  if (failed === 0) labState.campaign.status = 'completed';
  else if (completed === 0) labState.campaign.status = 'failed';
  else labState.campaign.status = 'completed_with_failures';
}

function cachedResult(runId: string): { record: BenchRunResult; sortie?: RunSortieSnapshot } | undefined {
  if (labState.run.result?.run_id === runId) {
    return {
      record: labState.run.result,
      sortie: labState.run.sortie ?? loadSortieManifest(runId),
    };
  }

  const member = labState.campaign.members.find(
    (candidate) => candidate.status === 'completed' && candidate.runId === runId && candidate.result?.run_id === runId
  );
  if (!member?.result) return undefined;

  const campaignTask = labState.campaign.snapshot?.task;
  return {
    record: member.result,
    sortie: campaignTask
      ? createRunSortieSnapshot(campaignTask, member.sortie.rosterEntry, member.sortie.input)
      : loadSortieManifest(runId),
  };
}

function readableError(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback;
}
