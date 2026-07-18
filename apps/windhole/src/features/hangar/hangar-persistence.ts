import { subscribe } from 'valtio';
import type { ReasoningEffort } from '../../components/scene/weapon-loadout';
import { labState } from '../../state/lab-state';
import {
  HANGAR_AIRFRAME_OPTIONS,
  HANGAR_PILOT_OPTIONS,
  type HangarRosterEntry,
  MAX_HANGAR_ROSTER_SIZE,
} from './hangar-configuration';
import { draftFromRosterEntry } from './hangar-roster-state';

const STORAGE_KEY = 'a3s-agent-evaluation.hangar.v1';
const STORAGE_VERSION = 1;
const EFFORTS: ReadonlySet<ReasoningEffort> = new Set(['none', 'minimal', 'low', 'medium', 'high', 'xhigh']);
const AIRFRAMES = new Set(HANGAR_AIRFRAME_OPTIONS.map((option) => option.id));
const PILOTS = new Set(HANGAR_PILOT_OPTIONS.map((option) => option.id));
const LEGACY_BUILT_IN_A3S_DEFAULT = Object.freeze({
  id: 'a3s-j-35',
  airframeId: 'j-35',
  pilotId: 'a3s',
  candidate: 'a3s-code',
  model: 'zai/glm-5.2',
});
const CURRENT_BUILT_IN_A3S_DEFAULT = Object.freeze({
  id: 'a3s-j-50',
  airframeId: 'j-50',
  model: 'anthropic/glm-5.2',
});

interface StorageAdapter {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

interface PersistedHangarState {
  version: typeof STORAGE_VERSION;
  activeEntryId: string;
  roster: HangarRosterEntry[];
}

export function restoreHangarState(storage: StorageAdapter = window.localStorage): boolean {
  try {
    const serialized = storage.getItem(STORAGE_KEY);
    if (!serialized) return false;
    const parsed = parseHangarState(JSON.parse(serialized) as unknown);
    if (!parsed) return false;
    const persisted = migrateLegacyBuiltInDefault(parsed);

    const activeEntry = persisted.roster.find((entry) => entry.id === persisted.activeEntryId) ?? persisted.roster[0];
    labState.hangar = {
      roster: persisted.roster.map((entry) => ({ ...entry })),
      activeEntryId: activeEntry.id,
      draft: draftFromRosterEntry(activeEntry),
    };
    if (persisted !== parsed) {
      try {
        storage.setItem(STORAGE_KEY, JSON.stringify(persisted));
      } catch {
        // The in-memory migration remains valid when storage is read-only or full.
      }
    }
    return true;
  } catch {
    return false;
  }
}

export function startHangarPersistence(storage: StorageAdapter = window.localStorage): () => void {
  return subscribe(labState.hangar, () => {
    try {
      const persisted: PersistedHangarState = {
        version: STORAGE_VERSION,
        activeEntryId: labState.hangar.activeEntryId,
        roster: labState.hangar.roster.map((entry) => ({ ...entry })),
      };
      storage.setItem(STORAGE_KEY, JSON.stringify(persisted));
    } catch {
      // Persistence is optional; the live in-memory roster remains authoritative for this session.
    }
  });
}

function parseHangarState(value: unknown): PersistedHangarState | undefined {
  if (!isRecord(value) || value.version !== STORAGE_VERSION || typeof value.activeEntryId !== 'string')
    return undefined;
  if (!Array.isArray(value.roster) || value.roster.length < 1 || value.roster.length > MAX_HANGAR_ROSTER_SIZE) {
    return undefined;
  }
  const roster: HangarRosterEntry[] = [];
  const ids = new Set<string>();
  for (const candidate of value.roster) {
    const entry = parseRosterEntry(candidate);
    if (!entry || ids.has(entry.id)) return undefined;
    ids.add(entry.id);
    roster.push(entry);
  }
  return { version: STORAGE_VERSION, activeEntryId: value.activeEntryId, roster };
}

function migrateLegacyBuiltInDefault(persisted: PersistedHangarState): PersistedHangarState {
  const legacyIndex = persisted.roster.findIndex(isLegacyBuiltInA3sDefault);
  if (legacyIndex < 0) return persisted;

  const legacyEntry = persisted.roster[legacyIndex];
  const migratedId = nextAvailableA3sDefaultId(persisted.roster, legacyIndex);
  const roster = persisted.roster.map((entry, index) =>
    index === legacyIndex
      ? {
          ...entry,
          id: migratedId,
          airframeId: CURRENT_BUILT_IN_A3S_DEFAULT.airframeId,
          model: CURRENT_BUILT_IN_A3S_DEFAULT.model,
        }
      : entry
  );
  return {
    ...persisted,
    activeEntryId: persisted.activeEntryId === legacyEntry.id ? migratedId : persisted.activeEntryId,
    roster,
  };
}

function isLegacyBuiltInA3sDefault(entry: HangarRosterEntry): boolean {
  return (
    entry.id === LEGACY_BUILT_IN_A3S_DEFAULT.id &&
    entry.airframeId === LEGACY_BUILT_IN_A3S_DEFAULT.airframeId &&
    entry.pilotId === LEGACY_BUILT_IN_A3S_DEFAULT.pilotId &&
    entry.candidate === LEGACY_BUILT_IN_A3S_DEFAULT.candidate &&
    entry.model === LEGACY_BUILT_IN_A3S_DEFAULT.model
  );
}

function nextAvailableA3sDefaultId(roster: readonly HangarRosterEntry[], legacyIndex: number): string {
  const occupiedIds = new Set(
    roster.filter((_, index) => index !== legacyIndex).map((entry) => normalizeRosterId(entry.id))
  );
  const baseId = CURRENT_BUILT_IN_A3S_DEFAULT.id;
  if (!occupiedIds.has(baseId)) return baseId;

  let suffix = 2;
  while (occupiedIds.has(`${baseId}-${suffix}`)) suffix += 1;
  return `${baseId}-${suffix}`;
}

function parseRosterEntry(value: unknown): HangarRosterEntry | undefined {
  if (!isRecord(value)) return undefined;
  if (
    !boundedString(value.id, 128, true) ||
    !boundedString(value.candidate, 1_024) ||
    !boundedString(value.model, 256) ||
    !boundedString(value.callsign, 128, true) ||
    !AIRFRAMES.has(value.airframeId as never) ||
    !PILOTS.has(value.pilotId as never) ||
    !EFFORTS.has(value.effort as ReasoningEffort)
  ) {
    return undefined;
  }
  return {
    id: value.id,
    airframeId: value.airframeId as HangarRosterEntry['airframeId'],
    pilotId: value.pilotId as HangarRosterEntry['pilotId'],
    candidate: value.candidate,
    model: value.model,
    effort: value.effort as ReasoningEffort,
    callsign: value.callsign,
  };
}

function boundedString(value: unknown, maxLength: number, required = false): value is string {
  return (
    typeof value === 'string' &&
    value.length <= maxLength &&
    !/[\0\r\n]/u.test(value) &&
    (!required || value.trim().length > 0)
  );
}

function normalizeRosterId(id: string): string {
  return id
    .normalize('NFKC')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
