import * as THREE from 'three';
import type { AircraftBlueprint } from './aircraft-blueprint';
import type { AircraftMaterials } from './aircraft-materials';
import { createPilotFigure } from './pilot-figure';
import type { PilotProfile } from './pilot-profile';

export function createCockpitModules(
  blueprint: AircraftBlueprint,
  materials: AircraftMaterials,
  pilotProfile?: PilotProfile
): THREE.Object3D[] {
  const modules: THREE.Object3D[] = [];
  if (pilotProfile) modules.push(createOccupiedCockpit(blueprint, pilotProfile));

  const canopySegments =
    blueprint.cockpit.style === 'faceted' ? 8 : blueprint.cockpit.style === 'low-profile' ? 14 : 24;
  const canopy = new THREE.Mesh(
    new THREE.SphereGeometry(0.66, canopySegments, Math.max(6, canopySegments / 2), 0, Math.PI * 2, 0, Math.PI / 2),
    materials.glass
  );
  canopy.scale.set(...blueprint.cockpit.scale);
  canopy.position.set(blueprint.cockpit.position[0], blueprint.cockpit.position[1], 0);
  canopy.name = 'canopy';
  canopy.userData.style = blueprint.cockpit.style;
  modules.push(canopy);

  const frame = new THREE.Mesh(new THREE.TorusGeometry(0.45, 0.024, 6, 24, Math.PI), materials.panel);
  frame.scale.set(1.3, 1, 0.72);
  frame.rotation.y = Math.PI / 2;
  frame.position.set(blueprint.cockpit.position[0] + 0.3, blueprint.cockpit.position[1], 0);
  frame.name = 'canopy-frame';
  modules.push(frame);
  return modules;
}

function createOccupiedCockpit(blueprint: AircraftBlueprint, profile: PilotProfile): THREE.Group {
  const cockpit = new THREE.Group();
  cockpit.name = 'cockpit-anchor';
  cockpit.userData.pilot = {
    id: profile.id,
    displayName: profile.displayName,
    helmetCode: profile.marking.helmetCode,
    attire: profile.marking.label,
  };

  const seat = new THREE.Mesh(
    new THREE.BoxGeometry(0.46, 0.5, 0.42),
    new THREE.MeshStandardMaterial({ color: 0x172125, metalness: 0.62, roughness: 0.48 })
  );
  seat.name = 'ejection-seat';
  seat.position.set(blueprint.cockpit.position[0] + 0.26, blueprint.cockpit.position[1] + 0.02, 0);
  seat.rotation.z = -0.16;
  cockpit.add(seat);

  const pilot = createPilotFigure(profile);
  pilot.position.set(blueprint.cockpit.position[0] + 0.02, blueprint.cockpit.position[1] + 0.05, 0);
  pilot.scale.setScalar(0.82);
  cockpit.add(pilot);

  const consoleGlow = new THREE.Mesh(
    new THREE.BoxGeometry(0.25, 0.025, 0.38),
    new THREE.MeshStandardMaterial({
      color: profile.cockpitGlowColor,
      emissive: profile.cockpitGlowColor,
      emissiveIntensity: 1.5,
      metalness: 0.4,
      roughness: 0.25,
    })
  );
  consoleGlow.name = 'cockpit-agent-console';
  consoleGlow.position.set(blueprint.cockpit.position[0] - 0.45, blueprint.cockpit.position[1] + 0.13, 0);
  consoleGlow.rotation.z = 0.24;
  cockpit.add(consoleGlow);
  return cockpit;
}
