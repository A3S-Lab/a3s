import * as THREE from 'three';
import {
  addAurora,
  addBeaconRow,
  addBox,
  addBuoys,
  addDistantHills,
  addDustField,
  addGround,
  addHorizontalPlane,
  addPineInstances,
  addWater,
  addWindTurbine,
  type BattlefieldAnimation,
  mesh,
  type SceneryContext,
  sceneryRandom,
  standard,
} from './battlefield-scenery-elements';
import type { BattlefieldTheaterProfile } from './battlefield-theater';

export interface BattlefieldScenery {
  readonly group: THREE.Group;
  readonly update: BattlefieldAnimation;
}

export function createBattlefieldScenery(profile: BattlefieldTheaterProfile): BattlefieldScenery {
  const context: SceneryContext = {
    root: new THREE.Group(),
    profile,
    animations: [],
    random: sceneryRandom(profile.id),
  };
  context.root.name = `battlefield-scenery:${profile.id}`;

  switch (profile.id) {
    case 'training-range':
      buildTrainingRange(context);
      break;
    case 'littoral-front':
      buildLittoralFront(context);
      break;
    case 'mountain-pass':
      buildMountainPass(context);
      break;
    case 'desert-frontier':
      buildDesertFrontier(context);
      break;
    case 'industrial-city':
      buildIndustrialCity(context);
      break;
    case 'arctic-highland':
      buildArcticHighland(context);
      break;
    case 'forest-valley':
      buildForestValley(context);
      break;
    case 'ocean-platforms':
      buildOceanPlatforms(context);
      break;
  }

  return {
    group: context.root,
    update: (elapsed, delta) => {
      for (const animate of context.animations) animate(elapsed, delta);
    },
  };
}

function buildTrainingRange(context: SceneryContext): void {
  const { palette } = context.profile;
  addGround(context, palette.ground);

  const runway = standard(0x303638, 0.88);
  addHorizontalPlane(context.root, 'training-runway', 48, 6.5, 0, -3.245, -8, runway);
  const marker = standard(0xe1d9bb, 0.75);
  for (let x = -19; x <= 19; x += 4.8)
    addBox(context.root, 'runway-centerline', 2.2, 0.025, 0.12, x, -3.21, -8, marker);
  for (const z of [-11, -5]) addBox(context.root, 'runway-edge', 48, 0.02, 0.05, 0, -3.205, z, marker);

  const concrete = standard(palette.structure, 0.74, 0.18);
  const dark = standard(0x293237, 0.64, 0.35);
  for (const x of [-12.5, -6.5, 7.5, 13.5]) {
    addBox(context.root, 'range-hangar', 4.3, 2, 3.1, x, -2.25, -16.5, concrete);
    addBox(context.root, 'hangar-door', 2.5, 1.25, 0.08, x, -2.38, -14.93, dark);
  }

  addBox(context.root, 'control-tower-base', 2.1, 4.4, 2.1, -16, -1.1, -11.5, concrete);
  addBox(context.root, 'control-tower-cab', 3, 1.25, 2.8, -16, 1.05, -11.5, dark);
  const radar = new THREE.Group();
  radar.name = 'range-radar';
  radar.position.set(-16, 2.05, -11.5);
  radar.add(mesh('radar-mast', new THREE.CylinderGeometry(0.08, 0.11, 1.2, 8), dark));
  const dish = mesh(
    'radar-dish',
    new THREE.SphereGeometry(0.7, 12, 6, 0, Math.PI),
    standard(palette.accent, 0.45, 0.55)
  );
  dish.scale.set(1, 0.2, 0.75);
  dish.position.y = 0.65;
  dish.rotation.x = -0.35;
  radar.add(dish);
  context.root.add(radar);
  context.animations.push((elapsed) => {
    radar.rotation.y = elapsed * 0.38;
  });

  addBeaconRow(context, -2.95, -5.1, 10);
  addDistantHills(context, 0.8, palette.rock, palette.groundSecondary);
}

