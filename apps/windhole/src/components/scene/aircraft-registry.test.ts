import { describe, expect, it } from 'vitest';
import { AIRCRAFT_PROFILES, createAircraft, resolveAircraft } from './aircraft-registry';

describe('aircraft registry', () => {
  it.each([
    ['a3s-code', 'j-35'],
    ['/opt/agents/a3s-code', 'j-35'],
    ['A3S Code', 'j-35'],
    ['codex', 'f-35'],
    ['openai/codex-adapter', 'f-35'],
    ['Claude Code', 'f-22'],
    ['@anthropic-ai/claude-code', 'f-22'],
  ])('maps %s to %s', (candidate, expectedModel) => {
    expect(resolveAircraft(candidate).id).toBe(expectedModel);
  });

  it('uses the generic prototype for an unknown or blank Candidate', () => {
    expect(resolveAircraft('./candidate').id).toBe('prototype');
    expect(resolveAircraft('')).toBe(AIRCRAFT_PROFILES.prototype);
  });

  it('creates independent, metadata-bearing aircraft instances', () => {
    const first = createAircraft('codex');
    const second = createAircraft('openai');

    expect(first).not.toBe(second);
    expect(first.name).toBe('aircraft:f-35');
    expect(first.userData.aircraft).toMatchObject({
      modelId: 'f-35',
      candidateFamily: 'codex',
      forwardAxis: '-x',
    });
  });

  it('can apply an Agent theme to a model-selected airframe', () => {
    const aircraft = createAircraft('a3s-code', { airframeId: 'f-35' });

    expect(aircraft.name).toBe('aircraft:f-35');
    expect(aircraft.userData.livery).toBe('a3s');
    expect(aircraft.userData.aircraft).toMatchObject({ candidateFamily: 'a3s-code', modelId: 'f-35' });
  });

  it('keeps the pilot brand theme independent from the executable Candidate reference', () => {
    const aircraft = createAircraft('', { airframeId: 'f-35', candidateFamily: 'codex' });

    expect(aircraft.name).toBe('aircraft:f-35');
    expect(aircraft.userData.livery).toBe('codex');
    expect(aircraft.userData.aircraft).toMatchObject({ candidateFamily: 'codex', modelId: 'f-35' });
  });

  it('keeps every public profile addressable by its canonical Candidate family', () => {
    for (const profile of Object.values(AIRCRAFT_PROFILES)) {
      expect(resolveAircraft(profile.candidateFamily)).toBe(profile);
    }
  });

  it('gives every named agent a distinct brand theme on the aircraft itself', () => {
    const namedProfiles = [AIRCRAFT_PROFILES.a3sCode, AIRCRAFT_PROFILES.codex, AIRCRAFT_PROFILES.claude];

    expect(new Set(namedProfiles.map((profile) => profile.accentColor)).size).toBe(namedProfiles.length);
    expect(namedProfiles.map((profile) => profile.create().userData.livery)).toEqual(['a3s', 'codex', 'claude']);
  });
});
