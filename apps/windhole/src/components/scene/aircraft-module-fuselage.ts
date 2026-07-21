import * as THREE from 'three';
import type { AircraftBlueprint } from './aircraft-blueprint';
import { createLoftGeometry } from './aircraft-geometry';
import type { AircraftMaterials } from './aircraft-materials';

export function createFuselageModule(blueprint: AircraftBlueprint, materials: AircraftMaterials): THREE.Mesh {
  const fuselage = new THREE.Mesh(
    createLoftGeometry(blueprint.fuselage.sections, blueprint.fuselage.profile),
    materials.skin
  );
  fuselage.name = 'fuselage';
  fuselage.userData.profile = blueprint.fuselage.profile.kind;
  return fuselage;
}
