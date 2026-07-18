import type * as THREE from 'three';
import { assembleAircraft } from './aircraft-assembler';
import type { AircraftAssemblyOptions, BuiltInAircraftId } from './aircraft-blueprint';
import { AIRCRAFT_BLUEPRINTS } from './aircraft-blueprints';

export type ProceduralFighterVariant = BuiltInAircraftId;
export type ProceduralFighterOptions = AircraftAssemblyOptions;

export const PROCEDURAL_FIGHTER_VARIANTS = AIRCRAFT_BLUEPRINTS;

export function createProceduralFighter(
  variant: ProceduralFighterVariant,
  options: ProceduralFighterOptions = {}
): THREE.Group {
  return assembleAircraft(AIRCRAFT_BLUEPRINTS[variant], options);
}
