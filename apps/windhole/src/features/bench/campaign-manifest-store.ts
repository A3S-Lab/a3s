import { subscribe } from 'valtio';
import {
  createRunCampaignSnapshot,
  type LabCampaignMemberRun,
  type LabCampaignState,
  labState,
  type RunCampaignSnapshot,
} from '../../state/lab-state';
import type {
  BenchCampaignMemberStatus,
  BenchCampaignStatus,
  BenchRunResult,
  BenchRunStage,
  BenchTask,
  StartBenchRunInput,
} from '../../types/bench';
import {
  HANGAR_AIRFRAME_OPTIONS,
  HANGAR_PILOT_OPTIONS,
  type HangarRosterEntry,
  MAX_HANGAR_ROSTER_SIZE,
} from '../hangar/hangar-configuration';
import { reconcileRestoredTaskCatalog } from './restored-task-catalog';

export const CAMPAIGN_MANIFEST_STORAGE_KEY = 'a3s-agent-evaluation.campaign.v1';

const STORAGE_VERSION = 1;
const MAX_SERIALIZED_LENGTH = 256 * 1_024;
const MAX_TAGS = 64;
const AIRFRAMES = new Set(HANGAR_AIRFRAME_OPTIONS.map((option) => option.id));
const PILOTS = new Set(HANGAR_PILOT_OPTIONS.map((option) => option.id));
const EFFORTS = new Set(['none', 'minimal', 'low', 'medium', 'high', 'xhigh']);
const AVAILABILITY = new Set(['ready', 'blocked']);
const ADMISSION = new Set(['admitted', 'quarantined']);
const EXECUTION_CLASSES = new Set(['conformance', 'long_horizon']);
const CAMPAIGN_STATUSES = new Set<BenchCampaignStatus>([
  'running',
  'completed',
  'completed_with_failures',
  'failed',
  'tracking_stopped',
]);
const MEMBER_STATUSES = new Set<BenchCampaignMemberStatus>([
  'queued',
  'starting',
  'running',
  'completed',
  'failed',
  'tracking_stopped',
]);
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

