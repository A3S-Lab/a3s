import * as THREE from 'three';
import type { BattlefieldTheaterProfile } from './battlefield-theater';

export type BattlefieldAnimation = (elapsed: number, delta: number) => void;

export interface SceneryContext {
  readonly root: THREE.Group;
  readonly profile: BattlefieldTheaterProfile;
  readonly animations: BattlefieldAnimation[];
  readonly random: () => number;
}

export function addGround(context: SceneryContext, color: number): void {
  const material = standard(0xffffff, 0.96);
  material.map = createSurfaceTexture(
    color,
    context.profile.palette.groundSecondary,
    `${context.profile.id}:ground`,
    'terrain'
  );
  addHorizontalPlane(context.root, 'battlefield-ground', 72, 64, 0, -3.3, -13, material);
}

export function addWater(context: SceneryContext, color: number): void {
  const material = new THREE.MeshStandardMaterial({
    color: 0xffffff,
    map: createSurfaceTexture(color, context.profile.palette.skyHorizon, `${context.profile.id}:water`, 'water'),
    roughness: 0.28,
    metalness: 0.18,
    transparent: true,
    opacity: 0.94,
  });
  const water = addHorizontalPlane(context.root, 'battlefield-water', 76, 66, 0, -3.28, -13, material);
  const waveMaterial = new THREE.LineBasicMaterial({ color: 0xa5d1d3, transparent: true, opacity: 0.17 });
  const waves = new THREE.Group();
  waves.name = 'water-wave-lines';
  for (let row = 0; row < 14; row += 1) {
    const points: THREE.Vector3[] = [];
    for (let point = 0; point < 28; point += 1) {
      points.push(new THREE.Vector3(-30 + point * 2.25, -3.21, -2 - row * 2.4 + Math.sin(point * 0.8 + row) * 0.16));
    }
    waves.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(points), waveMaterial));
  }
  context.root.add(waves);
  context.animations.push((elapsed) => {
    waves.position.x = (elapsed * 0.22) % 2.25;
    water.position.y = -3.28 + Math.sin(elapsed * 0.34) * 0.018;
  });
}

function createSurfaceTexture(
  primary: number,
  secondary: number,
  seedKey: string,
  pattern: 'terrain' | 'water'
): THREE.DataTexture {
  const size = 64;
  const data = new Uint8Array(size * size * 4);
  const base = new THREE.Color(primary);
  const detail = new THREE.Color(secondary);
  const seed = stableSurfaceHash(seedKey);
  const structured = seedKey.includes('training-range') || seedKey.includes('industrial-city');

  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const noise = surfaceNoise(x, y, seed);
      const broad = surfaceNoise(Math.floor(x / 5), Math.floor(y / 5), seed ^ 0x9e3779b9);
      const waterLine = pattern === 'water' && (y + Math.floor(Math.sin(x * 0.34) * 2)) % 11 === 0;
      const terrainTrack = pattern === 'terrain' && structured && (x % 16 === 0 || y % 16 === 0);
      const blend = Math.min(
        0.48,
        0.08 + noise * 0.14 + broad * 0.13 + (waterLine ? 0.16 : 0) + (terrainTrack ? 0.09 : 0)
      );
      const light = pattern === 'water' ? 0.91 + noise * 0.09 : 0.86 + noise * 0.18;
      const offset = (y * size + x) * 4;
      data[offset] = Math.round((base.r * (1 - blend) + detail.r * blend) * light * 255);
      data[offset + 1] = Math.round((base.g * (1 - blend) + detail.g * blend) * light * 255);
      data[offset + 2] = Math.round((base.b * (1 - blend) + detail.b * blend) * light * 255);
      data[offset + 3] = 255;
    }
  }

  const texture = new THREE.DataTexture(data, size, size, THREE.RGBAFormat);
  texture.name = `battlefield-surface:${seedKey}`;
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.wrapS = THREE.RepeatWrapping;
  texture.wrapT = THREE.RepeatWrapping;
  texture.repeat.set(pattern === 'water' ? 5 : 8, pattern === 'water' ? 6 : 7);
  texture.magFilter = THREE.LinearFilter;
  texture.minFilter = THREE.LinearMipmapLinearFilter;
  texture.generateMipmaps = true;
  texture.needsUpdate = true;
  return texture;
}

