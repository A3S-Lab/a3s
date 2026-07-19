export type BattlefieldTheaterId =
  | 'training-range'
  | 'littoral-front'
  | 'mountain-pass'
  | 'desert-frontier'
  | 'industrial-city'
  | 'arctic-highland'
  | 'forest-valley'
  | 'ocean-platforms';

export interface BattlefieldPalette {
  readonly skyZenith: number;
  readonly skyHorizon: number;
  readonly fog: number;
  readonly ground: number;
  readonly groundSecondary: number;
  readonly rock: number;
  readonly vegetation: number;
  readonly structure: number;
  readonly accent: number;
  readonly water: number;
}

export interface BattlefieldLightingProfile {
  readonly sky: number;
  readonly ground: number;
  readonly sun: number;
  readonly sunIntensity: number;
  readonly sunPosition: readonly [x: number, y: number, z: number];
}

export interface BattlefieldTheaterProfile {
  readonly id: BattlefieldTheaterId;
  readonly label: string;
  readonly labelZh: string;
  readonly palette: BattlefieldPalette;
  readonly lighting: BattlefieldLightingProfile;
}

export const BATTLEFIELD_THEATERS: Readonly<Record<BattlefieldTheaterId, BattlefieldTheaterProfile>> = Object.freeze({
  'training-range': theater(
    'training-range',
    'Falcon Training Range',
    '鹰隘训练基地',
    {
      skyZenith: 0x397eac,
      skyHorizon: 0xc0d4ca,
      fog: 0x8da9a5,
      ground: 0x5c694a,
      groundSecondary: 0x8b8664,
      rock: 0x6f6f61,
      vegetation: 0x344d31,
      structure: 0x59666b,
      accent: 0xe3b759,
      water: 0x477889,
    },
    { sky: 0xbfe4f5, ground: 0x354532, sun: 0xfff0c2, sunIntensity: 3.2, sunPosition: [-14, 17, 9] }
  ),
  'littoral-front': theater(
    'littoral-front',
    'Emerald Littoral',
    '翡翠海岸',
    {
      skyZenith: 0x286994,
      skyHorizon: 0xb2d3d1,
      fog: 0x86aaaa,
      ground: 0x86785a,
      groundSecondary: 0xc0ad78,
      rock: 0x626c67,
      vegetation: 0x275848,
      structure: 0x68777a,
      accent: 0xf3d58b,
      water: 0x1c6878,
    },
    { sky: 0xb9e6f2, ground: 0x23403c, sun: 0xffe4af, sunIntensity: 3, sunPosition: [-16, 13, 7] }
  ),
  'mountain-pass': theater(
    'mountain-pass',
    'Titan Mountain Pass',
    '巨神山口',
    {
      skyZenith: 0x315a7b,
      skyHorizon: 0xb0b8ad,
      fog: 0x818a83,
      ground: 0x45483f,
      groundSecondary: 0x626151,
      rock: 0x4b4d49,
      vegetation: 0x293b2c,
      structure: 0x626866,
      accent: 0xcc8a58,
      water: 0x3c6571,
    },
    { sky: 0xaac5d5, ground: 0x262b27, sun: 0xffd3a0, sunIntensity: 2.65, sunPosition: [-11, 16, 4] }
  ),
  'desert-frontier': theater(
    'desert-frontier',
    'Sirocco Frontier',
    '热风荒漠',
    {
      skyZenith: 0x4b7893,
      skyHorizon: 0xe3c69a,
      fog: 0xc5a87c,
      ground: 0xa2774e,
      groundSecondary: 0xc3955e,
      rock: 0x7d543d,
      vegetation: 0x626342,
      structure: 0x736b5c,
      accent: 0xf1b358,
      water: 0x477b82,
    },
    { sky: 0xffdcb5, ground: 0x5a3925, sun: 0xffd092, sunIntensity: 3.7, sunPosition: [-13, 16, 5] }
  ),
  'industrial-city': theater(
    'industrial-city',
    'Iron Grid District',
    '钢铁网格城',
    {
      skyZenith: 0x253c4c,
      skyHorizon: 0x798789,
      fog: 0x59686b,
      ground: 0x303739,
      groundSecondary: 0x454b4b,
      rock: 0x4f5656,
      vegetation: 0x34463b,
      structure: 0x414d53,
      accent: 0xe2893b,
      water: 0x385b63,
    },
    { sky: 0x93acb6, ground: 0x1c2325, sun: 0xffc07a, sunIntensity: 2.5, sunPosition: [-8, 14, 2] }
  ),
  'arctic-highland': theater(
    'arctic-highland',
    'Borealis Highland',
    '极光高原',
    {
      skyZenith: 0x355978,
      skyHorizon: 0xc6d8dc,
      fog: 0xa8bdc1,
      ground: 0xd4dfdd,
      groundSecondary: 0x9fb5b8,
      rock: 0x65757c,
      vegetation: 0x294346,
      structure: 0x67777f,
      accent: 0x6ee2dd,
      water: 0x3d758a,
    },
    { sky: 0xd6f2fa, ground: 0x56666d, sun: 0xe9f8ff, sunIntensity: 2.8, sunPosition: [-15, 10, 4] }
  ),
  'forest-valley': theater(
    'forest-valley',
    'Cedar Shadow Valley',
    '雪松影谷',
    {
      skyZenith: 0x315f70,
      skyHorizon: 0xa6bda1,
      fog: 0x728c78,
      ground: 0x354735,
      groundSecondary: 0x526342,
      rock: 0x50584e,
      vegetation: 0x173d2c,
      structure: 0x59625b,
      accent: 0xd6a65a,
      water: 0x315f66,
    },
    { sky: 0xb5d8d6, ground: 0x17281e, sun: 0xffdea8, sunIntensity: 2.7, sunPosition: [-16, 14, 8] }
  ),
  'ocean-platforms': theater(
    'ocean-platforms',
    'Tempest Offshore Array',
    '风暴离岸阵列',
    {
      skyZenith: 0x1e3d54,
      skyHorizon: 0x657d82,
      fog: 0x526c73,
      ground: 0x4d5552,
      groundSecondary: 0x6a746d,
      rock: 0x4f5d5e,
      vegetation: 0x24443e,
      structure: 0x53636a,
      accent: 0xf0a74f,
      water: 0x174958,
    },
    { sky: 0x829eaa, ground: 0x182d32, sun: 0xbfd7df, sunIntensity: 2.2, sunPosition: [-12, 12, 3] }
  ),
});

