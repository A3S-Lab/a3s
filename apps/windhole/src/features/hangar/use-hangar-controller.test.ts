import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';
import { labState } from '../../state/lab-state';
import { createHangarDraft, DEFAULT_HANGAR_ROSTER, MAX_HANGAR_ROSTER_SIZE } from './hangar-configuration';
import { useHangarController } from './use-hangar-controller';

beforeEach(() => {
  labState.workspace = 'hangar';
  labState.hangar = {
    draft: createHangarDraft('generic'),
    roster: DEFAULT_HANGAR_ROSTER.map((entry) => ({ ...entry })),
    activeEntryId: DEFAULT_HANGAR_ROSTER[0].id,
  };
  labState.runConfig = {
    candidateLock: './candidate.lock.json',
    deploymentScope: 'single',
    locked: false,
    taskLock: './task.lock.json',
  };
  labState.run = { stage: 'idle' };
  labState.campaign = { generation: 0, status: 'idle', members: [] };
  labState.notice = undefined;
});

describe('useHangarController', () => {
  it('combines an independently selected airframe with the selected pilot identity', () => {
    const { result } = renderHook(() => useHangarController());

    act(() => {
      result.current.selectAirframe('j-35');
      result.current.selectPilot('codex');
      result.current.setEffort('xhigh');
    });

    expect(labState.hangar.draft).toMatchObject({
      airframeId: 'j-35',
      pilotId: 'codex',
      candidate: '',
      model: '',
      effort: 'xhigh',
    });
  });

  it('adds the draft to the roster and activates the complete evaluation combination', () => {
    const { result } = renderHook(() => useHangarController());

    act(() => {
      result.current.selectAirframe('prototype');
      result.current.selectPilot('generic');
      result.current.setCandidate('./agents/custom');
      result.current.setModel('custom/model-v1');
      result.current.setEffort('minimal');
      result.current.setCallsign('NOVA-01');
      result.current.addToRoster();
    });

    const added = labState.hangar.roster.at(-1);
    expect(added).toMatchObject({
      airframeId: 'prototype',
      candidate: './agents/custom',
      callsign: 'NOVA-01',
      model: 'custom/model-v1',
      pilotId: 'generic',
      effort: 'minimal',
    });
    expect(labState.hangar.activeEntryId).toBe(added?.id);
    expect(labState.hangar.roster.find((entry) => entry.id === added?.id)).toMatchObject({
      airframeId: 'prototype',
      candidate: './agents/custom',
      model: 'custom/model-v1',
      effort: 'minimal',
    });
    expect(labState.runConfig.locked).toBe(false);
  });

  it('updates an existing visual preset with its real Adapter instead of creating a duplicate', () => {
    const { result } = renderHook(() => useHangarController());
    const codex = labState.hangar.roster.find((entry) => entry.pilotId === 'codex');
    expect(codex).toBeDefined();
    if (!codex) return;

    act(() => {
      result.current.activateRosterEntry(codex.id);
      result.current.setCandidate('./agents/codex-adapter');
      result.current.setModel('openai/gpt-5.6');
      result.current.updateRosterEntry();
    });

    expect(labState.hangar.roster).toHaveLength(DEFAULT_HANGAR_ROSTER.length);
    expect(labState.hangar.roster.find((entry) => entry.id === codex.id)).toMatchObject({
      candidate: './agents/codex-adapter',
      model: 'openai/gpt-5.6',
      callsign: 'CODEX-01',
    });
    expect(labState.hangar.draft.candidate).toBe('./agents/codex-adapter');
    expect(labState.notice?.message).toContain('同步到作战地图');
  });

  it('caps the formation and keeps at least one combination', () => {
    const { result } = renderHook(() => useHangarController());

    act(() => {
      result.current.setCandidate('./agents/capacity-test');
      while (labState.hangar.roster.length < MAX_HANGAR_ROSTER_SIZE) result.current.addToRoster();
      result.current.addToRoster();
    });
    expect(labState.hangar.roster).toHaveLength(MAX_HANGAR_ROSTER_SIZE);
    expect(labState.notice?.tone).toBe('error');

    act(() => {
      for (const entry of [...labState.hangar.roster].slice(1)) result.current.removeRosterEntry(entry.id);
      result.current.removeRosterEntry(labState.hangar.roster[0].id);
    });
    expect(labState.hangar.roster).toHaveLength(1);
    expect(labState.notice?.message).toContain('至少需要保留');
  });

  it('opens the evaluation workspace when launching a roster entry', () => {
    const { result } = renderHook(() => useHangarController());
    const entry = labState.hangar.roster[1];

    act(() => result.current.activateRosterEntry(entry.id, true));

    expect(labState.workspace).toBe('lab');
    expect(labState.hangar.activeEntryId).toBe(entry.id);
    expect(labState.hangar.draft).toMatchObject({
      airframeId: entry.airframeId,
      candidate: entry.candidate,
      model: entry.model,
      effort: entry.effort,
    });
    expect(labState.runConfig.locked).toBe(false);
  });

  it('locks composition mutations while a Bench run is active', () => {
    const { result } = renderHook(() => useHangarController());
    const originalRoster = labState.hangar.roster.map((entry) => ({ ...entry }));
    const originalDraft = { ...labState.hangar.draft };
    labState.run = { stage: 'candidate_running' };

    act(() => {
      result.current.setCandidate('./agents/should-not-apply');
      result.current.addToRoster();
      result.current.removeRosterEntry(labState.hangar.roster[1].id);
    });

    expect(labState.hangar.draft).toEqual(originalDraft);
    expect(labState.hangar.roster).toEqual(originalRoster);
    expect(labState.notice?.message).toContain('锁定');
  });

  it('locks the same composition mutations while a Campaign formation is active', () => {
    const { result } = renderHook(() => useHangarController());
    const originalRoster = labState.hangar.roster.map((entry) => ({ ...entry }));
    const originalDraft = { ...labState.hangar.draft };
    labState.campaign = { generation: 1, status: 'running', members: [] };

    act(() => {
      result.current.setCandidate('./agents/should-not-apply');
      result.current.addToRoster();
      result.current.removeRosterEntry(labState.hangar.roster[1].id);
    });

    expect(labState.hangar.draft).toEqual(originalDraft);
    expect(labState.hangar.roster).toEqual(originalRoster);
    expect(labState.notice?.message).toContain('锁定');
  });
});
