import { createRunSortieSnapshot, type RunSortieSnapshot } from '../../state/lab-state';
import type { BenchRunResult, BenchRunStage, BenchTask, StartBenchRunInput } from '../../types/bench';
import { HANGAR_AIRFRAME_OPTIONS, HANGAR_PILOT_OPTIONS, type HangarRosterEntry } from '../hangar/hangar-configuration';
import type {
  PersistedSingleRun,
  PersistedSingleRunStatus,
  StoredSingleRunManifest,
} from './single-run-manifest-store';

export const SINGLE_RUN_MANIFEST_VERSION = 1 as const;

const MAX_TAGS = 64;
const AIRFRAMES = new Set(HANGAR_AIRFRAME_OPTIONS.map((option) => option.id));
const PILOTS = new Set(HANGAR_PILOT_OPTIONS.map((option) => option.id));
const EFFORTS = new Set(['none', 'minimal', 'low', 'medium', 'high', 'xhigh']);
const AVAILABILITY = new Set(['ready', 'blocked']);
const ADMISSION = new Set(['admitted', 'quarantined']);
const EXECUTION_CLASSES = new Set(['conformance', 'long_horizon']);
const RUN_STAGES = new Set<BenchRunStage>([
  'idle',
  'planned',
  'running',
  'runtime_ready',
  'inputs_resolved',
  'candidate_running',
  'candidate_completed',
  'judging',
  'completed',
  'failed',
]);
const ACTIVE_STAGES = new Set<BenchRunStage>([
  'planned',
  'running',
  'runtime_ready',
  'inputs_resolved',
  'candidate_running',
  'candidate_completed',
  'judging',
]);

export function parseSingleRunManifest(value: unknown): StoredSingleRunManifest | undefined {
  if (!isRecord(value) || !hasOnlyKeys(value, ['version', 'savedAt', 'run'])) return undefined;
  if (
    value.version !== SINGLE_RUN_MANIFEST_VERSION ||
    !isSingleRunManifestTimestamp(value.savedAt) ||
    !isRecord(value.run)
  ) {
    return undefined;
  }
  const run = parseRun(value.run);
  if (!run || Date.parse(value.savedAt) < latestRunTimestamp(run)) return undefined;
  return { version: SINGLE_RUN_MANIFEST_VERSION, savedAt: value.savedAt, run };
}

export function isSingleRunManifestTimestamp(value: unknown): value is string {
  return (
    boundedString(value, 64, true) &&
    /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?(?:Z|[+-]\d{2}:\d{2})$/u.test(value) &&
    !Number.isNaN(Date.parse(value))
  );
}

function parseRun(value: Record<string, unknown>): PersistedSingleRun | undefined {
  if (
    !hasOnlyKeys(value, [
      'mode',
      'status',
      'stage',
      'jobId',
      'runId',
      'startedAt',
      'completedAt',
      'trackingStoppedAt',
      'sortie',
      'result',
      'error',
    ]) ||
    value.mode !== 'live' ||
    !isPersistedStatus(value.status) ||
    !RUN_STAGES.has(value.stage as BenchRunStage) ||
    !isSingleRunManifestTimestamp(value.startedAt) ||
    (value.completedAt !== undefined && !isSingleRunManifestTimestamp(value.completedAt)) ||
    (value.trackingStoppedAt !== undefined && !isSingleRunManifestTimestamp(value.trackingStoppedAt)) ||
    (value.jobId !== undefined && !boundedString(value.jobId, 128, true)) ||
    (value.runId !== undefined && !boundedString(value.runId, 128, true)) ||
    (value.error !== undefined && !boundedString(value.error, 8_192, true))
  ) {
    return undefined;
  }

  const sortie = parseSortie(value.sortie);
  const result = value.result === undefined ? undefined : parseResult(value.result);
  if (!sortie || (value.result !== undefined && !result)) return undefined;

  const run: PersistedSingleRun = {
    mode: 'live',
    status: value.status,
    stage: value.stage as BenchRunStage,
    jobId: value.jobId as string | undefined,
    runId: value.runId as string | undefined,
    startedAt: value.startedAt,
    completedAt: value.completedAt as string | undefined,
    trackingStoppedAt: value.trackingStoppedAt as string | undefined,
    sortie,
    result,
    error: value.error as string | undefined,
  };
  return runIsConsistent(run) ? run : undefined;
}

