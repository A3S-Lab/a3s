import * as THREE from 'three';
import type { AircraftBlueprint } from './aircraft-blueprint';
import type { AircraftMaterials } from './aircraft-materials';

export function createPropulsionModules(blueprint: AircraftBlueprint, materials: AircraftMaterials): THREE.Object3D[] {
  const modules: THREE.Object3D[] = [];
  const { engineOffsets } = blueprint.propulsion;

  for (const [index, offset] of engineOffsets.entries()) {
    modules.push(createNacelle(blueprint, materials, index, offset));
    modules.push(createNozzle(blueprint, materials, index, offset));
    modules.push(createNozzleTrim(blueprint, materials, index, offset));
  }
  modules.push(...createIntakes(blueprint, materials));
  return modules;
}

function createNacelle(
  blueprint: AircraftBlueprint,
  materials: AircraftMaterials,
  index: number,
  offset: number
): THREE.Mesh {
  const { engineRadius, nacelleLength, nacellePosition, nozzleStyle } = blueprint.propulsion;
  const geometry =
    nozzleStyle === 'rectangular'
      ? new THREE.BoxGeometry(nacelleLength, engineRadius * 1.35, engineRadius * 1.95)
      : new THREE.CylinderGeometry(engineRadius * 0.82, engineRadius, nacelleLength, 14);
  if (geometry instanceof THREE.CylinderGeometry) geometry.rotateZ(Math.PI / 2);

  const nacelle = new THREE.Mesh(geometry, materials.panel);
  nacelle.position.set(nacellePosition[0], nacellePosition[1], offset);
  nacelle.name = `engine-nacelle:${index}`;
  nacelle.userData.style = nozzleStyle === 'rectangular' ? 'engine-deck' : 'round-nacelle';
  return nacelle;
}

function createNozzle(
  blueprint: AircraftBlueprint,
  materials: AircraftMaterials,
  index: number,
  offset: number
): THREE.Object3D {
  const { engineRadius, nacellePosition, nozzleStyle, nozzleX } = blueprint.propulsion;
  if (nozzleStyle === 'rectangular') {
    const nozzle = new THREE.Group();
    nozzle.name = `engine-nozzle:${index}`;
    nozzle.position.set(nozzleX, nacellePosition[1], offset);
    nozzle.userData.style = nozzleStyle;

    const housing = new THREE.Mesh(
      new THREE.BoxGeometry(0.48, engineRadius * 1.28, engineRadius * 1.9),
      materials.nozzle
    );
    housing.name = 'rectangular-nozzle-housing';
    nozzle.add(housing);

    const core = new THREE.Mesh(
      new THREE.BoxGeometry(0.07, engineRadius * 0.86, engineRadius * 1.48),
      materials.intake
    );
    core.name = 'rectangular-nozzle-core';
    core.position.x = 0.27;
    nozzle.add(core);
    return nozzle;
  }

  const radialSegments = nozzleStyle === 'serrated-round' ? 12 : 20;
  const nozzle = new THREE.Mesh(
    new THREE.CylinderGeometry(engineRadius * 0.91, engineRadius, 0.48, radialSegments, 1, true),
    materials.nozzle
  );
  nozzle.geometry.rotateZ(Math.PI / 2);
  nozzle.position.set(nozzleX, nacellePosition[1], offset);
  nozzle.name = `engine-nozzle:${index}`;
  nozzle.userData.style = nozzleStyle;
  return nozzle;
}

function createNozzleTrim(
  blueprint: AircraftBlueprint,
  materials: AircraftMaterials,
  index: number,
  offset: number
): THREE.Object3D {
  const { engineRadius, exhaustX, nacellePosition, nozzleStyle } = blueprint.propulsion;
  if (nozzleStyle === 'rectangular') {
    const frame = new THREE.Group();
    frame.name = `nozzle-ring:${index}`;
    frame.position.set(exhaustX, nacellePosition[1], offset);
    const height = engineRadius * 1.28;
    const width = engineRadius * 1.9;
    for (const y of [-height / 2, height / 2]) {
      const bar = new THREE.Mesh(new THREE.BoxGeometry(0.055, 0.035, width), materials.accent);
      bar.position.y = y;
      frame.add(bar);
    }
    for (const z of [-width / 2, width / 2]) {
      const bar = new THREE.Mesh(new THREE.BoxGeometry(0.055, height, 0.035), materials.accent);
      bar.position.z = z;
      frame.add(bar);
    }
    return frame;
  }

  const tubularSegments = nozzleStyle === 'serrated-round' ? 12 : 28;
  const ring = new THREE.Mesh(
    new THREE.TorusGeometry(engineRadius * 0.96, 0.026, 8, tubularSegments),
    materials.accent
  );
  ring.rotation.y = Math.PI / 2;
  ring.position.set(exhaustX, nacellePosition[1], offset);
  ring.name = `nozzle-ring:${index}`;
  return ring;
}

