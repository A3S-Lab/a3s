import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import { assembleAircraft } from './aircraft-assembler';
import { defineAircraftBlueprint } from './aircraft-blueprint';
import { AIRCRAFT_BLUEPRINTS } from './aircraft-blueprints';

describe('assembleAircraft', () => {
  it('assembles a complete custom aircraft from copied prototype blueprint data', () => {
    const blueprint = defineAircraftBlueprint(copyPrototypeBlueprint('custom-prototype-x1'));
    const aircraft = assembleAircraft(blueprint, { callsign: 'CUSTOM-01' });
    const bounds = new THREE.Box3().setFromObject(aircraft);
    const engines = aircraft.children.filter((child) => child.name.startsWith('engine-nozzle:'));
    const tails = aircraft.children.filter((child) => child.name.startsWith('vertical-tail:'));

    expect(aircraft.name).toBe('aircraft:custom-prototype-x1');
    expect(aircraft.userData.aircraftModelId).toBe('custom-prototype-x1');
    expect(aircraft.userData.callsign).toBe('CUSTOM-01');
    expect(aircraft.userData.aircraftBlueprint).toEqual({
      id: 'custom-prototype-x1',
      displayName: 'Custom prototype custom-prototype-x1',
      layout: blueprint.layout,
    });
    expect(aircraft.userData.coordinateSystem).toEqual({ forwardAxis: '-x', upAxis: '+y', spanAxis: '+z' });

    expect(aircraft.getObjectByName('fuselage')).toBeInstanceOf(THREE.Mesh);
    expect(aircraft.getObjectByName('main-wing:left')).toBeInstanceOf(THREE.Mesh);
    expect(aircraft.getObjectByName('main-wing:right')).toBeInstanceOf(THREE.Mesh);
    expect(aircraft.getObjectByName('canopy')).toBeInstanceOf(THREE.Mesh);
    expect(aircraft.getObjectByName('identity-marker')).toBeInstanceOf(THREE.Mesh);
    expect(engines).toHaveLength(blueprint.layout.engineCount);
    expect(tails).toHaveLength(blueprint.layout.verticalTailCount);

    expect(bounds.isEmpty()).toBe(false);
    expect([...bounds.min.toArray(), ...bounds.max.toArray()].every(Number.isFinite)).toBe(true);
  });

  it.each([
    ['tailless aircraft with a vertical tail', true, [0]],
    ['tailed aircraft without a vertical tail', false, []],
  ] as const)('rejects an inconsistent %s configuration', (_case, tailless, tailOffsets) => {
    expect(() =>
      defineAircraftBlueprint(
        copyPrototypeBlueprint('invalid-tail-layout', {
          tailless,
          tailOffsets,
        })
      )
    ).toThrow('Aircraft blueprint invalid-tail-layout has an inconsistent tailless layout');
  });
});

interface PrototypeOverrides {
  tailless?: boolean;
  tailOffsets?: readonly number[];
}

function copyPrototypeBlueprint<const Id extends string>(id: Id, overrides: PrototypeOverrides = {}) {
  const prototype = AIRCRAFT_BLUEPRINTS.prototype;
  return {
    id,
    displayName: `Custom prototype ${id}`,
    baseColor: prototype.baseColor,
    accentColor: prototype.accentColor,
    layout: {
      wing: prototype.layout.wing,
      tailless: overrides.tailless ?? prototype.layout.tailless,
    },
    fuselage: {
      profile: { ...prototype.fuselage.profile },
      sections: prototype.fuselage.sections.map((section) => ({ ...section })),
    },
    surfaces: {
      chine: prototype.surfaces.chine.map(([x, z]) => [x, z] as const),
      wing: prototype.surfaces.wing.map(([x, z]) => [x, z] as const),
      stabilizer: prototype.surfaces.stabilizer.map(([x, z]) => [x, z] as const),
      verticalTail: prototype.surfaces.verticalTail.map(([x, z]) => [x, z] as const),
    },
    propulsion: {
      ...prototype.propulsion,
      engineOffsets: [...prototype.propulsion.engineOffsets],
    },
    tail: {
      cant: prototype.tail.cant,
      offsets: [...(overrides.tailOffsets ?? prototype.tail.offsets)],
    },
    cockpit: {
      position: [...prototype.cockpit.position] as const,
      scale: [...prototype.cockpit.scale] as const,
      style: prototype.cockpit.style,
    },
    signatureDetails: prototype.signatureDetails.map((detail) => ({ ...detail })),
  };
}