function buildLittoralFront(context: SceneryContext): void {
  const { palette } = context.profile;
  addWater(context, palette.water);
  const sand = standard(palette.groundSecondary, 0.95);
  const rock = standard(palette.rock, 0.98);
  const islandPositions: ReadonlyArray<readonly [number, number, number, number]> = [
    [-10, -4.4, -17, 6],
    [9.5, -4.25, -20, 7.5],
    [-19, -4.7, -29, 10],
  ];
  for (const [x, y, z, radius] of islandPositions) {
    const island = mesh('littoral-island', new THREE.CylinderGeometry(radius * 0.72, radius, 2.8, 7), rock);
    island.position.set(x, y, z);
    context.root.add(island);
    const beach = mesh('littoral-beach', new THREE.CylinderGeometry(radius * 0.77, radius * 0.82, 0.18, 7), sand);
    beach.position.set(x, y + 1.46, z);
    context.root.add(beach);
  }

  const lighthouse = new THREE.Group();
  lighthouse.name = 'littoral-lighthouse';
  lighthouse.position.set(-9.8, -2.45, -16.8);
  const tower = mesh('lighthouse-tower', new THREE.CylinderGeometry(0.35, 0.58, 4.8, 10), standard(0xd6d2c4, 0.72));
  tower.position.y = 2.4;
  lighthouse.add(tower);
  const lantern = mesh(
    'lighthouse-lantern',
    new THREE.CylinderGeometry(0.48, 0.48, 0.55, 10),
    standard(palette.accent, 0.4, 0.25)
  );
  lantern.position.y = 5;
  lighthouse.add(lantern);
  const beam = mesh(
    'lighthouse-beam',
    new THREE.ConeGeometry(0.8, 9, 10, 1, true),
    new THREE.MeshBasicMaterial({ color: palette.accent, transparent: true, opacity: 0.09, depthWrite: false })
  );
  beam.rotation.z = -Math.PI / 2;
  beam.position.set(4.4, 5, 0);
  lighthouse.add(beam);
  context.root.add(lighthouse);
  context.animations.push((elapsed) => {
    beam.rotation.x = elapsed * 0.48;
  });

  addBuoys(context, palette.accent);
  addDistantHills(context, 0.65, palette.rock, palette.vegetation);
}

function buildMountainPass(context: SceneryContext): void {
  const { palette } = context.profile;
  addGround(context, palette.ground);
  addHorizontalPlane(context.root, 'mountain-river', 58, 2.2, 4, -3.2, -15, standard(palette.water, 0.3, 0.1));
  const rock = standard(palette.rock, 1);
  const snow = standard(0xbac7c7, 0.92);
  const mountains: ReadonlyArray<readonly [number, number, number, number]> = [
    [-14, -18, 8.5, 9],
    [-6, -24, 10, 10],
    [6, -23, 9.5, 9],
    [15, -17, 8, 8],
    [-20, -8, 6.5, 7],
    [20, -9, 6, 7],
  ];
  for (const [x, z, height, radius] of mountains) {
    const mountain = mesh('mountain-massif', new THREE.ConeGeometry(radius, height, 6), rock);
    mountain.position.set(x, -3.2 + height / 2, z);
    mountain.rotation.y = context.random() * Math.PI;
    context.root.add(mountain);
    if (height > 8) {
      const cap = mesh('mountain-snow-cap', new THREE.ConeGeometry(radius * 0.32, height * 0.28, 6), snow);
      cap.position.set(x, -3.2 + height * 0.86, z);
      cap.rotation.y = mountain.rotation.y;
      context.root.add(cap);
    }
  }
  addPineInstances(context, 34, palette.vegetation, -23, -8);
}

function buildDesertFrontier(context: SceneryContext): void {
  const { palette } = context.profile;
  addGround(context, palette.ground);
  const duneMaterial = standard(palette.groundSecondary, 1);
  for (let index = 0; index < 14; index += 1) {
    const dune = mesh('desert-dune', new THREE.SphereGeometry(1, 10, 6), duneMaterial);
    const x = -23 + context.random() * 46;
    const z = -9 - context.random() * 26;
    dune.scale.set(3.5 + context.random() * 4.5, 0.55 + context.random() * 0.55, 2.4 + context.random() * 3);
    dune.position.set(x, -3.55, z);
    dune.rotation.y = context.random() * 0.5;
    context.root.add(dune);
  }

  const mesa = standard(palette.rock, 0.98);
  for (const [x, z, scale] of [
    [-15, -20, 1.1],
    [14, -24, 1.35],
    [20, -12, 0.8],
  ] as const) {
    const formation = mesh('desert-mesa', new THREE.CylinderGeometry(3.8 * scale, 5.8 * scale, 4.5 * scale, 7), mesa);
    formation.position.set(x, -3.3 + 2.25 * scale, z);
    context.root.add(formation);
  }

  const outpost = new THREE.Group();
  outpost.name = 'desert-outpost';
  outpost.position.set(-8, -3.15, -12);
  addBox(outpost, 'outpost-module', 5, 1.3, 2.4, 0, 0.65, 0, standard(palette.structure, 0.72, 0.25));
  const antenna = mesh(
    'outpost-antenna',
    new THREE.CylinderGeometry(0.035, 0.06, 4.5, 7),
    standard(0x6b7070, 0.4, 0.8)
  );
  antenna.position.set(1.6, 3.2, 0);
  outpost.add(antenna);
  context.root.add(outpost);
  addDustField(context);
}