function createIntakes(blueprint: AircraftBlueprint, materials: AircraftMaterials): THREE.Object3D[] {
  const { intakeOffset, intakeStyle } = blueprint.propulsion;
  if (intakeStyle === 'chin') return createChinIntake(materials);

  const intakes: THREE.Object3D[] = [];
  for (const side of [-1, 1]) {
    const body = createSideIntakeBody(intakeStyle, materials);
    body.position.set(intakeStyle === 'box' ? -0.12 : -0.28, intakeStyle === 'box' ? -0.1 : -0.04, side * intakeOffset);
    body.rotation.y = side * (intakeStyle === 'caret' ? -0.16 : -0.08);
    body.name = `intake-body:${side}`;
    body.userData.style = intakeStyle;
    intakes.push(body);

    const mouth = createSideIntakeMouth(intakeStyle, materials);
    mouth.position.set(intakeStyle === 'box' ? -0.92 : -0.86, -0.07, side * (intakeOffset + 0.02));
    mouth.rotation.y = side * (intakeStyle === 'caret' ? -0.2 : -0.12);
    mouth.name = `intake-mouth:${side}`;
    mouth.userData.style = intakeStyle;
    intakes.push(mouth);
  }
  return intakes;
}

function createSideIntakeBody(
  style: Exclude<AircraftBlueprint['propulsion']['intakeStyle'], 'chin'>,
  materials: AircraftMaterials
): THREE.Mesh {
  if (style === 'dsi') {
    const bump = new THREE.Mesh(new THREE.SphereGeometry(0.5, 14, 8), materials.panel);
    bump.scale.set(1.5, 0.5, 0.7);
    return bump;
  }
  if (style === 'box') {
    return new THREE.Mesh(new THREE.BoxGeometry(1.42, 0.34, 0.36), materials.panel);
  }

  const geometry = new THREE.CylinderGeometry(0.2, 0.34, 1.34, 4);
  geometry.rotateZ(Math.PI / 2);
  geometry.scale(1, 0.8, 1.25);
  return new THREE.Mesh(geometry, materials.panel);
}

function createSideIntakeMouth(
  style: Exclude<AircraftBlueprint['propulsion']['intakeStyle'], 'chin'>,
  materials: AircraftMaterials
): THREE.Mesh {
  if (style === 'caret') {
    const geometry = new THREE.CylinderGeometry(0.26, 0.26, 0.09, 3);
    geometry.rotateZ(Math.PI / 2);
    geometry.scale(1, 0.82, 1.18);
    return new THREE.Mesh(geometry, materials.intake);
  }
  const size = style === 'box' ? [0.1, 0.42, 0.46] : [0.08, 0.34, 0.36];
  return new THREE.Mesh(new THREE.BoxGeometry(size[0], size[1], size[2]), materials.intake);
}

function createChinIntake(materials: AircraftMaterials): THREE.Object3D[] {
  const bodyGeometry = new THREE.CylinderGeometry(0.18, 0.34, 1.18, 4);
  bodyGeometry.rotateZ(Math.PI / 2);
  bodyGeometry.scale(1, 0.78, 1.3);
  const body = new THREE.Mesh(bodyGeometry, materials.panel);
  body.name = 'intake-body:center';
  body.position.set(-0.22, -0.38, 0);
  body.userData.style = 'chin';

  const mouth = new THREE.Mesh(new THREE.BoxGeometry(0.1, 0.3, 0.5), materials.intake);
  mouth.name = 'intake-mouth:center';
  mouth.position.set(-0.84, -0.39, 0);
  mouth.userData.style = 'chin';
  return [body, mouth];
}
