import { subscribe } from 'valtio';
import { createRunSortieSnapshot, type LabRunState, labState, type RunSortieSnapshot } from '../../state/lab-state';
import type { BenchRunResult, BenchRunStage, BenchTask } from '../../types/bench';
import { reconcileRestoredTaskCatalog } from './restored-task-catalog';
import {
  isSingleRunManifestTimestamp,
  parseSingleRunManifest,
  SINGLE_RUN_MANIFEST_VERSION,
} from './single-run-manifest-codec';

export const SINGLE_RUN_MANIFEST_STORAGE_KEY = 'a3s-agent-evaluation.single-run.v1';

const MAX_SERIALIZED_LENGTH = 128 * 1_024;

export type PersistedSingleRunStatus = 'active' | 'completed' | 'failed' | 'tracking_stopped';

interface StorageAdapter {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export interface PersistedSingleRun {
  mode: 'live';
  status: PersistedSingleRunStatus;
  stage: BenchRunStage;
  jobId?: string;
  runId?: string;
  startedAt: string;
  completedAt?: string;
  trackingStoppedAt?: string;
  sortie: RunSortieSnapshot;
  result?: BenchRunResult;
  error?: string;
}

export interface StoredSingleRunManifest {
  version: typeof SINGLE_RUN_MANIFEST_VERSION;
  savedAt: string;
  run: PersistedSingleRun;
}

type PersistenceFingerprint = string | null | undefined;

export function restoreSingleRunManifest(
  storage: StorageAdapter = window.localStorage,
  now: () => string = () => new Date().toISOString()
): boolean {
  try {
    const serialized = storage.getItem(SINGLE_RUN_MANIFEST_STORAGE_KEY);
    if (!serialized || serialized.length > MAX_SERIALIZED_LENGTH) return false;
    const manifest = parseSingleRunManifest(JSON.parse(serialized) as unknown);
    if (!manifest) return false;

    if (manifest.run.status === 'active') {
      const restoredAt = now();
      if (!isSingleRunManifestTimestamp(restoredAt) || Date.parse(restoredAt) < Date.parse(manifest.run.startedAt)) {
        return false;
      }
      labState.run = restoreRun(manifest.run, restoredAt);
    } else {
      labState.run = restoreRun(manifest.run);
    }
    reconcileRestoredTaskCatalog();
    return true;
  } catch {
    return false;
  }
}

export function startSingleRunManifestPersistence(
  storage: StorageAdapter = window.localStorage,
  now: () => string = () => new Date().toISOString()
): () => void {
  let previousRun = persistCurrentRun(storage, now);
  return subscribe(labState, () => {
    previousRun = persistCurrentRun(storage, now, previousRun);
  });
}

function persistCurrentRun(
  storage: StorageAdapter,
  now: () => string,
  previousRun?: PersistenceFingerprint
): PersistenceFingerprint {
  try {
    const run = serializeRun(labState.run);
    if (!run) {
      if (labState.run.stage === 'idle' && previousRun !== null) storage.removeItem(SINGLE_RUN_MANIFEST_STORAGE_KEY);
      return labState.run.stage === 'idle' ? null : previousRun;
    }

    const serializedRun = JSON.stringify(run);
    if (serializedRun === previousRun) return previousRun;
    const manifest = {
      version: SINGLE_RUN_MANIFEST_VERSION,
      savedAt: now(),
      run,
    } satisfies StoredSingleRunManifest;
    if (!parseSingleRunManifest(manifest)) return previousRun;
    const serialized = JSON.stringify(manifest);
    if (serialized.length > MAX_SERIALIZED_LENGTH) return previousRun;
    storage.setItem(SINGLE_RUN_MANIFEST_STORAGE_KEY, serialized);
    return serializedRun;
  } catch {
    // The live state and exact Bench Job/Run APIs remain authoritative when browser persistence is unavailable.
    return previousRun;
  }
}

function serializeRun(run: LabRunState): PersistedSingleRun | undefined {
  if (run.mode !== 'live' || run.stage === 'idle' || !run.startedAt || !run.sortie) return undefined;

  const status: PersistedSingleRunStatus =
    run.trackingStatus === 'tracking_stopped'
      ? 'tracking_stopped'
      : run.stage === 'completed' && run.result
        ? 'completed'
        : run.stage === 'failed'
          ? 'failed'
          : 'active';
  return {
    mode: 'live',
    status,
    stage: run.stage,
    jobId: run.jobId,
    runId: run.runId,
    startedAt: run.startedAt,
    completedAt: run.completedAt,
    trackingStoppedAt: run.trackingStoppedAt,
    sortie: cloneSortie(run.sortie),
    result: run.result ? cloneResult(run.result) : undefined,
    error: run.error,
  };
}

function restoreRun(run: PersistedSingleRun, restoredAt?: string): LabRunState {
  return {
    mode: 'live',
    stage: run.stage,
    trackingStatus: run.status === 'active' || run.status === 'tracking_stopped' ? 'tracking_stopped' : undefined,
    trackingStoppedAt: run.status === 'active' ? restoredAt : run.trackingStoppedAt,
    jobId: run.jobId,
    runId: run.runId,
    startedAt: run.startedAt,
    completedAt: run.completedAt,
    sortie: cloneSortie(run.sortie),
    result: run.result ? cloneResult(run.result) : undefined,
    error: run.error,
  };
}

function cloneSortie(sortie: Readonly<RunSortieSnapshot>): RunSortieSnapshot {
  const task: BenchTask = {
    id: sortie.task.id,
    path: sortie.task.path,
    name: sortie.task.name,
    category: sortie.task.category,
    execution_class: sortie.task.execution_class,
    availability: sortie.task.availability,
    availability_reason: sortie.task.availability_reason,
    admission: sortie.task.admission,
    admission_reason: sortie.task.admission_reason,
    provenance_ref: sortie.task.provenance_ref,
    description: sortie.task.description,
    tags: sortie.task.tags ? [...sortie.task.tags] : undefined,
  };
  return createRunSortieSnapshot(task, { ...sortie.rosterEntry }, { ...sortie.input });
}

function cloneResult(result: BenchRunResult): BenchRunResult {
  return {
    status: result.status,
    governance_status: result.governance_status,
    run_id: result.run_id,
    task_reference: result.task_reference,
    task_id: result.task_id,
    score: result.score,
    result_path: result.result_path,
    primary_metric: result.primary_metric,
    runtime_provider: result.runtime_provider,
    model: result.model,
    result_digest: result.result_digest,
    task_lock_digest: result.task_lock_digest,
    candidate_lock_digest: result.candidate_lock_digest,
    candidate_identity: result.candidate_identity,
    judge_identity: result.judge_identity,
    model_usage: result.model_usage ? { ...result.model_usage } : result.model_usage,
  };
}