function surfaceNoise(x: number, y: number, seed: number): number {
  let value = Math.imul(x + 0x68bc21eb, 0x85ebca6b) ^ Math.imul(y + 0x02e5be93, 0xc2b2ae35) ^ seed;
  value ^= value >>> 16;
  value = Math.imul(value, 0x7feb352d);
  value ^= value >>> 15;
  return (value >>> 0) / 4294967295;
}

function stableSurfaceHash(value: string): number {
  let hash = 2166136261;
  for (const character of value) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

export function addDistantHills(
  context: SceneryContext,
  heightScale: number,
  rockColor: number,
  coverColor: number
): void {
  const rock = standard(rockColor, 1);
  const cover = standard(coverColor, 1);
  for (let index = 0; index < 11; index += 1) {
    const height = (3.5 + context.random() * 3.5) * heightScale;
    const radius = 4 + context.random() * 4;
    const hill = mesh('distant-ridge', new THREE.ConeGeometry(radius, height, 7), index % 3 === 0 ? rock : cover);
    hill.position.set(-31 + index * 6.2, -3.35 + height / 2, -27 - context.random() * 6);
    hill.rotation.y = context.random() * Math.PI;
    context.root.add(hill);
  }
}

export function addPineInstances(
  context: SceneryContext,
  count: number,
  color: number,
  minZ: number,
  maxZ: number
): void {
  const trunks = new THREE.InstancedMesh(
    new THREE.CylinderGeometry(0.09, 0.13, 1.5, 6),
    standard(0x43392d, 0.94),
    count
  );
  trunks.name = 'forest-tree-trunks';
  const crowns = new THREE.InstancedMesh(new THREE.ConeGeometry(0.75, 2.6, 7), standard(color, 1), count);
  crowns.name = 'forest-tree-crowns';
  const matrix = new THREE.Matrix4();
  const position = new THREE.Vector3();
  const quaternion = new THREE.Quaternion();
  const scale = new THREE.Vector3();
  for (let index = 0; index < count; index += 1) {
    const treeScale = 0.65 + context.random() * 0.9;
    let x = -27 + context.random() * 54;
    const z = minZ + context.random() * (maxZ - minZ);
    if (z > -10 && Math.abs(x) < 7) x += x < 0 ? -8 : 8;
    position.set(x, -3.3 + treeScale * 0.75, z);
    scale.set(treeScale, treeScale, treeScale);
    matrix.compose(position, quaternion, scale);
    trunks.setMatrixAt(index, matrix);
    position.y = -3.3 + treeScale * 2.05;
    matrix.compose(position, quaternion, scale);
    crowns.setMatrixAt(index, matrix);
  }
  trunks.receiveShadow = true;
  crowns.castShadow = true;
  context.root.add(trunks, crowns);
}

export function addBeaconRow(context: SceneryContext, y: number, z: number, count: number): void {
  const lights: THREE.Mesh[] = [];
  const material = new THREE.MeshBasicMaterial({ color: context.profile.palette.accent });
  for (let index = 0; index < count; index += 1) {
    const beacon = mesh('runway-beacon', new THREE.SphereGeometry(0.055, 6, 4), material);
    beacon.position.set(-18 + index * 4, y, z);
    context.root.add(beacon);
    lights.push(beacon);
  }
  context.animations.push((elapsed) => {
    for (const [index, light] of lights.entries()) light.visible = (elapsed * 2.2 + index * 0.13) % 1 > 0.25;
  });
}

export function addBuoys(context: SceneryContext, color: number): void {
  const material = standard(color, 0.55, 0.25);
  for (let index = 0; index < 7; index += 1) {
    const buoy = mesh('navigation-buoy', new THREE.ConeGeometry(0.16, 0.65, 8), material);
    buoy.position.set(-15 + index * 5, -2.95, -7 - (index % 2) * 2.5);
    context.root.add(buoy);
    const baseY = buoy.position.y;
    context.animations.push((elapsed) => {
      buoy.position.y = baseY + Math.sin(elapsed * 1.4 + index) * 0.08;
      buoy.rotation.z = Math.sin(elapsed * 1.1 + index) * 0.08;
    });
  }
}

export function addDustField(context: SceneryContext): void {
  const positions = new Float32Array(150 * 3);
  for (let index = 0; index < 150; index += 1) {
    positions[index * 3] = -26 + context.random() * 52;
    positions[index * 3 + 1] = -2.95 + context.random() * 1.8;
    positions[index * 3 + 2] = -5 - context.random() * 28;
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  const dust = new THREE.Points(
    geometry,
    new THREE.PointsMaterial({ color: context.profile.palette.accent, size: 0.075, transparent: true, opacity: 0.23 })
  );
  dust.name = 'desert-dust';
  context.root.add(dust);
  context.animations.push((_elapsed, delta) => {
    dust.position.x += delta * 0.22;
    if (dust.position.x > 3) dust.position.x = 0;
  });
}

export function addAurora(context: SceneryContext): void {
  const ribbons = new THREE.Group();
  ribbons.name = 'arctic-aurora';
  const material = new THREE.MeshBasicMaterial({
    color: context.profile.palette.accent,
    transparent: true,
    opacity: 0.11,
    side: THREE.DoubleSide,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
  });
  for (let index = 0; index < 4; index += 1) {
    const ribbon = mesh('aurora-ribbon', new THREE.PlaneGeometry(10, 4), material);
    ribbon.position.set(-12 + index * 8, 6 + index * 0.8, -30);
    ribbon.rotation.z = -0.08 + index * 0.04;
    ribbons.add(ribbon);
  }
  context.root.add(ribbons);
  context.animations.push((elapsed) => {
    ribbons.position.x = Math.sin(elapsed * 0.13) * 1.2;
    material.opacity = 0.08 + (Math.sin(elapsed * 0.55) + 1) * 0.035;
  });
}

export function addWindTurbine(
  context: SceneryContext,
  x: number,
  z: number,
  scale: number,
  material: THREE.Material
): void {
  const turbine = new THREE.Group();
  turbine.name = 'offshore-wind-turbine';
  turbine.position.set(x, -3.1, z);
  const tower = mesh('turbine-tower', new THREE.CylinderGeometry(0.12, 0.28, 8 * scale, 10), material);
  tower.position.y = 4 * scale;
  turbine.add(tower);
  const rotor = new THREE.Group();
  rotor.name = 'turbine-rotor';
  rotor.position.set(0, 8 * scale, 0.12);
  for (let index = 0; index < 3; index += 1) {
    const blade = mesh('turbine-blade', new THREE.BoxGeometry(0.12, 3.1 * scale, 0.06), material);
    blade.position.y = 1.5 * scale;
    blade.rotation.z = (index * Math.PI * 2) / 3;
    rotor.add(blade);
  }
  turbine.add(rotor);
  context.root.add(turbine);
  context.animations.push((_elapsed, delta) => {
    rotor.rotation.z -= delta * 0.72;
  });
}

export function addHorizontalPlane(
  parent: THREE.Object3D,
  name: string,
  width: number,
  depth: number,
  x: number,
  y: number,
  z: number,
  material: THREE.Material
): THREE.Mesh {
  const plane = mesh(name, new THREE.PlaneGeometry(width, depth), material);
  plane.rotation.x = -Math.PI / 2;
  plane.position.set(x, y, z);
  plane.receiveShadow = true;
  parent.add(plane);
  return plane;
}

export function addBox(
  parent: THREE.Object3D,
  name: string,
  width: number,
  height: number,
  depth: number,
  x: number,
  y: number,
  z: number,
  material: THREE.Material
): THREE.Mesh {
  const box = mesh(name, new THREE.BoxGeometry(width, height, depth), material);
  box.position.set(x, y, z);
  parent.add(box);
  return box;
}

export function mesh(name: string, geometry: THREE.BufferGeometry, material: THREE.Material): THREE.Mesh {
  const result = new THREE.Mesh(geometry, material);
  result.name = name;
  result.castShadow = true;
  result.receiveShadow = true;
  return result;
}

export function standard(color: number, roughness: number, metalness = 0): THREE.MeshStandardMaterial {
  return new THREE.MeshStandardMaterial({ color, roughness, metalness });
}

export function sceneryRandom(id: string): () => number {
  let state = stableHash(id);
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
