import { describe, expect, it } from 'vitest';
import {
  candidateReferenceStatus,
  candidateRunStatus,
  createHangarDraft,
  createHangarRosterEntry,
  DEFAULT_HANGAR_ROSTER,
  HANGAR_AIRFRAME_OPTIONS,
  HANGAR_PILOT_OPTIONS,
  MAX_HANGAR_ROSTER_SIZE,
} from './hangar-configuration';

describe('hangar configuration', () => {
  it('publishes the five supported airframes with useful catalog copy', () => {
    expect(MAX_HANGAR_ROSTER_SIZE).toBe(5);
    expect(HANGAR_AIRFRAME_OPTIONS.map((option) => option.id)).toEqual(['j-50', 'j-35', 'f-35', 'f-22', 'prototype']);
    expect(HANGAR_AIRFRAME_OPTIONS.every((option) => option.role && option.description)).toBe(true);
  });

  it('publishes honest Candidate and model defaults for all four pilots', () => {
    expect(HANGAR_PILOT_OPTIONS.map((option) => option.id)).toEqual(['a3s', 'codex', 'claude', 'generic']);
    expect(HANGAR_PILOT_OPTIONS.map(({ candidate, defaultModel }) => [candidate, defaultModel])).toEqual([
      ['a3s-code', 'anthropic/glm-5.2'],
      ['', ''],
      ['', ''],
      ['', ''],
    ]);
  });

  it('provides three visual presets without inventing unavailable Candidate adapters', () => {
    expect(DEFAULT_HANGAR_ROSTER).toEqual([
      {
        id: 'a3s-j-50',
        airframeId: 'j-50',
        pilotId: 'a3s',
        candidate: 'a3s-code',
        model: 'anthropic/glm-5.2',
        effort: 'high',
        callsign: 'A3S-01',
      },
      {
        id: 'codex-f-35',
        airframeId: 'f-35',
        pilotId: 'codex',
        candidate: '',
        model: '',
        effort: 'high',
        callsign: 'CODEX-01',
      },
      {
        id: 'claude-f-22',
        airframeId: 'f-22',
        pilotId: 'claude',
        candidate: '',
        model: '',
        effort: 'high',
        callsign: 'CLAUDE-01',
      },
    ]);
    expect(Object.isFrozen(DEFAULT_HANGAR_ROSTER)).toBe(true);
  });

  it('creates an unassigned prototype draft by default', () => {
    expect(createHangarDraft()).toEqual({
      airframeId: 'prototype',
      pilotId: 'generic',
      candidate: '',
      model: '',
      effort: 'medium',
      callsign: 'TEST-01',
    });
  });

  it('derives Candidate defaults from the pilot while allowing an explicit airframe', () => {
    expect(createHangarDraft('codex', 'j-35')).toEqual({
      airframeId: 'j-35',
      pilotId: 'codex',
      candidate: '',
      model: '',
      effort: 'high',
      callsign: 'CODEX-01',
    });
    expect(createHangarDraft('a3s').airframeId).toBe('j-50');
    expect(createHangarDraft('claude').airframeId).toBe('f-22');
  });

  it.each([
    ['a3s-code', true, 'bundled', 'Bench 内置'],
    ['./agents/codex', true, 'local', '由 Bench 在部署时校验'],
    ['../adapters/claude', true, 'local', '由 Bench 在部署时校验'],
    ['oci://registry.example.com/a3s/codex:v1', true, 'oci', '由 Bench 在部署时校验'],
    ['', false, 'missing', '需配置 Candidate Adapter'],
    ['   ', false, 'missing', '需配置 Candidate Adapter'],
    ['codex', false, 'unsupported', '仅支持 a3s-code'],
    ['claude-code', false, 'unsupported', '仅支持 a3s-code'],
    ['oci://', false, 'unsupported', '仅支持 a3s-code'],
  ] as const)('classifies Candidate reference %j as %s/%s', (reference, deployable, kind, messageFragment) => {
    expect(candidateReferenceStatus(reference)).toMatchObject({
      deployable,
      kind,
      message: expect.stringContaining(messageFragment),
    });
  });

  it('requires a model route for the bundled A3S Code Adapter only', () => {
    expect(candidateRunStatus('a3s-code', '')).toMatchObject({
      deployable: false,
      kind: 'bundled',
      message: expect.stringContaining('provider/model'),
    });
    expect(candidateRunStatus('a3s-code', 'provider/model').deployable).toBe(true);
    expect(candidateRunStatus('./agents/custom', '').deployable).toBe(true);
  });

  it('creates deterministic IDs and uses the smallest available suffix', () => {
    const draft = createHangarDraft('codex');

    expect(createHangarRosterEntry(draft, []).id).toBe('codex-f-35');
    expect(createHangarRosterEntry(draft, ['codex-f-35']).id).toBe('codex-f-35-2');
    expect(createHangarRosterEntry(draft, ['codex-f-35', 'CODEX-F-35-2']).id).toBe('codex-f-35-3');
    expect(createHangarRosterEntry(draft, ['codex-f-35-2']).id).toBe('codex-f-35');
  });

  it('does not mutate the draft while creating a roster entry', () => {
    const draft = createHangarDraft('a3s');
    const snapshot = { ...draft };
    const entry = createHangarRosterEntry(draft, new Set<string>());

    expect(draft).toEqual(snapshot);
    expect(entry).toEqual({ id: 'a3s-j-50', ...snapshot });
    expect(entry).not.toBe(draft);
  });

  it('rejects entries beyond the hangar capacity', () => {
    expect(() => createHangarRosterEntry(createHangarDraft(), ['one', 'two', 'three', 'four', 'five'])).toThrow(
      `Hangar roster cannot exceed ${MAX_HANGAR_ROSTER_SIZE} aircraft`
    );
  });
});
