import { useCallback, useMemo } from 'react';
import type { BuiltInAircraftId } from '../../components/scene/aircraft-blueprint';
import type { PilotIdentity } from '../../components/scene/pilot-profile';
import type { ReasoningEffort } from '../../components/scene/weapon-loadout';
import { isBenchRunActive, labState } from '../../state/lab-state';
import {
  candidateRunStatus,
  createHangarDraft,
  createHangarRosterEntry,
  HANGAR_PILOT_OPTIONS,
  type HangarDraft,
  type HangarRosterEntry,
  MAX_HANGAR_ROSTER_SIZE,
} from './hangar-configuration';
import { activateHangarRosterEntry } from './hangar-roster-state';

export interface HangarController {
  replaceDraft: (draft: HangarDraft) => void;
  selectAirframe: (airframeId: BuiltInAircraftId) => void;
  selectPilot: (pilotId: PilotIdentity) => void;
  setCandidate: (candidate: string) => void;
  setModel: (model: string) => void;
  setEffort: (effort: ReasoningEffort) => void;
  setCallsign: (callsign: string) => void;
  resetDraft: () => void;
  addToRoster: () => void;
  updateRosterEntry: (entryId?: string) => void;
  activateRosterEntry: (entryId: string, openLab?: boolean) => void;
  removeRosterEntry: (entryId: string) => void;
  copyRosterEntryToDraft: (entryId: string) => void;
}

export function useHangarController(): HangarController {
  const selectAirframe = useCallback((airframeId: BuiltInAircraftId) => {
    if (!ensureHangarMutable()) return;
    labState.hangar.draft.airframeId = airframeId;
  }, []);

  const selectPilot = useCallback((pilotId: PilotIdentity) => {
    if (!ensureHangarMutable()) return;
    const pilot = HANGAR_PILOT_OPTIONS.find((option) => option.id === pilotId);
    if (!pilot) return;
    Object.assign(labState.hangar.draft, {
      pilotId: pilot.id,
      candidate: pilot.candidate,
      model: pilot.defaultModel,
      callsign: nextCallsign(pilot.defaultCallsign, labState.hangar.roster),
    });
  }, []);

  const activateRosterEntry = useCallback((entryId: string, openLab = false) => {
    activateHangarRosterEntry(entryId, { openMap: openLab });
  }, []);

  const removeRosterEntry = useCallback((entryId: string) => {
    if (!ensureHangarMutable()) return;
    if (labState.hangar.roster.length <= 1) {
      labState.notice = { tone: 'error', message: '评测编队至少需要保留一组飞机与飞行员。' };
      return;
    }
    const removedIndex = labState.hangar.roster.findIndex((entry) => entry.id === entryId);
    if (removedIndex < 0) return;
    const wasActive = labState.hangar.activeEntryId === entryId;
    labState.hangar.roster.splice(removedIndex, 1);
    if (wasActive) {
      const nextEntry = labState.hangar.roster[Math.min(removedIndex, labState.hangar.roster.length - 1)];
      if (nextEntry) activateHangarRosterEntry(nextEntry.id);
    }
    labState.notice = { tone: 'info', message: '组合已从当前评测编队移除。' };
  }, []);

  const copyRosterEntryToDraft = useCallback((entryId: string) => {
    if (!ensureHangarMutable()) return;
    const entry = labState.hangar.roster.find((candidate) => candidate.id === entryId);
    if (!entry) return;
    labState.hangar.draft = {
      airframeId: entry.airframeId,
      pilotId: entry.pilotId,
      candidate: entry.candidate,
      model: entry.model,
      effort: entry.effort,
      callsign: nextCallsign(entry.callsign, labState.hangar.roster),
    };
  }, []);

  return useMemo(
    () => ({
      replaceDraft: (draft: HangarDraft) => {
        if (!ensureHangarMutable()) return;
        labState.hangar.draft = { ...draft };
      },
      selectAirframe,
      selectPilot,
      setCandidate: (candidate: string) => {
        if (!ensureHangarMutable()) return;
        labState.hangar.draft.candidate = candidate;
      },
      setModel: (model: string) => {
        if (!ensureHangarMutable()) return;
        labState.hangar.draft.model = model;
      },
      setEffort: (effort: ReasoningEffort) => {
        if (!ensureHangarMutable()) return;
        labState.hangar.draft.effort = effort;
      },
      setCallsign: (callsign: string) => {
        if (!ensureHangarMutable()) return;
        labState.hangar.draft.callsign = callsign;
      },
      resetDraft: () => {
        if (!ensureHangarMutable()) return;
        labState.hangar.draft = createHangarDraft('generic');
      },
      addToRoster: () => addDraftToRoster(),
      updateRosterEntry: (entryId?: string) => updateRosterEntryFromDraft(entryId),
      activateRosterEntry,
      removeRosterEntry,
      copyRosterEntryToDraft,
    }),
    [activateRosterEntry, copyRosterEntryToDraft, removeRosterEntry, selectAirframe, selectPilot]
  );
}

