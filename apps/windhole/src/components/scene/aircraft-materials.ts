import * as THREE from 'three';
import type { AircraftAssemblyOptions, AircraftBlueprint } from './aircraft-blueprint';

export interface AircraftMaterials {
  skin: THREE.MeshPhysicalMaterial;
  upperSkin: THREE.MeshStandardMaterial;
  panel: THREE.MeshStandardMaterial;
  intake: THREE.MeshStandardMaterial;
  nozzle: THREE.MeshStandardMaterial;
  glass: THREE.MeshPhysicalMaterial;
  accent: THREE.MeshStandardMaterial;
  secondary: THREE.MeshStandardMaterial;
}

export function createAircraftMaterials(
  blueprint: AircraftBlueprint,
  options: AircraftAssemblyOptions
): AircraftMaterials {
  const baseColor = options.baseColor ?? blueprint.baseColor;
  const accentColor = options.accentColor ?? blueprint.accentColor;
  const secondaryColor = options.secondaryColor ?? 0xdcebed;

  return {
    skin: new THREE.MeshPhysicalMaterial({
      color: baseColor,
      metalness: 0.72,
      roughness: 0.32,
      clearcoat: 0.18,
      clearcoatRoughness: 0.4,
    }),
    upperSkin: new THREE.MeshStandardMaterial({ color: baseColor, metalness: 0.64, roughness: 0.4 }),
    panel: new THREE.MeshStandardMaterial({ color: 0x26363b, metalness: 0.8, roughness: 0.3 }),
    intake: new THREE.MeshStandardMaterial({ color: 0x10191d, metalness: 0.7, roughness: 0.25 }),
    nozzle: new THREE.MeshStandardMaterial({ color: 0x30383a, metalness: 0.96, roughness: 0.2 }),
    glass: new THREE.MeshPhysicalMaterial({
      color: 0x173a43,
      transparent: true,
      opacity: 0.72,
      emissive: 0x09262d,
      emissiveIntensity: 0.32,
      metalness: 0.18,
      roughness: 0.08,
      transmission: 0.42,
      thickness: 0.15,
      depthWrite: false,
    }),
    accent: new THREE.MeshStandardMaterial({
      color: accentColor,
      emissive: accentColor,
      emissiveIntensity: 0.38,
      metalness: 0.52,
      roughness: 0.34,
    }),
    secondary: new THREE.MeshStandardMaterial({
      color: secondaryColor,
      emissive: secondaryColor,
      emissiveIntensity: 0.14,
      metalness: 0.48,
      roughness: 0.4,
    }),
  };
}
