import { describe, expect, it } from 'vitest';
import { buildRosterFormation, type FormationRosterEntry, selectedFormationId } from './flight-formation';

const HANGAR_ROSTER = [
  {
    id: 'a3s-lead',
    airframeId: 'j-50',
    pilotId: 'a3s',
    candidate: 'a3s-code',
    callsign: 'A3S-01',
    model: 'anthropic/glm-5.2',
    effort: 'high',
  },
  {
    id: 'codex-wing',
    airframeId: 'f-35',
    pilotId: 'codex',
    candidate: 'codex',
    callsign: 'CODEX-02',
    model: 'openai/gpt-5.6',
    effort: 'medium',
  },
  {
    id: 'claude-cover',
    airframeId: 'f-22',
    pilotId: 'claude',
    candidate: 'claude-code',
    callsign: 'CLAUDE-03',
    model: 'anthropic/claude-opus-4.6',
    effort: 'low',
  },
  {
    id: 'experimental-lambda',
    airframeId: 'j-50',
    pilotId: 'generic',
    candidate: './candidates/lambda',
    callsign: 'LAMBDA-04',
    model: 'vendor/lambda',
    effort: 'minimal',
  },
  {
    id: 'prototype-chase',
    airframeId: 'prototype',
    pilotId: 'generic',
    candidate: './candidates/chase',
    callsign: 'CHASE-05',
    model: '',
    effort: 'none',
  },
] as const satisfies readonly FormationRosterEntry[];

describe('flight formation', () => {
  it('projects the saved named-Agent roster into distinct airframes and independent lanes', () => {
    const formation = buildRosterFormation(HANGAR_ROSTER.slice(0, 3));

    expect(formation.map((entry) => [entry.candidateLabel, entry.profile.candidateFamily])).toEqual([
      ['A3S-01', 'a3s-code'],
      ['CODEX-02', 'codex'],
      ['CLAUDE-03', 'claude-code'],
    ]);
    expect(formation.map((entry) => entry.configuration.airframe.airframe.id)).toEqual(['j-50', 'f-35', 'f-22']);
    expect(formation.map((entry) => entry.pilot.id)).toEqual(['a3s', 'codex', 'claude']);
    for (let first = 0; first < formation.length; first += 1) {
      for (let second = first + 1; second < formation.length; second += 1) {
        expect(distance(formation[first].position, formation[second].position)).toBeGreaterThan(4.2);
      }
    }
  });

  it('selects an unknown roster Candidate by exact identity without a preferred entry id', () => {
    const formation = buildRosterFormation(HANGAR_ROSTER);

    expect(selectedFormationId(formation, './candidates/chase')).toBe('prototype-chase');
  });

  it('selects a named Agent through its Candidate alias', () => {
    const formation = buildRosterFormation(HANGAR_ROSTER);

    expect(selectedFormationId(formation, 'openai/codex-adapter')).toBe('codex-wing');
  });

  it.each([1, 2, 3, 4, 5])('builds a %i-aircraft hangar roster in unique formation slots', (size) => {
    const roster = HANGAR_ROSTER.slice(0, size);
    const formation = buildRosterFormation(roster);

    expect(formation).toHaveLength(size);
    expect(formation.map((entry) => entry.instanceId)).toEqual(roster.map((entry) => entry.id));
    expect(formation.map((entry) => entry.candidateLabel)).toEqual(roster.map((entry) => entry.callsign));
    expect(formation.map((entry) => entry.configuration.airframe.airframe.id)).toEqual(
      roster.map((entry) => entry.airframeId)
    );
    expect(formation.map((entry) => entry.pilot.id)).toEqual(roster.map((entry) => entry.pilotId));
    expect(new Set(formation.map((entry) => entry.position.join(','))).size).toBe(size);
    for (let first = 0; first < formation.length; first += 1) {
      for (let second = first + 1; second < formation.length; second += 1) {
        expect(distance(formation[first].position, formation[second].position)).toBeGreaterThan(4.2);
      }
    }
  });

  it('uses each roster entry as the only formation source', () => {
    const revisedRoster = HANGAR_ROSTER.map((entry) =>
      entry.id === 'codex-wing'
        ? {
            ...entry,
            candidate: './candidates/revised-codex',
            model: 'vendor/revised-model',
            effort: 'xhigh' as const,
            airframeId: 'j-50' as const,
          }
        : entry
    );
    const formation = buildRosterFormation(revisedRoster);
    const inactive = formation[0];
    const active = formation[1];

    expect(inactive).toMatchObject({
      candidate: 'a3s-code',
      candidateLabel: 'A3S-01',
      effort: 'high',
      model: 'anthropic/glm-5.2',
      pilot: { id: 'a3s' },
      configuration: { airframe: { airframe: { id: 'j-50' } } },
    });
    expect(active).toMatchObject({
      candidate: './candidates/revised-codex',
      candidateLabel: 'CODEX-02',
      effort: 'xhigh',
      model: 'vendor/revised-model',
      pilot: { id: 'codex' },
      configuration: {
        airframe: { airframe: { id: 'j-50' } },
        loadout: { id: 'full-combat' },
      },
    });
    expect(selectedFormationId(formation, active.candidate, 'codex-wing')).toBe('codex-wing');
  });

  it('preserves named Agent branding when its executable Adapter is not configured yet', () => {
    const roster = HANGAR_ROSTER.slice(0, 3).map((entry) =>
      entry.pilotId === 'a3s' ? entry : { ...entry, candidate: '', model: '' }
    );

    const formation = buildRosterFormation(roster);

    expect(formation.map((entry) => [entry.pilot.id, entry.profile.candidateFamily])).toEqual([
      ['a3s', 'a3s-code'],
      ['codex', 'codex'],
      ['claude', 'claude-code'],
    ]);
  });

  it('caps defensive roster input at the five available scene slots', () => {
    const oversizedRoster = [...HANGAR_ROSTER, { ...HANGAR_ROSTER[4], id: 'sixth-aircraft' }];

    expect(buildRosterFormation(oversizedRoster)).toHaveLength(5);
  });
});

function distance(first: readonly number[], second: readonly number[]): number {
  return Math.hypot(first[0] - second[0], first[1] - second[1], first[2] - second[2]);
}
