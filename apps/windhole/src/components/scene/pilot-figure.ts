import * as THREE from 'three';
import type { PilotMarkingPattern, PilotProfile } from './pilot-profile';

export function createPilotFigure(profile: PilotProfile): THREE.Group {
  const pilot = new THREE.Group();
  pilot.name = `pilot:${profile.id}`;
  pilot.userData.pilot = {
    id: profile.id,
    displayName: profile.displayName,
    attire: profile.marking.label,
    helmetCode: profile.marking.helmetCode,
  };

  const suit = material(profile.attire.flightSuit, 0.78, 0.08);
  const helmet = material(profile.attire.helmet, 0.3, 0.55);
  const visor = material(profile.attire.visor, 0.12, 0.82);
  const harness = material(profile.attire.harness, 0.52, 0.24);
  const gloves = material(profile.attire.gloves, 0.62, 0.16);
  const marking = material(profile.marking.primaryColor, 0.38, 0.42);
  const secondaryMarking = material(profile.marking.secondaryColor, 0.42, 0.32);

  const torso = new THREE.Mesh(new THREE.CylinderGeometry(0.24, 0.32, 0.46, 10), suit);
  torso.name = 'pilot-flight-suit';
  torso.position.y = 0.02;
  pilot.add(torso);

  for (const side of [-1, 1]) {
    const arm = new THREE.Mesh(new THREE.CylinderGeometry(0.075, 0.085, 0.34, 8), suit);
    arm.name = `pilot-sleeve:${side}`;
    arm.position.set(-0.02, 0.05, side * 0.27);
    arm.rotation.x = side * 0.32;
    pilot.add(arm);

    const glove = new THREE.Mesh(new THREE.SphereGeometry(0.085, 10, 7), gloves);
    glove.name = `pilot-glove:${side}`;
    glove.position.set(-0.1, -0.08, side * 0.32);
    pilot.add(glove);

    const strap = new THREE.Mesh(new THREE.BoxGeometry(0.055, 0.38, 0.035), harness);
    strap.name = `pilot-harness:${side}`;
    strap.position.set(-0.24, 0.04, side * 0.12);
    strap.rotation.z = side * 0.13;
    pilot.add(strap);
  }

  const buckle = new THREE.Mesh(new THREE.BoxGeometry(0.055, 0.07, 0.09), secondaryMarking);
  buckle.name = 'pilot-harness-buckle';
  buckle.position.set(-0.275, -0.1, 0);
  pilot.add(buckle);

  const helmetShell = new THREE.Mesh(new THREE.SphereGeometry(0.225, 18, 12), helmet);
  helmetShell.name = 'pilot-helmet';
  helmetShell.position.set(-0.035, 0.36, 0);
  pilot.add(helmetShell);

  const visorShell = new THREE.Mesh(new THREE.SphereGeometry(0.19, 18, 10), visor);
  visorShell.name = 'pilot-visor';
  visorShell.scale.set(0.58, 0.72, 0.94);
  visorShell.position.set(-0.16, 0.36, 0);
  pilot.add(visorShell);

  const oxygenMask = new THREE.Mesh(new THREE.ConeGeometry(0.1, 0.16, 8), harness);
  oxygenMask.name = 'pilot-oxygen-mask';
  oxygenMask.rotation.z = -Math.PI / 2;
  oxygenMask.position.set(-0.235, 0.27, 0);
  pilot.add(oxygenMask);

  addHelmetMarking(pilot, profile.marking.pattern, marking, secondaryMarking);
  pilot.traverse((object) => {
    if (object instanceof THREE.Mesh) object.castShadow = true;
  });
  return pilot;
}

function addHelmetMarking(
  pilot: THREE.Group,
  pattern: PilotMarkingPattern,
  primary: THREE.Material,
  secondary: THREE.Material
): void {
  const markings = new THREE.Group();
  markings.name = `pilot-marking:${pattern}`;

  if (pattern === 'orbit-ring') {
    const ring = new THREE.Mesh(new THREE.TorusGeometry(0.115, 0.018, 6, 20), primary);
    ring.name = 'pilot-marking-orbit';
    ring.position.set(-0.035, 0.39, 0.205);
    markings.add(ring);
  } else if (pattern === 'triple-ray') {
    for (let index = -1; index <= 1; index += 1) {
      const ray = new THREE.Mesh(
        new THREE.BoxGeometry(0.025, 0.16 - Math.abs(index) * 0.025, 0.018),
        index === 0 ? primary : secondary
      );
      ray.name = `pilot-marking-ray:${index}`;
      ray.position.set(-0.04 + index * 0.05, 0.42, 0.211);
      ray.rotation.z = index * 0.22;
      markings.add(ray);
    }
  } else if (pattern === 'test-chevron') {
    for (const side of [-1, 1]) {
      const bar = new THREE.Mesh(new THREE.BoxGeometry(0.025, 0.15, 0.018), primary);
      bar.name = `pilot-marking-chevron:${side}`;
      bar.position.set(-0.04 + side * 0.045, 0.41, 0.211);
      bar.rotation.z = side * -0.5;
      markings.add(bar);
    }
  } else {
    for (let index = -1; index <= 1; index += 1) {
      const bar = new THREE.Mesh(new THREE.BoxGeometry(0.035, 0.17, 0.018), index === 0 ? secondary : primary);
      bar.name = `pilot-marking-spectrum:${index}`;
      bar.position.set(-0.04 + index * 0.05, 0.42, 0.211);
      markings.add(bar);
    }
  }
  pilot.add(markings);
}

function material(color: THREE.ColorRepresentation, roughness: number, metalness: number): THREE.MeshStandardMaterial {
  return new THREE.MeshStandardMaterial({ color, roughness, metalness });
}
