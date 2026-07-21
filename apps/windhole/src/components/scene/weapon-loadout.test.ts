import { describe, expect, it } from 'vitest';
import { EFFORT_LOADOUT_MAP, resolveWeaponLoadout, WEAPON_LOADOUTS } from './weapon-loadout';

describe('resolveWeaponLoadout', () => {
  it.each([
    ['none', 'clean', 0],
    ['minimal', 'self-defense', 2],
    ['low', 'light-air-to-air', 2],
    ['medium', 'balanced-air-to-air', 4],
    ['high', 'heavy-air-to-air', 6],
    ['xhigh', 'full-combat', 8],
  ] as const)('maps %s effort to the %s loadout', (effort, loadoutId, storeCount) => {
    const loadout = resolveWeaponLoadout(effort);

    expect(loadout.id).toBe(loadoutId);
    expect(loadout.totalStores).toBe(storeCount);
    expect(loadout.visualizationOnly).toBe(true);
  });

  it.each(['x-high', 'extra_high', 'maximum'])('accepts %s as an extra-high alias', (effort) => {
    expect(resolveWeaponLoadout(effort)).toBe(WEAPON_LOADOUTS.fullCombat);
  });

  it('uses medium as the stable default for absent or unknown effort values', () => {
    expect(resolveWeaponLoadout()).toBe(WEAPON_LOADOUTS.balancedAirToAir);
    expect(resolveWeaponLoadout('unexpected')).toBe(WEAPON_LOADOUTS.balancedAirToAir);
  });

  it('keeps the mapping complete for all supported effort levels', () => {
    expect(Object.keys(EFFORT_LOADOUT_MAP)).toEqual(['none', 'minimal', 'low', 'medium', 'high', 'xhigh']);
    expect(Object.values(EFFORT_LOADOUT_MAP)).toHaveLength(6);
  });
});
