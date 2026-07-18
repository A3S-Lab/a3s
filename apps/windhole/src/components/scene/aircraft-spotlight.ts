import * as THREE from 'three';
import type { AircraftFleet } from './aircraft-fleet';

export interface AircraftSpotlight {
  light: THREE.SpotLight;
  target: THREE.Object3D;
}

const targetPosition = new THREE.Vector3();
const lightPosition = new THREE.Vector3();
const lightColor = new THREE.Color();
const white = new THREE.Color(0xffffff);
const lightOffset = new THREE.Vector3(-1.8, 6.2, 3.8);

export function createAircraftSpotlight(): AircraftSpotlight {
  const light = new THREE.SpotLight(0xffffff, 32, 18, Math.PI / 9, 0.62, 1.4);
  light.name = 'selected-aircraft-spotlight';
  light.castShadow = true;
  light.shadow.mapSize.set(512, 512);
  light.shadow.bias = -0.0004;

  const target = new THREE.Object3D();
  target.name = 'selected-aircraft-light-target';
  light.target = target;
  return { light, target };
}

export function updateAircraftSpotlight(
  spotlight: AircraftSpotlight,
  fleet: AircraftFleet,
  selectedId: string,
  immediate = false
): void {
  const selected = fleet.instances.find((instance) => instance.descriptor.instanceId === selectedId);
  if (!selected) {
    spotlight.light.visible = false;
    return;
  }

  spotlight.light.visible = true;
  selected.laneRoot.getWorldPosition(targetPosition);
  lightPosition.copy(targetPosition).add(lightOffset);
  lightColor.set(selected.descriptor.profile.accentColor).lerp(white, 0.7);
  const blend = immediate ? 1 : 0.12;
  spotlight.target.position.lerp(targetPosition, blend);
  spotlight.light.position.lerp(lightPosition, blend);
  spotlight.light.color.lerp(lightColor, blend);
}