function addDraftToRoster(): void {
  if (!ensureHangarMutable()) return;
  if (labState.hangar.roster.length >= MAX_HANGAR_ROSTER_SIZE) {
    labState.notice = { tone: 'error', message: `当前编队最多容纳 ${MAX_HANGAR_ROSTER_SIZE} 组组合。` };
    return;
  }
  const draft = normalizedDraft(labState.hangar.draft);
  const candidateStatus = candidateRunStatus(draft.candidate, draft.model);
  if (!candidateStatus.deployable) {
    labState.notice = { tone: 'error', message: candidateStatus.message };
    return;
  }
  const entry = createHangarRosterEntry(
    draft,
    labState.hangar.roster.map((candidate) => candidate.id)
  );
  labState.hangar.roster.push(entry);
  activateHangarRosterEntry(entry.id);
  labState.notice = { tone: 'success', message: `${entry.callsign} 已加入评测编队并设为当前组合。` };
}

function updateRosterEntryFromDraft(entryId = labState.hangar.activeEntryId): void {
  if (!ensureHangarMutable()) return;
  const index = labState.hangar.roster.findIndex((entry) => entry.id === entryId);
  if (index < 0) {
    labState.notice = { tone: 'error', message: '当前编队成员不存在，无法保存组合。' };
    return;
  }
  const draft = normalizedDraft(labState.hangar.draft, entryId);
  const candidateStatus = candidateRunStatus(draft.candidate, draft.model);
  if (!candidateStatus.deployable) {
    labState.notice = { tone: 'error', message: candidateStatus.message };
    return;
  }
  const entry = labState.hangar.roster[index];
  Object.assign(entry, draft);
  labState.hangar.draft = { ...draft };
  labState.hangar.activeEntryId = entry.id;
  labState.runConfig.locked = false;
  labState.notice = { tone: 'success', message: `${entry.callsign} 已更新并同步到作战地图。` };
}

function normalizedDraft(draft: HangarDraft, excludedEntryId?: string): HangarDraft {
  return {
    ...draft,
    candidate: draft.candidate.trim(),
    model: draft.model.trim(),
    callsign: nextCallsign(
      draft.callsign.trim() || 'TEST-01',
      excludedEntryId ? labState.hangar.roster.filter((entry) => entry.id !== excludedEntryId) : labState.hangar.roster
    ),
  };
}

function nextCallsign(baseCallsign: string, roster: readonly HangarRosterEntry[]): string {
  const normalizedBase = baseCallsign.trim() || 'TEST-01';
  const occupied = new Set(roster.map((entry) => entry.callsign.toLocaleLowerCase()));
  if (!occupied.has(normalizedBase.toLocaleLowerCase())) return normalizedBase;
  const match = /^(.*?)(?:-(\d+))?$/.exec(normalizedBase);
  const prefix = match?.[1] || normalizedBase;
  let suffix = Number(match?.[2] ?? 1) + 1;
  while (occupied.has(`${prefix}-${String(suffix).padStart(2, '0')}`.toLocaleLowerCase())) suffix += 1;
  return `${prefix}-${String(suffix).padStart(2, '0')}`;
}

function ensureHangarMutable(): boolean {
  if (!isBenchRunActive()) return true;
  labState.notice = { tone: 'error', message: '评测运行中，机库编队已锁定。' };
  return false;
}
