import { describe, expect, it } from 'vitest';
import { createHangarPreviewAircraft } from './hangar-preview-runtime';
import { pilotProfileForId } from './pilot-profile';
import { resolveWeaponLoadout } from './weapon-loadout';

describe('hangar preview aircraft identity', () => {
  it.each([
    ['a3s', 'j-50', 'a3s'],
    ['codex', 'f-35', 'codex'],
    ['claude', 'f-22', 'claude'],
    ['generic', 'prototype', 'generic'],
  ] as const)('keeps the %s pilot brand when the executable Candidate is blank', (pilotId, airframeId, livery) => {
    const aircraft = createHangarPreviewAircraft({
      airframeId,
      candidate: '',
      pilotProfile: pilotProfileForId(pilotId),
      loadout: resolveWeaponLoadout('high'),
    });

    expect(aircraft.userData.livery).toBe(livery);
    expect(aircraft.userData.aircraft).toMatchObject({
      candidateFamily: pilotProfileForId(pilotId).candidateFamily,
      modelId: airframeId,
    });
    expect(aircraft.getObjectByName(`pilot:${pilotId}`)).toBeDefined();
    expect(aircraft.getObjectByName('weapon-loadout:heavy-air-to-air')).toBeDefined();
  });
});
