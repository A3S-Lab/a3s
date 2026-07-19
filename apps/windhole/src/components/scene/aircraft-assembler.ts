import * as THREE from 'three';
import type { AircraftAssemblyOptions, AircraftBlueprint } from './aircraft-blueprint';
import { addAircraftLivery } from './aircraft-livery';
import { createAircraftMaterials } from './aircraft-materials';
import { createCockpitModules } from './aircraft-module-cockpit';
import { createFuselageModule } from './aircraft-module-fuselage';
import { createPropulsionModules } from './aircraft-module-propulsion';
import { createAirframeSignature } from './aircraft-module-signature';
import { createSurfaceModules } from './aircraft-module-surfaces';
import { createWeaponLoadoutVisual } from './weapon-loadout-visual';

export function assembleAircraft(blueprint: AircraftBlueprint, options: AircraftAssemblyOptions = {}): THREE.Group {
  const aircraft = new THREE.Group();
  aircraft.name = `aircraft:${blueprint.id}`;
  aircraft.userData.aircraftModelId = blueprint.id;
  aircraft.userData.aircraftBlueprint = {
    id: blueprint.id,
    displayName: blueprint.displayName,
    layout: { ...blueprint.layout },
  };
  aircraft.userData.callsign = options.callsign;
  aircraft.userData.livery = options.livery ?? 'generic';
  aircraft.userData.coordinateSystem = { forwardAxis: '-x', upAxis: '+y', spanAxis: '+z' };

  const materials = createAircraftMaterials(blueprint, options);
  aircraft.add(createFuselageModule(blueprint, materials));
  aircraft.add(...createSurfaceModules(blueprint, materials));
  aircraft.add(...createPropulsionModules(blueprint, materials));
  aircraft.add(createAirframeSignature(blueprint, materials));
  aircraft.add(...createCockpitModules(blueprint, materials, options.pilotProfile));

  addAircraftLivery(aircraft, options.livery ?? 'generic', materials);
  if (options.weaponLoadout) aircraft.add(createWeaponLoadoutVisual(options.weaponLoadout));

  const marker = new THREE.Mesh(new THREE.BoxGeometry(0.62, 0.026, 0.11), materials.accent);
  marker.name = 'identity-marker';
  marker.position.set(-0.22, 0.49, 0);
  aircraft.add(marker);

  aircraft.traverse((object) => {
    if (object instanceof THREE.Mesh) {
      object.castShadow = true;
      object.receiveShadow = true;
    }
  });
  return aircraft;
}
