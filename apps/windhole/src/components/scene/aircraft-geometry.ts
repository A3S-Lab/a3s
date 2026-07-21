import * as THREE from 'three';
import type { FuselageProfile, LoftSection, PlanPoint } from './aircraft-blueprint';

export function createLoftGeometry(sections: readonly LoftSection[], profile: FuselageProfile): THREE.BufferGeometry {
  const vertices: number[] = [];
  const indices: number[] = [];
  const radialSegments = profile.segments;
  for (const section of sections) {
    for (let segment = 0; segment < radialSegments; segment += 1) {
      const angle = (segment / radialSegments) * Math.PI * 2;
      const [vertical, lateral] = crossSectionPoint(profile, angle);
      vertices.push(section.x, (section.centerY ?? 0) + vertical * section.height, lateral * section.width);
    }
  }

  for (let section = 0; section < sections.length - 1; section += 1) {
    const current = section * radialSegments;
    const next = (section + 1) * radialSegments;
    for (let segment = 0; segment < radialSegments; segment += 1) {
      const following = (segment + 1) % radialSegments;
      indices.push(current + segment, next + segment, next + following);
      indices.push(current + segment, next + following, current + following);
    }
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(vertices, 3));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  return geometry;
}

function crossSectionPoint(profile: FuselageProfile, angle: number): readonly [vertical: number, lateral: number] {
  const cosine = Math.cos(angle);
  const sine = Math.sin(angle);
  if (profile.kind === 'ellipse') return [cosine, sine];

  const power = 2 / profile.exponent;
  return [signedPower(cosine, power), signedPower(sine, power)];
}

function signedPower(value: number, power: number): number {
  return Math.sign(value) * Math.abs(value) ** power;
}

export function createPlanform(
  points: readonly PlanPoint[],
  material: THREE.Material,
  thickness: number,
  centerY: number
): THREE.Mesh {
  const shape = shapeFromPoints(points);
  const geometry = new THREE.ExtrudeGeometry(shape, {
    depth: thickness,
    bevelEnabled: true,
    bevelSegments: 1,
    bevelSize: 0.015,
    bevelThickness: 0.015,
    curveSegments: 1,
  });
  geometry.rotateX(Math.PI / 2);
  geometry.translate(0, thickness / 2, 0);
  const mesh = new THREE.Mesh(geometry, material);
  mesh.position.y = centerY;
  return mesh;
}

export function createSideSurface(
  points: readonly PlanPoint[],
  material: THREE.Material,
  thickness: number
): THREE.Mesh {
  const geometry = new THREE.ExtrudeGeometry(shapeFromPoints(points), {
    depth: thickness,
    bevelEnabled: true,
    bevelSegments: 1,
    bevelSize: 0.014,
    bevelThickness: 0.014,
  });
  geometry.translate(0, 0, -thickness / 2);
  return new THREE.Mesh(geometry, material);
}

function shapeFromPoints(points: readonly PlanPoint[]): THREE.Shape {
  const [first, ...remaining] = points;
  const shape = new THREE.Shape();
  shape.moveTo(first[0], first[1]);
  for (const [x, y] of remaining) shape.lineTo(x, y);
  shape.closePath();
  return shape;
}