interface StorageAdapter {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

interface PersistedCampaignMember {
  rosterEntryId: string;
  status: BenchCampaignMemberStatus;
  stage?: BenchRunStage;
  jobId?: string;
  runId?: string;
  startedAt?: string;
  completedAt?: string;
  result?: BenchRunResult;
  error?: string;
}

interface PersistedCampaign {
  generation: number;
  status: Exclude<BenchCampaignStatus, 'idle'>;
  startedAt: string;
  completedAt?: string;
  snapshot: RunCampaignSnapshot;
  members: PersistedCampaignMember[];
  error?: string;
}

interface StoredCampaignManifest {
  version: typeof STORAGE_VERSION;
  savedAt: string;
  campaign: PersistedCampaign;
}

export function restoreCampaignManifest(
  storage: StorageAdapter = window.localStorage,
  now: () => string = () => new Date().toISOString()
): boolean {
  try {
    const serialized = storage.getItem(CAMPAIGN_MANIFEST_STORAGE_KEY);
    if (!serialized || serialized.length > MAX_SERIALIZED_LENGTH) return false;
    const manifest = parseManifest(JSON.parse(serialized) as unknown);
    if (!manifest) return false;

    const restoredAt = now();
    if (manifest.campaign.status === 'running' && !isoTimestamp(restoredAt)) return false;
    labState.campaign = restoreCampaignState(manifest.campaign, restoredAt);
    labState.runConfig.locked = false;
    labState.runConfig.deploymentScope = 'campaign';
    reconcileRestoredTaskCatalog();
    return true;
  } catch {
    return false;
  }
}

export function startCampaignManifestPersistence(
  storage: StorageAdapter = window.localStorage,
  now: () => string = () => new Date().toISOString()
): () => void {
  let previousCampaign = persistCurrentCampaign(storage, now);
  return subscribe(labState, () => {
    previousCampaign = persistCurrentCampaign(storage, now, previousCampaign);
  });
}

function persistCurrentCampaign(
  storage: StorageAdapter,
  now: () => string,
  previousCampaign?: string
): string | undefined {
  try {
    const campaign = serializeCampaign(labState.campaign);
    if (!campaign) return previousCampaign;
    const serializedCampaign = JSON.stringify(campaign);
    if (serializedCampaign === previousCampaign) return previousCampaign;
    const manifest = {
      version: STORAGE_VERSION,
      savedAt: now(),
      campaign,
    } satisfies StoredCampaignManifest;
    if (!parseManifest(manifest)) return previousCampaign;
    const serialized = JSON.stringify(manifest);
    if (serialized.length > MAX_SERIALIZED_LENGTH) return previousCampaign;
    storage.setItem(CAMPAIGN_MANIFEST_STORAGE_KEY, serialized);
    return serializedCampaign;
  } catch {
    // The live campaign and exact Run ID result API remain authoritative when browser persistence is unavailable.
    return previousCampaign;
  }
}

function serializeCampaign(campaign: LabCampaignState): PersistedCampaign | undefined {
  if (
    campaign.status === 'idle' ||
    !campaign.snapshot ||
    !campaign.startedAt ||
    campaign.members.length < 1 ||
    campaign.members.length > MAX_HANGAR_ROSTER_SIZE ||
    campaign.members.length !== campaign.snapshot.roster.length
  ) {
    return undefined;
  }

  return {
    generation: campaign.generation,
    status: campaign.status,
    startedAt: campaign.startedAt,
    completedAt: campaign.completedAt,
    snapshot: cloneCampaignSnapshot(campaign.snapshot),
    members: campaign.members.map((member) => ({
      rosterEntryId: member.rosterEntryId,
      status: member.status,
      stage: member.stage,
      jobId: member.jobId,
      runId: member.runId,
      startedAt: member.startedAt,
      completedAt: member.completedAt,
      result: member.result ? cloneResult(member.result) : undefined,
      error: member.error,
    })),
    error: campaign.error,
  };
}

function restoreCampaignState(campaign: PersistedCampaign, restoredAt: string): LabCampaignState {
  const trackingWasInterrupted = campaign.status === 'running';
  const members: LabCampaignMemberRun[] = campaign.members.map((member, index) => {
    const wasActive = member.status === 'queued' || member.status === 'starting' || member.status === 'running';
    return {
      ...member,
      sortie: campaign.snapshot.roster[index],
      status: trackingWasInterrupted && wasActive ? 'tracking_stopped' : member.status,
      completedAt: trackingWasInterrupted && wasActive ? (member.completedAt ?? restoredAt) : member.completedAt,
      result: member.result ? cloneResult(member.result) : undefined,
    };
  });

  return {
    generation: campaign.generation,
    status: trackingWasInterrupted ? 'tracking_stopped' : campaign.status,
    startedAt: campaign.startedAt,
    completedAt: trackingWasInterrupted ? restoredAt : campaign.completedAt,
    snapshot: campaign.snapshot,
    members,
    error: campaign.error,
  };
}

function parseManifest(value: unknown): StoredCampaignManifest | undefined {
  if (!isRecord(value) || !hasOnlyKeys(value, ['version', 'savedAt', 'campaign'])) return undefined;
  if (value.version !== STORAGE_VERSION || !isoTimestamp(value.savedAt) || !isRecord(value.campaign)) return undefined;
  const campaign = parseCampaign(value.campaign);
  return campaign ? { version: STORAGE_VERSION, savedAt: value.savedAt, campaign } : undefined;
}

function parseCampaign(value: Record<string, unknown>): PersistedCampaign | undefined {
  if (
    !hasOnlyKeys(value, ['generation', 'status', 'startedAt', 'completedAt', 'snapshot', 'members', 'error']) ||
    !positiveInteger(value.generation) ||
    !CAMPAIGN_STATUSES.has(value.status as BenchCampaignStatus) ||
    !isoTimestamp(value.startedAt) ||
    (value.completedAt !== undefined && !isoTimestamp(value.completedAt)) ||
    (value.error !== undefined && !boundedString(value.error, 8_192, true)) ||
    !Array.isArray(value.members) ||
    value.members.length < 1 ||
    value.members.length > MAX_HANGAR_ROSTER_SIZE
  ) {
    return undefined;
  }

  const snapshot = parseCampaignSnapshot(value.snapshot);
  if (!snapshot || snapshot.roster.length !== value.members.length) return undefined;

  const members: PersistedCampaignMember[] = [];
  const jobIds = new Set<string>();
  const runIds = new Set<string>();
  for (const [index, candidate] of value.members.entries()) {
    const member = parseMember(candidate);
    if (!member || member.rosterEntryId !== snapshot.roster[index].rosterEntry.id) return undefined;
    if ((member.jobId && jobIds.has(member.jobId)) || (member.runId && runIds.has(member.runId))) return undefined;
    if (member.jobId) jobIds.add(member.jobId);
    if (member.runId) runIds.add(member.runId);
    if (member.result && member.result.task_id !== undefined && member.result.task_id !== snapshot.task.id) {
      return undefined;
    }
    members.push(member);
  }

  const campaign: PersistedCampaign = {
    generation: value.generation,
    status: value.status as PersistedCampaign['status'],
    startedAt: value.startedAt,
    completedAt: value.completedAt as string | undefined,
    snapshot,
    members,
    error: value.error as string | undefined,
  };
  return campaignIsConsistent(campaign) ? campaign : undefined;
}

function parseCampaignSnapshot(value: unknown): RunCampaignSnapshot | undefined {
  if (!isRecord(value) || !hasOnlyKeys(value, ['task', 'roster']) || !Array.isArray(value.roster)) return undefined;
  if (value.roster.length < 1 || value.roster.length > MAX_HANGAR_ROSTER_SIZE) return undefined;
  const task = parseTask(value.task);
  if (!task) return undefined;

  const roster: HangarRosterEntry[] = [];
  const ids = new Set<string>();
  for (const candidate of value.roster) {
    if (!isRecord(candidate) || !hasOnlyKeys(candidate, ['rosterEntry', 'input'])) return undefined;
    const rosterEntry = parseRosterEntry(candidate.rosterEntry);
    const input = parseRunInput(candidate.input);
    if (!rosterEntry || !input || ids.has(rosterEntry.id)) return undefined;
    if (
      input.task !== task.id ||
      input.candidate !== rosterEntry.candidate.trim() ||
      input.model !== (rosterEntry.model.trim() || undefined)
    ) {
      return undefined;
    }
    ids.add(rosterEntry.id);
    roster.push(rosterEntry);
  }
  return createRunCampaignSnapshot(task, roster);
}

function parseMember(value: unknown): PersistedCampaignMember | undefined {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      'rosterEntryId',
      'status',
      'stage',
      'jobId',
      'runId',
      'startedAt',
      'completedAt',
      'result',
      'error',
    ]) ||
    !boundedString(value.rosterEntryId, 128, true) ||
    !MEMBER_STATUSES.has(value.status as BenchCampaignMemberStatus) ||
    (value.stage !== undefined && !RUN_STAGES.has(value.stage as BenchRunStage)) ||
    (value.jobId !== undefined && !boundedString(value.jobId, 128, true)) ||
    (value.runId !== undefined && !boundedString(value.runId, 128, true)) ||
    (value.startedAt !== undefined && !isoTimestamp(value.startedAt)) ||
    (value.completedAt !== undefined && !isoTimestamp(value.completedAt)) ||
    (value.error !== undefined && !boundedString(value.error, 8_192, true))
  ) {
    return undefined;
  }
  const result = value.result === undefined ? undefined : parseResult(value.result);
  if (value.result !== undefined && !result) return undefined;
  if (result && (value.runId === undefined || result.run_id !== value.runId)) return undefined;

  const member: PersistedCampaignMember = {
    rosterEntryId: value.rosterEntryId,
    status: value.status as BenchCampaignMemberStatus,
    stage: value.stage as BenchRunStage | undefined,
    jobId: value.jobId as string | undefined,
    runId: value.runId as string | undefined,
    startedAt: value.startedAt as string | undefined,
    completedAt: value.completedAt as string | undefined,
    result,
    error: value.error as string | undefined,
  };
  return memberIsConsistent(member) ? member : undefined;
}

