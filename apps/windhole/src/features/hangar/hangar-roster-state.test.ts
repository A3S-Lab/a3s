import { beforeEach, describe, expect, it } from 'vitest';
import { labState } from '../../state/lab-state';
import { createHangarDraft, DEFAULT_HANGAR_ROSTER } from './hangar-configuration';
import {
  activateHangarRosterEntry,
  activeHangarRosterEntry,
  hangarDraftMatchesRosterEntry,
  updateActiveHangarRosterEntry,
} from './hangar-roster-state';

beforeEach(() => {
  labState.workspace = 'hangar';
  labState.hangar = {
    draft: createHangarDraft('a3s'),
    roster: DEFAULT_HANGAR_ROSTER.map((entry) => ({ ...entry })),
    activeEntryId: DEFAULT_HANGAR_ROSTER[0].id,
  };
  labState.runConfig = {
    candidateLock: './candidate.lock.json',
    deploymentScope: 'single',
    locked: true,
    taskLock: './task.lock.json',
  };
  labState.run = { stage: 'idle' };
  labState.campaign = { generation: 0, status: 'idle', members: [] };
});

describe('hangar roster state', () => {
  it('activates one authoritative roster entry across the hangar and map', () => {
    const codex = labState.hangar.roster[1];

    expect(activateHangarRosterEntry(codex.id, { openMap: true })).toBe(true);

    expect(labState.workspace).toBe('lab');
    expect(labState.hangar.activeEntryId).toBe(codex.id);
    expect(labState.hangar.draft).toEqual({
      airframeId: codex.airframeId,
      pilotId: codex.pilotId,
      candidate: codex.candidate,
      model: codex.model,
      effort: codex.effort,
      callsign: codex.callsign,
    });
    expect(labState.runConfig.locked).toBe(false);
    expect(activeHangarRosterEntry()?.id).toBe(codex.id);
  });

  it('writes map-side configuration changes back to only the active roster entry', () => {
    const originalA3s = { ...labState.hangar.roster[0] };
    const codex = labState.hangar.roster[1];
    activateHangarRosterEntry(codex.id);

    expect(updateActiveHangarRosterEntry({ candidate: 'codex-next', model: 'openai/gpt-next', effort: 'xhigh' })).toBe(
      true
    );

    expect(labState.hangar.roster[0]).toEqual(originalA3s);
    expect(activeHangarRosterEntry()).toMatchObject({
      id: codex.id,
      candidate: 'codex-next',
      model: 'openai/gpt-next',
      effort: 'xhigh',
    });
    expect(labState.hangar.draft).toMatchObject({
      candidate: 'codex-next',
      model: 'openai/gpt-next',
      effort: 'xhigh',
    });
  });

  it('rejects an unknown scene selection without changing the active combination', () => {
    const activeId = labState.hangar.activeEntryId;

    expect(activateHangarRosterEntry('missing-aircraft')).toBe(false);

    expect(labState.hangar.activeEntryId).toBe(activeId);
    expect(labState.runConfig.locked).toBe(true);
  });

  it('keeps locked mode and an unsaved draft when the active aircraft is selected again', () => {
    labState.hangar.draft = createHangarDraft('generic', 'prototype');
    const unsavedDraft = { ...labState.hangar.draft };

    expect(activateHangarRosterEntry(labState.hangar.activeEntryId, { openMap: true })).toBe(true);

    expect(labState.workspace).toBe('lab');
    expect(labState.runConfig.locked).toBe(true);
    expect(labState.hangar.draft).toEqual(unsavedDraft);
  });

  it('detects whether the previewed draft is the deployed roster entry', () => {
    const activeEntry = activeHangarRosterEntry();

    expect(hangarDraftMatchesRosterEntry(labState.hangar.draft, activeEntry)).toBe(true);

    labState.hangar.draft.airframeId = 'prototype';
    expect(hangarDraftMatchesRosterEntry(labState.hangar.draft, activeEntry)).toBe(false);
  });

  it('keeps the active roster identity immutable while a Bench run is active', () => {
    const originalId = labState.hangar.activeEntryId;
    const codex = labState.hangar.roster.find((entry) => entry.pilotId === 'codex');
    expect(codex).toBeDefined();
    if (!codex) return;
    labState.run = { stage: 'candidate_running' };

    expect(activateHangarRosterEntry(codex.id)).toBe(false);
    expect(updateActiveHangarRosterEntry({ callsign: 'MUTATED-01' })).toBe(false);

    expect(labState.hangar.activeEntryId).toBe(originalId);
    expect(activeHangarRosterEntry()?.callsign).not.toBe('MUTATED-01');
    expect(labState.notice?.message).toContain('运行中');
  });

  it('keeps the active roster identity and configuration immutable while a Campaign is active', () => {
    const originalId = labState.hangar.activeEntryId;
    const codex = labState.hangar.roster.find((entry) => entry.pilotId === 'codex');
    expect(codex).toBeDefined();
    if (!codex) return;
    labState.campaign = { generation: 1, status: 'running', members: [] };

    expect(activateHangarRosterEntry(codex.id)).toBe(false);
    expect(updateActiveHangarRosterEntry({ callsign: 'MUTATED-01' })).toBe(false);

    expect(labState.hangar.activeEntryId).toBe(originalId);
    expect(activeHangarRosterEntry()?.callsign).not.toBe('MUTATED-01');
    expect(labState.notice?.message).toContain('运行中');
  });
});