const TASK_THEATERS: Readonly<Record<string, BattlefieldTheaterId>> = Object.freeze({
  quick_file_edit: 'training-range',
  ann_vector_search_qps: 'littoral-front',
  bipedalwalker_locomotion_rl: 'mountain-pass',
  portfolio_risk_calibration: 'desert-frontier',
  rust_multicrate_reconstruction: 'arctic-highland',
  warehouse_forklift_routing: 'industrial-city',
  college_english_exam_bank: 'forest-valley',
  wireless_electricity_layout: 'ocean-platforms',
});

const CATEGORY_THEATERS: Readonly<Record<string, BattlefieldTheaterId>> = Object.freeze({
  conformance: 'training-range',
  'systems & software engineering': 'industrial-city',
  'scientific problems & ml': 'mountain-pass',
  'combinatorial optimization': 'desert-frontier',
  'professional knowledge work': 'forest-valley',
});

const THEATER_SEQUENCE = Object.values(BATTLEFIELD_THEATERS);

export function taskBattlefieldTheater(taskId: string, category?: string): BattlefieldTheaterProfile {
  const known = TASK_THEATERS[taskId];
  if (known) return BATTLEFIELD_THEATERS[known];

  const categoryTheater = category ? CATEGORY_THEATERS[category.trim().toLowerCase()] : undefined;
  if (categoryTheater) return BATTLEFIELD_THEATERS[categoryTheater];

  return THEATER_SEQUENCE[stableHash(taskId) % THEATER_SEQUENCE.length];
}

/** Compact public resolver used by map cards and HUD overlays. */
export const taskTheater = taskBattlefieldTheater;

function theater(
  id: BattlefieldTheaterId,
  label: string,
  labelZh: string,
  palette: BattlefieldPalette,
  lighting: BattlefieldLightingProfile
): BattlefieldTheaterProfile {
  return Object.freeze({
    id,
    label,
    labelZh,
    palette: Object.freeze(palette),
    lighting: Object.freeze(lighting),
  });
}

function stableHash(value: string): number {
  let hash = 2166136261;
  for (const character of value) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}
