import type { BuiltInAircraftId } from '../../components/scene/aircraft-blueprint';
import type { PilotIdentity } from '../../components/scene/pilot-profile';
import type { ReasoningEffort } from '../../components/scene/weapon-loadout';

export interface HangarDraft {
  airframeId: BuiltInAircraftId;
  pilotId: PilotIdentity;
  candidate: string;
  model: string;
  effort: ReasoningEffort;
  callsign: string;
}

export interface HangarRosterEntry extends HangarDraft {
  id: string;
}

export type HangarAirframeRole = 'air-dominance' | 'carrier-multirole' | 'strike-multirole' | 'experimental';

export interface HangarAirframeOption {
  id: BuiltInAircraftId;
  displayName: string;
  role: HangarAirframeRole;
  description: string;
}

export interface HangarPilotOption {
  id: PilotIdentity;
  displayName: string;
  candidate: string;
  defaultModel: string;
  defaultAirframeId: BuiltInAircraftId;
  defaultEffort: ReasoningEffort;
  defaultCallsign: string;
}

export type CandidateReferenceKind = 'bundled' | 'local' | 'oci' | 'missing' | 'unsupported';

export interface CandidateReferenceStatus {
  deployable: boolean;
  kind: CandidateReferenceKind;
  message: string;
}

export const MAX_HANGAR_ROSTER_SIZE = 5;

export const HANGAR_AIRFRAME_OPTIONS: readonly HangarAirframeOption[] = Object.freeze([
  Object.freeze({
    id: 'j-50',
    displayName: 'J-50',
    role: 'air-dominance',
    description: 'Tailless next-generation air-superiority demonstrator with twin engines.',
  }),
  Object.freeze({
    id: 'j-35',
    displayName: 'J-35',
    role: 'carrier-multirole',
    description: 'Carrier-capable stealth fighter configured for balanced multirole testing.',
  }),
  Object.freeze({
    id: 'f-35',
    displayName: 'F-35 Lightning II',
    role: 'strike-multirole',
    description: 'Single-engine stealth strike fighter with an integrated sensor-focused profile.',
  }),
  Object.freeze({
    id: 'f-22',
    displayName: 'F-22 Raptor',
    role: 'air-dominance',
    description: 'Twin-engine air-dominance fighter with diamond wings and rectangular exhausts.',
  }),
  Object.freeze({
    id: 'prototype',
    displayName: 'Generic Test Prototype',
    role: 'experimental',
    description: 'Neutral configurable airframe for custom Candidates and exploratory trials.',
  }),
]);

export const HANGAR_PILOT_OPTIONS: readonly HangarPilotOption[] = Object.freeze([
  Object.freeze({
    id: 'a3s',
    displayName: 'A3S Flight Lead',
    candidate: 'a3s-code',
    defaultModel: 'anthropic/glm-5.2',
    defaultAirframeId: 'j-50',
    defaultEffort: 'high',
    defaultCallsign: 'A3S-01',
  }),
  Object.freeze({
    id: 'codex',
    displayName: 'Codex Systems Pilot',
    candidate: '',
    defaultModel: '',
    defaultAirframeId: 'f-35',
    defaultEffort: 'high',
    defaultCallsign: 'CODEX-01',
  }),
  Object.freeze({
    id: 'claude',
    displayName: 'Claude Test Pilot',
    candidate: '',
    defaultModel: '',
    defaultAirframeId: 'f-22',
    defaultEffort: 'high',
    defaultCallsign: 'CLAUDE-01',
  }),
  Object.freeze({
    id: 'generic',
    displayName: 'Unassigned Test Pilot',
    candidate: '',
    defaultModel: '',
    defaultAirframeId: 'prototype',
    defaultEffort: 'medium',
    defaultCallsign: 'TEST-01',
  }),
]);

export const DEFAULT_HANGAR_ROSTER: readonly Readonly<HangarRosterEntry>[] = Object.freeze([
  Object.freeze(createHangarRosterEntry(createHangarDraft('a3s'), [])),
  Object.freeze(createHangarRosterEntry(createHangarDraft('codex'), ['a3s-j-50'])),
  Object.freeze(createHangarRosterEntry(createHangarDraft('claude'), ['a3s-j-50', 'codex-f-35'])),
]);

export function createHangarDraft(pilotId: PilotIdentity = 'generic', airframeId?: BuiltInAircraftId): HangarDraft {
  const pilot = HANGAR_PILOT_OPTIONS.find((option) => option.id === pilotId) ?? pilotOption('generic');
  return {
    airframeId: airframeId ?? pilot.defaultAirframeId,
    pilotId: pilot.id,
    candidate: pilot.candidate,
    model: pilot.defaultModel,
    effort: pilot.defaultEffort,
    callsign: pilot.defaultCallsign,
  };
}

export function createHangarRosterEntry(draft: HangarDraft, existingIds: Iterable<string>): HangarRosterEntry {
  const occupiedIds = new Set(Array.from(existingIds, normalizeRosterId));
  if (occupiedIds.size >= MAX_HANGAR_ROSTER_SIZE) {
    throw new RangeError(`Hangar roster cannot exceed ${MAX_HANGAR_ROSTER_SIZE} aircraft`);
  }

  const baseId = normalizeRosterId(`${draft.pilotId}-${draft.airframeId}`);
  let id = baseId;
  let suffix = 2;
  while (occupiedIds.has(id)) {
    id = `${baseId}-${suffix}`;
    suffix += 1;
  }
  return { id, ...draft };
}

export function candidateReferenceStatus(reference: string): CandidateReferenceStatus {
  const candidate = reference.trim();
  if (!candidate) {
    return {
      deployable: false,
      kind: 'missing',
      message: '需配置 Candidate Adapter：输入本地相对路径或 oci:// 引用。',
    };
  }
  if (candidate === 'a3s-code') {
    return { deployable: true, kind: 'bundled', message: '使用 Bench 内置 A3S Code Adapter。' };
  }
  if (candidate.startsWith('./') || candidate.startsWith('../')) {
    return { deployable: true, kind: 'local', message: '本地 Adapter 将由 Bench 在部署时校验。' };
  }
  if (candidate.startsWith('oci://') && candidate.length > 'oci://'.length) {
    return { deployable: true, kind: 'oci', message: 'OCI Adapter 将由 Bench 在部署时校验。' };
  }
  return {
    deployable: false,
    kind: 'unsupported',
    message: '仅支持 a3s-code、本地相对路径或 oci:// Candidate Adapter。',
  };
}

export function candidateRunStatus(reference: string, model: string): CandidateReferenceStatus {
  const referenceStatus = candidateReferenceStatus(reference);
  if (!referenceStatus.deployable) return referenceStatus;
  if (referenceStatus.kind === 'bundled' && !model.trim()) {
    return {
      deployable: false,
      kind: 'bundled',
      message: 'A3S Code Adapter 需要配置可用的 provider/model。',
    };
  }
  return referenceStatus;
}

function pilotOption(id: PilotIdentity): HangarPilotOption {
  const option = HANGAR_PILOT_OPTIONS.find((candidate) => candidate.id === id);
  if (!option) throw new Error(`Missing hangar pilot option for ${id}`);
  return option;
}

function normalizeRosterId(id: string): string {
  return id
    .normalize('NFKC')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}
