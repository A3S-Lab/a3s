import { isBenchRunActive, labState } from '../../state/lab-state';
import type { HangarDraft, HangarRosterEntry } from './hangar-configuration';

export type ActiveRosterPatch = Partial<
  Pick<HangarRosterEntry, 'airframeId' | 'pilotId' | 'candidate' | 'model' | 'effort' | 'callsign'>
>;

export interface ActivateRosterOptions {
  openMap?: boolean;
  preserveLockedMode?: boolean;
}

export function activeHangarRosterEntry(): HangarRosterEntry | undefined {
  return labState.hangar.roster.find((entry) => entry.id === labState.hangar.activeEntryId);
}

export function activateHangarRosterEntry(entryId: string, options: ActivateRosterOptions = {}): boolean {
  const entry = labState.hangar.roster.find((candidate) => candidate.id === entryId);
  if (!entry) return false;

  const switchingEntry = labState.hangar.activeEntryId !== entry.id;
  if (switchingEntry && isBenchRunActive()) {
    labState.notice = { tone: 'error', message: '评测运行中，不能切换出击飞机。' };
    return false;
  }
  if (switchingEntry) {
    labState.hangar.activeEntryId = entry.id;
    labState.hangar.draft = draftFromRosterEntry(entry);
    if (!options.preserveLockedMode) labState.runConfig.locked = false;
  }
  if (options.openMap) labState.workspace = 'lab';
  return true;
}

export function updateActiveHangarRosterEntry(patch: ActiveRosterPatch): boolean {
  if (isBenchRunActive()) {
    labState.notice = { tone: 'error', message: '评测运行中，不能修改当前出击组合。' };
    return false;
  }
  const entry = activeHangarRosterEntry();
  if (!entry) return false;

  Object.assign(entry, patch);
  Object.assign(labState.hangar.draft, patch);
  return true;
}

export function draftFromRosterEntry(entry: Readonly<HangarRosterEntry>): HangarDraft {
  return {
    airframeId: entry.airframeId,
    pilotId: entry.pilotId,
    candidate: entry.candidate,
    model: entry.model,
    effort: entry.effort,
    callsign: entry.callsign,
  };
}

export function hangarDraftMatchesRosterEntry(
  draft: Readonly<HangarDraft>,
  entry: Readonly<HangarRosterEntry> | undefined
): boolean {
  if (!entry) return false;
  return (
    draft.airframeId === entry.airframeId &&
    draft.pilotId === entry.pilotId &&
    draft.candidate.trim() === entry.candidate &&
    draft.model.trim() === entry.model &&
    draft.effort === entry.effort &&
    draft.callsign.trim() === entry.callsign
  );
}