function runIsConsistent(run: PersistedSingleRun): boolean {
  const startedAt = Date.parse(run.startedAt);
  if (run.completedAt && Date.parse(run.completedAt) < startedAt) return false;
  if (run.trackingStoppedAt && Date.parse(run.trackingStoppedAt) < startedAt) return false;
  if (run.runId && !run.jobId) return false;
  if (run.result && (!run.runId || run.result.run_id !== run.runId)) return false;
  if (run.result?.task_id !== undefined && run.result.task_id !== run.sortie.task.id) return false;

  switch (run.status) {
    case 'active':
      return activeRecordIsConsistent(run) && !run.trackingStoppedAt;
    case 'tracking_stopped':
      return activeRecordIsConsistent(run) && Boolean(run.trackingStoppedAt);
    case 'completed':
      return Boolean(
        run.stage === 'completed' &&
          run.jobId &&
          run.runId &&
          run.completedAt &&
          run.result?.status === 'completed' &&
          !run.trackingStoppedAt &&
          !run.error
      );
    case 'failed':
      return Boolean(
        run.stage === 'failed' &&
          run.completedAt &&
          !run.trackingStoppedAt &&
          (run.error || run.result) &&
          (!run.result || run.result.status === 'failed')
      );
  }
}

function activeRecordIsConsistent(run: PersistedSingleRun): boolean {
  if (run.result || run.error || run.stage === 'idle' || run.stage === 'failed') return false;
  if (run.stage === 'completed') return Boolean(run.jobId && run.runId && run.completedAt);
  if (!ACTIVE_STAGES.has(run.stage)) return false;
  if (run.stage !== 'planned' && !run.jobId) return false;
  if (run.runId || run.completedAt) return Boolean(run.jobId && run.runId && run.completedAt);
  return true;
}

function parseSortie(value: unknown): RunSortieSnapshot | undefined {
  if (!isRecord(value) || !hasOnlyKeys(value, ['task', 'rosterEntry', 'input'])) return undefined;
  const task = parseTask(value.task);
  const rosterEntry = parseRosterEntry(value.rosterEntry);
  const input = parseRunInput(value.input);
  if (!task || !rosterEntry || !input) return undefined;
  if (
    !input.locked &&
    (input.task !== task.id ||
      input.candidate !== rosterEntry.candidate.trim() ||
      input.model !== (rosterEntry.model.trim() || undefined))
  ) {
    return undefined;
  }
  return createRunSortieSnapshot(task, rosterEntry, input);
}

function parseTask(value: unknown): BenchTask | undefined {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      'id',
      'path',
      'name',
      'category',
      'execution_class',
      'availability',
      'availability_reason',
      'admission',
      'admission_reason',
      'provenance_ref',
      'description',
      'tags',
    ]) ||
    !boundedString(value.id, 256, true) ||
    !boundedString(value.path, 2_048, true) ||
    !boundedString(value.name, 512, true) ||
    !boundedString(value.category, 256, true) ||
    !EXECUTION_CLASSES.has(value.execution_class as string) ||
    !AVAILABILITY.has(value.availability as string) ||
    !boundedString(value.availability_reason, 2_048, true) ||
    !ADMISSION.has(value.admission as string) ||
    !boundedString(value.admission_reason, 2_048, true) ||
    !boundedString(value.provenance_ref, 2_048, true) ||
    (value.description !== undefined && !boundedString(value.description, 8_192)) ||
    (value.tags !== undefined &&
      (!Array.isArray(value.tags) ||
        value.tags.length > MAX_TAGS ||
        value.tags.some((tag) => !boundedString(tag, 256, true))))
  ) {
    return undefined;
  }
  return {
    id: value.id,
    path: value.path,
    name: value.name,
    category: value.category,
    execution_class: value.execution_class as BenchTask['execution_class'],
    availability: value.availability as BenchTask['availability'],
    availability_reason: value.availability_reason,
    admission: value.admission as BenchTask['admission'],
    admission_reason: value.admission_reason,
    provenance_ref: value.provenance_ref,
    description: value.description as string | undefined,
    tags: value.tags as string[] | undefined,
  };
}

function parseRosterEntry(value: unknown): HangarRosterEntry | undefined {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, ['id', 'airframeId', 'pilotId', 'candidate', 'model', 'effort', 'callsign']) ||
    !boundedString(value.id, 128, true) ||
    !AIRFRAMES.has(value.airframeId as never) ||
    !PILOTS.has(value.pilotId as never) ||
    !boundedString(value.candidate, 1_024) ||
    !boundedString(value.model, 256) ||
    !EFFORTS.has(value.effort as string) ||
    !boundedString(value.callsign, 128, true)
  ) {
    return undefined;
  }
  return {
    id: value.id,
    airframeId: value.airframeId as HangarRosterEntry['airframeId'],
    pilotId: value.pilotId as HangarRosterEntry['pilotId'],
    candidate: value.candidate,
    model: value.model,
    effort: value.effort as HangarRosterEntry['effort'],
    callsign: value.callsign,
  };
}

function parseRunInput(value: unknown): StartBenchRunInput | undefined {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, ['task', 'candidate', 'model', 'locked']) ||
    !boundedString(value.task, 2_048, true) ||
    !boundedString(value.candidate, 1_024, true) ||
    (value.model !== undefined && !boundedString(value.model, 256, true)) ||
    typeof value.locked !== 'boolean' ||
    (value.locked && value.model !== undefined)
  ) {
    return undefined;
  }
  return {
    task: value.task,
    candidate: value.candidate,
    model: value.model as string | undefined,
    locked: value.locked,
  };
}

