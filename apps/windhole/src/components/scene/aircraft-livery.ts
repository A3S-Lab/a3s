import * as THREE from 'three';

export type AircraftLivery = 'a3s' | 'codex' | 'claude' | 'generic';

interface AircraftLiveryMaterials {
  accent: THREE.Material;
  secondary: THREE.Material;
}

export function addAircraftLivery(
  fighter: THREE.Group,
  livery: AircraftLivery,
  materials: AircraftLiveryMaterials
): void {
  const liveryGroup = new THREE.Group();
  liveryGroup.name = `brand-livery:${livery}`;

  if (livery === 'a3s') {
    for (const side of [-1, 1]) {
      for (let stripe = 0; stripe < 3; stripe += 1) {
        const marker = new THREE.Mesh(
          new THREE.BoxGeometry(0.46 - stripe * 0.06, 0.022, 0.075),
          stripe === 1 ? materials.secondary : materials.accent
        );
        marker.position.set(0.34 + stripe * 0.22, 0.135, side * (1.05 + stripe * 0.12));
        marker.rotation.y = side * -0.28;
        marker.name = `a3s-spectrum:${side}:${stripe}`;
        liveryGroup.add(marker);
      }
    }
  } else if (livery === 'codex') {
    for (const side of [-1, 1]) {
      const outer = new THREE.Mesh(new THREE.TorusGeometry(0.18, 0.035, 8, 28), materials.accent);
      outer.rotation.x = Math.PI / 2;
      outer.scale.z = 0.72;
      outer.position.set(0.48, 0.145, side * 1.12);
      outer.name = `codex-orbit:${side}`;
      liveryGroup.add(outer);

      const core = new THREE.Mesh(new THREE.CylinderGeometry(0.055, 0.055, 0.024, 18), materials.secondary);
      core.position.set(0.48, 0.145, side * 1.12);
      core.name = `codex-core:${side}`;
      liveryGroup.add(core);
    }
  } else if (livery === 'claude') {
    for (const side of [-1, 1]) {
      for (let ray = -1; ray <= 1; ray += 1) {
        const marker = new THREE.Mesh(
          new THREE.BoxGeometry(ray === 0 ? 0.42 : 0.31, 0.022, 0.07),
          ray === 0 ? materials.accent : materials.secondary
        );
        marker.position.set(0.48 + ray * 0.16, 0.14, side * (1.22 + ray * 0.08));
        marker.rotation.y = side * 0.22;
        marker.name = `claude-ray:${side}:${ray}`;
        liveryGroup.add(marker);
      }
    }
  } else {
    const marker = new THREE.Mesh(new THREE.BoxGeometry(0.68, 0.022, 0.08), materials.accent);
    marker.position.set(0.4, 0.135, 0.92);
    marker.name = 'generic-livery-marker';
    liveryGroup.add(marker);
  }

  fighter.add(liveryGroup);
}
