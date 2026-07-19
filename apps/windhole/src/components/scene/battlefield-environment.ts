import * as THREE from 'three';
import { type BattlefieldScenery, createBattlefieldScenery } from './battlefield-scenery';
import { type BattlefieldTheaterProfile, taskBattlefieldTheater } from './battlefield-theater';

interface BattlefieldStage {
  readonly group: THREE.Group;
  readonly scenery: BattlefieldScenery;
  readonly clouds: THREE.Group;
  readonly cloudMaterial: THREE.MeshStandardMaterial;
}

export interface BattlefieldEnvironment {
  readonly scene: THREE.Scene;
  group: THREE.Group;
  profile: BattlefieldTheaterProfile;
  readonly fog: THREE.FogExp2;
  readonly originalBackground: THREE.Scene['background'];
  readonly originalFog: THREE.Scene['fog'];
  readonly originalFogColor?: THREE.Color;
  stage: BattlefieldStage;
  disposed: boolean;
}

export function createBattlefieldEnvironment(
  scene: THREE.Scene,
  taskId: string,
  category?: string
): BattlefieldEnvironment {
  const profile = taskBattlefieldTheater(taskId, category);
  const originalFog = scene.fog;
  const fog = originalFog instanceof THREE.FogExp2 ? originalFog : new THREE.FogExp2(profile.palette.fog, 0.014);
  const stage = createStage(profile);
  const system: BattlefieldEnvironment = {
    scene,
    group: stage.group,
    profile,
    fog,
    originalBackground: scene.background,
    originalFog,
    originalFogColor: originalFog?.color.clone(),
    stage,
    disposed: false,
  };

  scene.fog = fog;
  scene.add(stage.group);
  applyAtmosphere(system);
  return system;
}

/** Safely swaps all theater-owned resources while preserving aircraft, weather, and interaction objects. */
export function setBattlefieldTheater(system: BattlefieldEnvironment, taskId: string, category?: string): boolean {
  if (system.disposed) return false;
  const nextProfile = taskBattlefieldTheater(taskId, category);
  if (nextProfile.id === system.profile.id) return false;

  const previous = system.stage;
  const next = createStage(nextProfile);
  system.scene.add(next.group);
  system.stage = next;
  system.group = next.group;
  system.profile = nextProfile;
  applyAtmosphere(system);
  system.scene.remove(previous.group);
  disposeObjectResources(previous.group);
  return true;
}

export function updateBattlefieldEnvironment(system: BattlefieldEnvironment, elapsed: number, delta: number): void {
  if (system.disposed) return;
  system.stage.scenery.update(elapsed, delta);
  system.stage.clouds.position.x = ((elapsed * 0.12 + 32) % 64) - 32;
  system.stage.clouds.position.z = Math.sin(elapsed * 0.04) * 1.2;
  system.stage.cloudMaterial.opacity = 0.18 + Math.sin(elapsed * 0.16) * 0.025;
}

export function disposeBattlefieldEnvironment(system: BattlefieldEnvironment): void {
  if (system.disposed) return;
  system.disposed = true;
  system.scene.remove(system.stage.group);
  disposeObjectResources(system.stage.group);
  system.scene.background = system.originalBackground;
  if (system.scene.fog === system.fog) {
    if (system.originalFog === system.fog && system.originalFogColor) {
      system.fog.color.copy(system.originalFogColor);
    } else {
      system.scene.fog = system.originalFog;
    }
  }
}

function createStage(profile: BattlefieldTheaterProfile): BattlefieldStage {
  const group = new THREE.Group();
  group.name = `battlefield-theater:${profile.id}`;
  const sky = createSky(profile);
  const lighting = createLighting(profile);
  const scenery = createBattlefieldScenery(profile);
  const { group: clouds, material: cloudMaterial } = createCloudBank(profile);
  group.add(sky, lighting, scenery.group, clouds);
  return { group, scenery, clouds, cloudMaterial };
}

