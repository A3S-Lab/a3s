import type { EvaluationEffort } from '../../types/evaluation';

export type ReasoningEffort = EvaluationEffort;
export type WeaponLoadoutId =
  | 'clean'
  | 'self-defense'
  | 'light-air-to-air'
  | 'balanced-air-to-air'
  | 'heavy-air-to-air'
  | 'full-combat';
export type WeaponStoreKind = 'short-range-aam' | 'medium-range-aam';
export type WeaponPlacement = 'internal' | 'external';

export interface WeaponStore {
  kind: WeaponStoreKind;
  quantity: number;
  placement: WeaponPlacement;
}

export interface WeaponLoadout {
  id: WeaponLoadoutId;
  displayName: string;
  effort: ReasoningEffort;
  visualizationOnly: true;
  stores: readonly WeaponStore[];
  totalStores: number;
}

export const WEAPON_LOADOUTS = Object.freeze({
  clean: defineLoadout('clean', 'Clean configuration', 'none', []),
  selfDefense: defineLoadout('self-defense', 'Self-defense', 'minimal', [
    { kind: 'short-range-aam', quantity: 2, placement: 'internal' },
  ]),
  lightAirToAir: defineLoadout('light-air-to-air', 'Light air-to-air', 'low', [
    { kind: 'medium-range-aam', quantity: 2, placement: 'internal' },
  ]),
  balancedAirToAir: defineLoadout('balanced-air-to-air', 'Balanced air-to-air', 'medium', [
    { kind: 'medium-range-aam', quantity: 4, placement: 'internal' },
  ]),
  heavyAirToAir: defineLoadout('heavy-air-to-air', 'Heavy air-to-air', 'high', [
    { kind: 'medium-range-aam', quantity: 4, placement: 'internal' },
    { kind: 'short-range-aam', quantity: 2, placement: 'external' },
  ]),
  fullCombat: defineLoadout('full-combat', 'Full combat load', 'xhigh', [
    { kind: 'medium-range-aam', quantity: 6, placement: 'internal' },
    { kind: 'short-range-aam', quantity: 2, placement: 'external' },
  ]),
});

export const EFFORT_LOADOUT_MAP: Readonly<Record<ReasoningEffort, WeaponLoadout>> = Object.freeze({
  none: WEAPON_LOADOUTS.clean,
  minimal: WEAPON_LOADOUTS.selfDefense,
  low: WEAPON_LOADOUTS.lightAirToAir,
  medium: WEAPON_LOADOUTS.balancedAirToAir,
  high: WEAPON_LOADOUTS.heavyAirToAir,
  xhigh: WEAPON_LOADOUTS.fullCombat,
});

export function resolveWeaponLoadout(effort?: string | null): WeaponLoadout {
  return EFFORT_LOADOUT_MAP[normalizeReasoningEffort(effort)];
}

export function normalizeReasoningEffort(effort?: string | null): ReasoningEffort {
  const normalized = (effort ?? '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-');

  if (normalized === 'none' || normalized === 'off' || normalized === 'disabled') return 'none';
  if (normalized === 'minimal' || normalized === 'min') return 'minimal';
  if (normalized === 'low') return 'low';
  if (normalized === 'high') return 'high';
  if (['xhigh', 'x-high', 'extra-high', 'very-high', 'maximum', 'max'].includes(normalized)) return 'xhigh';
  return 'medium';
}

function defineLoadout(
  id: WeaponLoadoutId,
  displayName: string,
  effort: ReasoningEffort,
  stores: readonly WeaponStore[]
): WeaponLoadout {
  const immutableStores = Object.freeze(stores.map((store) => Object.freeze({ ...store })));
  return Object.freeze({
    id,
    displayName,
    effort,
    visualizationOnly: true as const,
    stores: immutableStores,
    totalStores: immutableStores.reduce((total, store) => total + store.quantity, 0),
  });
}