function parseTask(value: unknown): BenchTask | undefined {
  if (!isRecord(value)) return undefined;
  if (
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
    value.locked !== false
  ) {
    return undefined;
  }
  return {
    task: value.task,
    candidate: value.candidate,
    model: value.model as string | undefined,
    locked: false,
  };
}

function parseResult(value: unknown): BenchRunResult | undefined {
  if (!isRecord(value)) return undefined;
  if (
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

function memberIsConsistent(member: PersistedCampaignMember): boolean {
  if (member.startedAt && member.completedAt && Date.parse(member.completedAt) < Date.parse(member.startedAt)) {
    return false;
  }
  switch (member.status) {
    case 'queued':
      return (
        !member.stage &&
        !member.jobId &&
        !member.runId &&
        !member.startedAt &&
        !member.completedAt &&
        !member.result &&
        !member.error
      );
    case 'starting':
      return (
        (Boolean(member.startedAt) &&
          !member.stage &&
          !member.jobId &&
          !member.runId &&
          !member.completedAt &&
          !member.result &&
          !member.error) ||
        isAwaitingExactResult(member)
      );
    case 'running':
      return (
        (Boolean(member.startedAt && member.jobId && member.stage) &&
          member.stage !== 'idle' &&
          member.stage !== 'completed' &&
          member.stage !== 'failed' &&
          !member.runId &&
          !member.completedAt &&
          !member.result &&
          !member.error) ||
        isAwaitingExactResult(member)
      );
    case 'completed':
      return Boolean(
        member.startedAt &&
          member.completedAt &&
          member.jobId &&
          member.runId &&
          member.stage === 'completed' &&
          member.result?.status === 'completed' &&
          !member.error
      );
    case 'failed':
      return (
        Boolean(member.completedAt && member.error) &&
        (!member.runId || Boolean(member.jobId)) &&
        (!member.result || Boolean(member.jobId && member.runId && member.stage === 'failed')) &&
        (!member.result || member.result.status === 'failed')
      );
    case 'tracking_stopped':
      return Boolean(member.completedAt) && !member.result && !member.error;
  }
}

function isAwaitingExactResult(member: PersistedCampaignMember): boolean {
  return Boolean(
    member.startedAt &&
      member.completedAt &&
      member.jobId &&
      member.runId &&
      member.stage === 'completed' &&
      !member.result &&
      !member.error
  );
}

function campaignIsConsistent(campaign: PersistedCampaign): boolean {
  if (campaign.completedAt && Date.parse(campaign.completedAt) < Date.parse(campaign.startedAt)) return false;
  const statuses = new Set(campaign.members.map((member) => member.status));
  switch (campaign.status) {
    case 'running':
      return (
        !campaign.completedAt &&
        !campaign.error &&
        [...statuses].some((status) => status === 'queued' || status === 'starting' || status === 'running')
      );
    case 'completed':
      return Boolean(campaign.completedAt) && statuses.size === 1 && statuses.has('completed') && !campaign.error;
    case 'completed_with_failures':
      return (
        Boolean(campaign.completedAt && campaign.error) &&
        statuses.has('completed') &&
        statuses.has('failed') &&
        statuses.size === 2
      );
    case 'failed':
      return (
        Boolean(campaign.completedAt && campaign.error) &&
        [...statuses].every((status) => status === 'completed' || status === 'failed') &&
        statuses.has('failed')
      );
    case 'tracking_stopped':
      return (
        Boolean(campaign.completedAt) &&
        !campaign.error &&
        [...statuses].every(
          (status) => status === 'completed' || status === 'failed' || status === 'tracking_stopped'
        ) &&
        statuses.has('tracking_stopped')
      );
  }
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

function cloneCampaignSnapshot(snapshot: RunCampaignSnapshot): RunCampaignSnapshot {
  const task: BenchTask = {
    id: snapshot.task.id,
    path: snapshot.task.path,
    name: snapshot.task.name,
    category: snapshot.task.category,
    execution_class: snapshot.task.execution_class,
    availability: snapshot.task.availability,
    availability_reason: snapshot.task.availability_reason,
    admission: snapshot.task.admission,
    admission_reason: snapshot.task.admission_reason,
    provenance_ref: snapshot.task.provenance_ref,
    description: snapshot.task.description,
    tags: snapshot.task.tags ? [...snapshot.task.tags] : undefined,
  };
  const roster = snapshot.roster.map((sortie) => ({
    id: sortie.rosterEntry.id,
    airframeId: sortie.rosterEntry.airframeId,
    pilotId: sortie.rosterEntry.pilotId,
    candidate: sortie.rosterEntry.candidate,
    model: sortie.rosterEntry.model,
    effort: sortie.rosterEntry.effort,
    callsign: sortie.rosterEntry.callsign,
  }));
  return createRunCampaignSnapshot(task, roster);
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

function isoTimestamp(value: unknown): value is string {
  return (
    boundedString(value, 64, true) &&
    /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?(?:Z|[+-]\d{2}:\d{2})$/u.test(value) &&
    !Number.isNaN(Date.parse(value))
  );
}

function positiveInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) > 0;
}

function nonNegativeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function hasOnlyKeys(value: Record<string, unknown>, allowed: readonly string[]): boolean {
  const allowedKeys = new Set(allowed);
  return Object.keys(value).every((key) => allowedKeys.has(key));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
