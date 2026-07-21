import * as THREE from 'three';
import type { AircraftBlueprint, PlanPoint } from './aircraft-blueprint';
import { createPlanform, createSideSurface } from './aircraft-geometry';
import type { AircraftMaterials } from './aircraft-materials';

export function createSurfaceModules(blueprint: AircraftBlueprint, materials: AircraftMaterials): THREE.Object3D[] {
  return [
    ...createMirroredPlanforms(blueprint.surfaces.chine, materials.upperSkin, 0.075, 0.07, 'stealth-chine'),
    ...createMirroredPlanforms(blueprint.surfaces.wing, materials.skin, 0.105, -0.025, 'main-wing'),
    ...createMirroredPlanforms(blueprint.surfaces.stabilizer, materials.upperSkin, 0.08, 0, 'stabilizer'),
    ...createVerticalTails(blueprint, materials.skin),
  ];
}

export function createMirroredPlanforms(
  points: readonly PlanPoint[],
  material: THREE.Material,
  thickness: number,
  centerY: number,
  name: string
): THREE.Mesh[] {
  if (points.length < 3) return [];
  const right = createPlanform(points, material, thickness, centerY);
  right.name = `${name}:right`;

  const leftPoints = points.map(([x, z]) => [x, -z] as const).reverse();
  const left = createPlanform(leftPoints, material, thickness, centerY);
  left.name = `${name}:left`;
  return [right, left];
}

function createVerticalTails(blueprint: AircraftBlueprint, material: THREE.Material): THREE.Mesh[] {
  return blueprint.tail.offsets.map((offset, index) => {
    const tail = createSideSurface(blueprint.surfaces.verticalTail, material, 0.08);
    tail.position.z = offset;
    tail.rotation.x = Math.sign(offset) * -blueprint.tail.cant;
    tail.name = `vertical-tail:${index}`;
    return tail;
  });
}
