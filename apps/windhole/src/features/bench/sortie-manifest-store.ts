import { createRunSortieSnapshot, type RunSortieSnapshot } from '../../state/lab-state';
import type { BenchTask, StartBenchRunInput } from '../../types/bench';
import { HANGAR_AIRFRAME_OPTIONS, HANGAR_PILOT_OPTIONS, type HangarRosterEntry } from '../hangar/hangar-configuration';

const STORAGE_KEY = 'a3s-agent-evaluation.sorties.v1';
const STORAGE_VERSION = 1;
const MAX_MANIFESTS = 100;
const MAX_SERIALIZED_LENGTH = 4 * 1_024 * 1_024;
const MAX_TAGS = 64;
const AIRFRAMES = new Set(HANGAR_AIRFRAME_OPTIONS.map((option) => option.id));
const PILOTS = new Set(HANGAR_PILOT_OPTIONS.map((option) => option.id));
const EFFORTS = new Set(['none', 'minimal', 'low', 'medium', 'high', 'xhigh']);
const AVAILABILITY = new Set(['ready', 'blocked']);
const ADMISSION = new Set(['admitted', 'quarantined']);
const EXECUTION_CLASSES = new Set(['conformance', 'long_horizon']);

interface StorageAdapter {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

interface StoredManifest {
  runId: string;
  savedAt: string;
  sortie: RunSortieSnapshot;
}

interface StoredArchive {
  version: typeof STORAGE_VERSION;
  manifests: StoredManifest[];
}

export function saveSortieManifest(
  runId: string,
  sortie: Readonly<RunSortieSnapshot>,
  storage: StorageAdapter = window.localStorage
): void {
  try {
    const normalizedRunId = runId.trim();
    const manifest = parseManifest({
      runId: normalizedRunId,
      savedAt: new Date().toISOString(),
      sortie,
    });
    if (!manifest) return;

    const archive = readArchive(storage);
    if (archive.manifests.some((entry) => entry.runId === normalizedRunId)) {
      // A real Run ID owns one immutable attribution. Repeated or conflicting
      // callbacks must never rebind it to another map, aircraft, or input.
      return;
    }

    const manifests = [manifest, ...archive.manifests].slice(0, MAX_MANIFESTS);
    const serialized = JSON.stringify({ version: STORAGE_VERSION, manifests } satisfies StoredArchive);
    if (serialized.length > MAX_SERIALIZED_LENGTH) return;
    storage.setItem(STORAGE_KEY, serialized);
  } catch {
    // The Bench result remains authoritative even when optional browser metadata cannot be persisted.
  }
}

export function loadSortieManifest(
  runId: string,
  storage: StorageAdapter = window.localStorage
): RunSortieSnapshot | undefined {
  const normalizedRunId = runId.trim();
  if (!canonicalString(normalizedRunId, 128)) return undefined;
  return readArchive(storage).manifests.find((entry) => entry.runId === normalizedRunId)?.sortie;
}

function readArchive(storage: StorageAdapter): StoredArchive {
  try {
    const serialized = storage.getItem(STORAGE_KEY);
    if (!serialized || serialized.length > MAX_SERIALIZED_LENGTH) return emptyArchive();
    const value = JSON.parse(serialized) as unknown;
    if (
      !isRecord(value) ||
      !hasOnlyKeys(value, ['version', 'manifests']) ||
      value.version !== STORAGE_VERSION ||
      !Array.isArray(value.manifests) ||
      value.manifests.length > MAX_MANIFESTS
    ) {
      return emptyArchive();
    }

    const manifests: StoredManifest[] = [];
    const seenRunIds = new Set<string>();
    const ambiguousRunIds = new Set<string>();
    for (const candidate of value.manifests) {
      const manifest = parseManifest(candidate);
      if (!manifest) continue;
      if (seenRunIds.has(manifest.runId)) {
        ambiguousRunIds.add(manifest.runId);
        continue;
      }
      seenRunIds.add(manifest.runId);
      manifests.push(manifest);
    }

    return {
      version: STORAGE_VERSION,
      manifests: manifests.filter((manifest) => !ambiguousRunIds.has(manifest.runId)),
    };
  } catch {
    return emptyArchive();
  }
}

function parseManifest(value: unknown): StoredManifest | undefined {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, ['runId', 'savedAt', 'sortie']) ||
    !canonicalString(value.runId, 128) ||
    !isoTimestamp(value.savedAt)
  ) {
    return undefined;
  }
  const sortie = parseSortie(value.sortie);
  return sortie ? { runId: value.runId, savedAt: value.savedAt, sortie } : undefined;
}

function parseSortie(value: unknown): RunSortieSnapshot | undefined {
  if (!isRecord(value) || !hasOnlyKeys(value, ['task', 'rosterEntry', 'input'])) return undefined;
  const task = parseTask(value.task);
  const rosterEntry = parseRosterEntry(value.rosterEntry);
  const input = parseRunInput(value.input);
  if (!task || !rosterEntry || !input || !sortieIsConsistent(task, rosterEntry, input)) return undefined;
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
    !canonicalString(value.task, 2_048) ||
    !canonicalString(value.candidate, 1_024) ||
    (value.model !== undefined && !canonicalString(value.model, 256)) ||
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

function sortieIsConsistent(
  task: Readonly<BenchTask>,
  rosterEntry: Readonly<HangarRosterEntry>,
  input: Readonly<StartBenchRunInput>
): boolean {
  if (input.locked) return input.model === undefined;
  return (
    input.task === task.id &&
    input.candidate === rosterEntry.candidate.trim() &&
    input.model === (rosterEntry.model.trim() || undefined)
  );
}

function canonicalString(value: unknown, maxLength: number): value is string {
  return boundedString(value, maxLength, true) && value === value.trim();
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
  if (!boundedString(value, 64, true) || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u.test(value)) {
    return false;
  }
  const date = new Date(value);
  return !Number.isNaN(date.valueOf()) && date.toISOString() === value;
}

function hasOnlyKeys(value: Record<string, unknown>, allowed: readonly string[]): boolean {
  const allowedKeys = new Set(allowed);
  return Object.keys(value).every((key) => allowedKeys.has(key));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function emptyArchive(): StoredArchive {
  return { version: STORAGE_VERSION, manifests: [] };
}
