import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import { WEAPON_LOADOUTS } from './weapon-loadout';
import { createWeaponLoadoutVisual } from './weapon-loadout-visual';

describe('weapon loadout visual', () => {
  it.each(Object.values(WEAPON_LOADOUTS))('renders the $displayName profile', (loadout) => {
    const visual = createWeaponLoadoutVisual(loadout);
    const stores = visual.children.filter((child) => child.name.startsWith('weapon-store:'));

    expect(visual.name).toBe(`weapon-loadout:${loadout.id}`);
    expect(stores).toHaveLength(loadout.totalStores);
    expect(visual.userData.loadout.visualizationOnly).toBe(true);
    expect(stores.every((store) => store instanceof THREE.Group)).toBe(true);
  });

  it('shows external pylons only for high-effort profiles', () => {
    const medium = createWeaponLoadoutVisual(WEAPON_LOADOUTS.balancedAirToAir);
    const high = createWeaponLoadoutVisual(WEAPON_LOADOUTS.heavyAirToAir);

    expect(medium.children.some((child) => child.name.startsWith('weapon-pylon:'))).toBe(false);
    expect(high.children.filter((child) => child.name.startsWith('weapon-pylon:'))).toHaveLength(2);
  });
});