function parseResult(value: unknown): BenchRunResult | undefined {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      'status',
      'governance_status',
      'run_id',
      'task_reference',
      'task_id',
      'score',
      'result_path',
      'primary_metric',
      'runtime_provider',
      'model',
      'result_digest',
      'task_lock_digest',
      'candidate_lock_digest',
      'candidate_identity',
      'judge_identity',
      'model_usage',
    ]) ||
    !RUN_STAGES.has(value.status as BenchRunStage) ||
    !boundedString(value.run_id, 128, true) ||
    (value.governance_status !== undefined && value.governance_status !== 'local_unofficial') ||
    !optionalBoundedString(value.task_reference, 2_048) ||
    !optionalBoundedString(value.task_id, 256) ||
    !optionalBoundedString(value.score, 256) ||
    !optionalBoundedString(value.result_path, 2_048) ||
    !optionalBoundedString(value.primary_metric, 256) ||
    !optionalBoundedString(value.runtime_provider, 256) ||
    (value.model !== undefined && value.model !== null && !boundedString(value.model, 256)) ||
    !optionalBoundedString(value.result_digest, 512) ||
    !optionalBoundedString(value.task_lock_digest, 512) ||
    !optionalBoundedString(value.candidate_lock_digest, 512) ||
    !optionalBoundedString(value.candidate_identity, 1_024) ||
    !optionalBoundedString(value.judge_identity, 1_024)
  ) {
    return undefined;
  }
  const modelUsage = value.model_usage === null ? null : parseModelUsage(value.model_usage);
  if (value.model_usage !== undefined && value.model_usage !== null && !modelUsage) return undefined;
  return {
    status: value.status as BenchRunStage,
    governance_status: value.governance_status as BenchRunResult['governance_status'],
    run_id: value.run_id,
    task_reference: value.task_reference as string | undefined,
    task_id: value.task_id as string | undefined,
    score: value.score as string | undefined,
    result_path: value.result_path as string | undefined,
    primary_metric: value.primary_metric as string | undefined,
    runtime_provider: value.runtime_provider as string | undefined,
    model: value.model as string | null | undefined,
    result_digest: value.result_digest as string | undefined,
    task_lock_digest: value.task_lock_digest as string | undefined,
    candidate_lock_digest: value.candidate_lock_digest as string | undefined,
    candidate_identity: value.candidate_identity as string | undefined,
    judge_identity: value.judge_identity as string | undefined,
    model_usage: modelUsage,
  };
}

function parseModelUsage(value: unknown): NonNullable<BenchRunResult['model_usage']> | undefined {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      'prompt_tokens',
      'completion_tokens',
      'total_tokens',
      'cache_read_tokens',
      'cache_write_tokens',
      'tool_calls_count',
    ]) ||
    !nonNegativeInteger(value.prompt_tokens) ||
    !nonNegativeInteger(value.completion_tokens) ||
    !nonNegativeInteger(value.total_tokens) ||
    (value.cache_read_tokens !== undefined &&
      value.cache_read_tokens !== null &&
      !nonNegativeInteger(value.cache_read_tokens)) ||
    (value.cache_write_tokens !== undefined &&
      value.cache_write_tokens !== null &&
      !nonNegativeInteger(value.cache_write_tokens)) ||
    (value.tool_calls_count !== undefined &&
      value.tool_calls_count !== null &&
      !nonNegativeInteger(value.tool_calls_count))
  ) {
    return undefined;
  }
  return {
    prompt_tokens: value.prompt_tokens,
    completion_tokens: value.completion_tokens,
    total_tokens: value.total_tokens,
    cache_read_tokens: value.cache_read_tokens as number | null | undefined,
    cache_write_tokens: value.cache_write_tokens as number | null | undefined,
    tool_calls_count: value.tool_calls_count as number | null | undefined,
  };
}

function latestRunTimestamp(run: PersistedSingleRun): number {
  return Math.max(
    Date.parse(run.startedAt),
    run.completedAt ? Date.parse(run.completedAt) : 0,
    run.trackingStoppedAt ? Date.parse(run.trackingStoppedAt) : 0
  );
}

function isPersistedStatus(value: unknown): value is PersistedSingleRunStatus {
  return value === 'active' || value === 'completed' || value === 'failed' || value === 'tracking_stopped';
}

function optionalBoundedString(value: unknown, maxLength: number): boolean {
  return value === undefined || boundedString(value, maxLength);
}

function boundedString(value: unknown, maxLength: number, required = false): value is string {
  return (
    typeof value === 'string' &&
    value.length <= maxLength &&
    !/[\0\r\n]/u.test(value) &&
    (!required || value.trim().length > 0)
  );
}

function nonNegativeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function hasOnlyKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const allowed = new Set(keys);
  return Object.keys(value).every((key) => allowed.has(key));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