function createSky(profile: BattlefieldTheaterProfile): THREE.Mesh<THREE.SphereGeometry, THREE.ShaderMaterial> {
  const material = new THREE.ShaderMaterial({
    name: `battlefield-sky:${profile.id}`,
    side: THREE.BackSide,
    depthWrite: false,
    fog: false,
    uniforms: {
      zenithColor: { value: new THREE.Color(profile.palette.skyZenith) },
      horizonColor: { value: new THREE.Color(profile.palette.skyHorizon) },
    },
    vertexShader: `
      varying float vHeight;
      void main() {
        vHeight = normalize(position).y;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: `
      uniform vec3 zenithColor;
      uniform vec3 horizonColor;
      varying float vHeight;
      void main() {
        float gradient = smoothstep(-0.12, 0.72, vHeight);
        gl_FragColor = vec4(mix(horizonColor, zenithColor, gradient), 1.0);
      }
    `,
  });
  const sky = new THREE.Mesh(new THREE.SphereGeometry(58, 24, 14), material);
  sky.name = 'battlefield-sky-dome';
  sky.frustumCulled = false;
  sky.renderOrder = -10;
  return sky;
}

function createLighting(profile: BattlefieldTheaterProfile): THREE.Group {
  const lighting = new THREE.Group();
  lighting.name = `battlefield-lighting:${profile.id}`;
  const ambient = new THREE.HemisphereLight(profile.lighting.sky, profile.lighting.ground, 1.2);
  ambient.name = 'battlefield-ambient-light';
  lighting.add(ambient);

  const sun = new THREE.DirectionalLight(profile.lighting.sun, profile.lighting.sunIntensity);
  sun.name = 'battlefield-sun';
  sun.position.set(...profile.lighting.sunPosition);
  sun.castShadow = true;
  sun.shadow.mapSize.set(1024, 1024);
  sun.shadow.camera.left = -18;
  sun.shadow.camera.right = 18;
  sun.shadow.camera.top = 14;
  sun.shadow.camera.bottom = -10;
  sun.shadow.camera.near = 1;
  sun.shadow.camera.far = 55;
  sun.shadow.bias = -0.0004;
  lighting.add(sun, sun.target);

  const horizonFill = new THREE.DirectionalLight(profile.palette.skyHorizon, 0.75);
  horizonFill.name = 'battlefield-horizon-fill';
  horizonFill.position.set(10, 5, -14);
  lighting.add(horizonFill);
  return lighting;
}

function createCloudBank(profile: BattlefieldTheaterProfile): {
  group: THREE.Group;
  material: THREE.MeshStandardMaterial;
} {
  const group = new THREE.Group();
  group.name = 'battlefield-cloud-bank';
  group.position.set(0, 9, -28);
  const material = new THREE.MeshStandardMaterial({
    color: profile.palette.skyHorizon,
    emissive: profile.palette.skyHorizon,
    emissiveIntensity: 0.12,
    transparent: true,
    opacity: 0.18,
    roughness: 1,
    depthWrite: false,
  });
  const cloudGeometry = new THREE.IcosahedronGeometry(1, 1);
  const random = seededRandom(stableHash(profile.id));
  for (let index = 0; index < 18; index += 1) {
    const cloud = new THREE.Mesh(cloudGeometry, material);
    cloud.name = 'battlefield-cloud';
    cloud.position.set(-30 + random() * 60, -2 + random() * 8, -8 + random() * 16);
    cloud.scale.set(2.4 + random() * 3.8, 0.45 + random() * 0.75, 0.9 + random() * 1.6);
    group.add(cloud);
  }
  return { group, material };
}

function applyAtmosphere(system: BattlefieldEnvironment): void {
  system.scene.background = new THREE.Color(system.profile.palette.skyZenith);
  system.fog.color.set(system.profile.palette.fog);
}

function disposeObjectResources(root: THREE.Object3D): void {
  const geometries = new Set<THREE.BufferGeometry>();
  const materials = new Set<THREE.Material>();
  const textures = new Set<THREE.Texture>();
  root.traverse((object) => {
    if (object instanceof THREE.Light) object.dispose();
    if (!(object instanceof THREE.Mesh || object instanceof THREE.Line || object instanceof THREE.Points)) return;
    geometries.add(object.geometry);
    for (const material of Array.isArray(object.material) ? object.material : [object.material]) {
      materials.add(material);
      for (const value of Object.values(material)) {
        if (value instanceof THREE.Texture) textures.add(value);
      }
    }
  });
  for (const texture of textures) texture.dispose();
  for (const geometry of geometries) geometry.dispose();
  for (const material of materials) material.dispose();
}

function seededRandom(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state += 0x6d2b79f5;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };
}

function stableHash(value: string): number {
  let hash = 2166136261;
  for (const character of value) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}
