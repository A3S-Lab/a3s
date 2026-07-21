import * as THREE from 'three';
import type { AircraftBlueprint, SignatureDetail } from './aircraft-blueprint';
import { createPlanform } from './aircraft-geometry';
import type { AircraftMaterials } from './aircraft-materials';

export function createAirframeSignature(blueprint: AircraftBlueprint, materials: AircraftMaterials): THREE.Group {
  const signature = new THREE.Group();
  signature.name = `airframe-signature:${blueprint.id}`;
  signature.userData.details = blueprint.signatureDetails.map((detail) => detail.id);

  for (const detail of blueprint.signatureDetails) {
    const primary = createSignatureDetail(detail, materials);
    primary.name = `signature:${detail.id}${detail.mirrored ? ':right' : ''}`;
    signature.add(primary);

    if (detail.mirrored) {
      const mirrored = createMirroredDetail(detail, materials);
      mirrored.name = `signature:${detail.id}:left`;
      signature.add(mirrored);
    }
  }
  return signature;
}

function createSignatureDetail(detail: SignatureDetail, materials: AircraftMaterials): THREE.Object3D {
  const material = materials[detail.material];
  if (detail.kind === 'box') {
    const mesh = new THREE.Mesh(new THREE.BoxGeometry(...detail.size), material);
    mesh.position.set(...detail.position);
    if (detail.rotation) mesh.rotation.set(...detail.rotation);
    return mesh;
  }
  if (detail.kind === 'ellipsoid') {
    const mesh = new THREE.Mesh(new THREE.SphereGeometry(1, 16, 8), material);
    mesh.position.set(...detail.position);
    mesh.scale.set(...detail.scale);
    return mesh;
  }
  return createPlanform(detail.points, material, detail.thickness, detail.elevation);
}

function createMirroredDetail(detail: SignatureDetail, materials: AircraftMaterials): THREE.Object3D {
  if (detail.kind === 'planform') {
    const points = detail.points.map(([x, z]) => [x, -z] as const).reverse();
    return createPlanform(points, materials[detail.material], detail.thickness, detail.elevation);
  }

  const mirrored = createSignatureDetail(detail, materials);
  mirrored.position.z *= -1;
  mirrored.rotation.y *= -1;
  mirrored.rotation.x *= -1;
  return mirrored;
}