function buildIndustrialCity(context: SceneryContext): void {
  const { palette } = context.profile;
  addGround(context, palette.ground);
  const road = standard(0x20282b, 0.94);
  addHorizontalPlane(context.root, 'industrial-artery', 60, 5, 0, -3.235, -11, road);
  const concrete = standard(palette.structure, 0.76, 0.32);
  const glass = standard(0x29434c, 0.35, 0.7);
  for (let index = 0; index < 24; index += 1) {
    const x = -22 + (index % 12) * 4;
    const row = Math.floor(index / 12);
    const height = 2.5 + context.random() * 7;
    const building = addBox(
      context.root,
      'industrial-building',
      2.5 + context.random(),
      height,
      2.4 + context.random(),
      x,
      -3.2 + height / 2,
      -16 - row * 6,
      index % 4 === 0 ? glass : concrete
    );
    building.rotation.y = (context.random() - 0.5) * 0.08;
  }

  const stackMaterial = standard(0x596064, 0.66, 0.45);
  for (const x of [-13, 12]) {
    const stack = mesh('industrial-stack', new THREE.CylinderGeometry(0.45, 0.7, 8, 10), stackMaterial);
    stack.position.set(x, 0.8, -12);
    context.root.add(stack);
    const warning = new THREE.PointLight(palette.accent, 4, 7, 2);
    warning.name = 'industrial-warning-beacon';
    warning.position.set(x, 5, -12);
    context.root.add(warning);
    context.animations.push((elapsed) => {
      warning.intensity = 2 + Math.max(0, Math.sin(elapsed * 2.8)) * 12;
    });
  }
}

function buildArcticHighland(context: SceneryContext): void {
  const { palette } = context.profile;
  addGround(context, palette.ground);
  addHorizontalPlane(context.root, 'frozen-lake', 34, 12, 3, -3.22, -13, standard(palette.water, 0.2, 0.18));
  const mountain = standard(palette.rock, 0.94);
  const snow = standard(palette.groundSecondary, 0.96);
  for (const [x, z, height] of [
    [-17, -20, 9],
    [-7, -25, 11],
    [7, -24, 10],
    [17, -19, 8],
  ] as const) {
    const base = mesh('arctic-peak', new THREE.ConeGeometry(height * 0.76, height, 6), mountain);
    base.position.set(x, -3.2 + height / 2, z);
    context.root.add(base);
    const cap = mesh('arctic-snow-cap', new THREE.ConeGeometry(height * 0.34, height * 0.42, 6), snow);
    cap.position.set(x, -3.2 + height * 0.79, z);
    context.root.add(cap);
  }
  for (let index = 0; index < 16; index += 1) {
    const floe = mesh('ice-floe', new THREE.CircleGeometry(0.65 + context.random() * 1.4, 7), standard(0xdbe9e8, 0.75));
    floe.rotation.x = -Math.PI / 2;
    floe.rotation.z = context.random() * Math.PI;
    floe.position.set(-12 + context.random() * 29, -3.17, -8 - context.random() * 10);
    floe.scale.y = 0.5 + context.random() * 0.6;
    context.root.add(floe);
  }
  addAurora(context);
}

function buildForestValley(context: SceneryContext): void {
  const { palette } = context.profile;
  addGround(context, palette.ground);
  addHorizontalPlane(context.root, 'forest-river', 58, 2.6, 4, -3.21, -16, standard(palette.water, 0.32, 0.08));
  addDistantHills(context, 1.15, palette.rock, palette.vegetation);
  addPineInstances(context, 92, palette.vegetation, -28, -6);

  const watchtower = new THREE.Group();
  watchtower.name = 'forest-watchtower';
  watchtower.position.set(10, -3.1, -12);
  const timber = standard(0x4a4033, 0.9);
  for (const x of [-0.7, 0.7]) {
    for (const z of [-0.7, 0.7]) addBox(watchtower, 'watchtower-leg', 0.15, 4.5, 0.15, x, 2.25, z, timber);
  }
  addBox(watchtower, 'watchtower-cab', 2.3, 1.4, 2.3, 0, 4.6, 0, standard(palette.structure, 0.78));
  context.root.add(watchtower);
}

function buildOceanPlatforms(context: SceneryContext): void {
  const { palette } = context.profile;
  addWater(context, palette.water);
  const steel = standard(palette.structure, 0.48, 0.72);
  const deck = standard(0x424a4b, 0.65, 0.55);
  for (const [x, z, scale] of [
    [-11, -17, 1],
    [10, -22, 1.2],
  ] as const) {
    const platform = new THREE.Group();
    platform.name = 'offshore-platform';
    platform.position.set(x, -3.05, z);
    for (const legX of [-1.4, 1.4]) {
      for (const legZ of [-1.1, 1.1]) {
        addBox(platform, 'platform-leg', 0.18, 5 * scale, 0.18, legX * scale, 1.5 * scale, legZ * scale, steel);
      }
    }
    addBox(platform, 'platform-deck', 4.2 * scale, 0.4, 3.4 * scale, 0, 3.5 * scale, 0, deck);
    addBox(platform, 'platform-module', 2.4 * scale, 1.5 * scale, 2.1 * scale, 0, 4.45 * scale, 0, steel);
    context.root.add(platform);
  }
  for (const [x, z, scale] of [
    [-20, -24, 0.9],
    [0, -28, 1.05],
    [20, -25, 0.95],
  ] as const) {
    addWindTurbine(context, x, z, scale, steel);
  }
  addBuoys(context, palette.accent);
}
