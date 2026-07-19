import { describe, expect, it } from 'vitest';
import { resolveAircraft } from './aircraft-registry';
import { PILOT_PROFILES, pilotProfileForFamily, pilotProfileForId, resolvePilotProfile } from './pilot-profile';

describe('pilot profile', () => {
  it.each([
    ['A3S Code', 'a3s', 'spectrum-bars', 'A3S'],
    ['/opt/agents/a3s-code', 'a3s', 'spectrum-bars', 'A3S'],
    ['openai/codex-adapter', 'codex', 'orbit-ring', 'CX'],
    ['@anthropic-ai/claude-code', 'claude', 'triple-ray', 'CC'],
  ] as const)('maps %s to its pilot attire and marking', (candidate, id, pattern, helmetCode) => {
    const profile = resolvePilotProfile(candidate);

    expect(profile.id).toBe(id);
    expect(profile.marking.pattern).toBe(pattern);
    expect(profile.marking.helmetCode).toBe(helmetCode);
    expect(profile.candidateFamily).toBe(resolveAircraft(candidate).candidateFamily);
  });

  it('uses the neutral test pilot for an unknown Candidate', () => {
    expect(resolvePilotProfile('./agents/custom-adapter')).toBe(PILOT_PROFILES.unknown);
    expect(resolvePilotProfile('').marking.pattern).toBe('test-chevron');
    expect(pilotProfileForFamily('unknown')).toBe(PILOT_PROFILES.unknown);
  });

  it('resolves a pilot directly for hangar composition', () => {
    expect(pilotProfileForId('codex')).toBe(PILOT_PROFILES.codex);
    expect(pilotProfileForId('generic')).toBe(PILOT_PROFILES.unknown);
  });

  it('keeps named Agent pilots visually distinct', () => {
    const profiles = [PILOT_PROFILES['a3s-code'], PILOT_PROFILES.codex, PILOT_PROFILES['claude-code']];

    expect(new Set(profiles.map((profile) => profile.attire.flightSuit)).size).toBe(profiles.length);
    expect(new Set(profiles.map((profile) => profile.marking.pattern)).size).toBe(profiles.length);
    expect(new Set(profiles.map((profile) => profile.cockpitGlowColor)).size).toBe(profiles.length);
  });

  it('exposes immutable, render-safe color profiles', () => {
    for (const profile of Object.values(PILOT_PROFILES)) {
      expect(Object.isFrozen(profile)).toBe(true);
      expect(Object.isFrozen(profile.attire)).toBe(true);
      expect(Object.isFrozen(profile.marking)).toBe(true);
      for (const color of [
        profile.cockpitGlowColor,
        ...Object.values(profile.attire),
        profile.marking.primaryColor,
        profile.marking.secondaryColor,
      ]) {
        expect(Number.isInteger(color)).toBe(true);
        expect(color).toBeGreaterThanOrEqual(0);
        expect(color).toBeLessThanOrEqual(0xffffff);
      }
    }
  });
});
