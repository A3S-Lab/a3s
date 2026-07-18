import { describe, expect, it } from 'vitest';
import { DEFAULT_HANGAR_ROSTER } from '../../features/hangar/hangar-configuration';
import { createAircraftFleet } from './aircraft-fleet';
import { createAircraftSpotlight, updateAircraftSpotlight } from './aircraft-spotlight';
import { buildRosterFormation } from './flight-formation';

describe('aircraft spotlight', () => {
  it('moves a real light target to the selected aircraft without modifying its materials', () => {
    const fleet = createAircraftFleet(buildRosterFormation(DEFAULT_HANGAR_ROSTER));
    const spotlight = createAircraftSpotlight();
    const material = fleet.instances[0].model.getObjectByName('fuselage');

    updateAircraftSpotlight(spotlight, fleet, DEFAULT_HANGAR_ROSTER[0].id, true);

    expect(spotlight.target.position.toArray()).toEqual(fleet.instances[0].laneRoot.position.toArray());
    expect(spotlight.light.visible).toBe(true);
    expect(material?.userData.selected).toBeUndefined();
  });

  it('retargets the light when another aircraft is selected', () => {
    const fleet = createAircraftFleet(buildRosterFormation(DEFAULT_HANGAR_ROSTER));
    const spotlight = createAircraftSpotlight();

    updateAircraftSpotlight(spotlight, fleet, DEFAULT_HANGAR_ROSTER[2].id, true);

    expect(spotlight.target.position.toArray()).toEqual(fleet.instances[2].laneRoot.position.toArray());
  });
});
