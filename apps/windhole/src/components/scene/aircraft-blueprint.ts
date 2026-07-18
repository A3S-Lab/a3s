import type * as THREE from 'three';
import type { AircraftLivery } from './aircraft-livery';
import type { PilotProfile } from './pilot-profile';
import type { WeaponLoadout } from './weapon-loadout';

export type BuiltInAircraftId = 'j-50' | 'j-35' | 'f-35' | 'f-22' | 'prototype';
export type WingPlanform = 'cranked-delta' | 'diamond' | 'trapezoid' | 'forward-swept';
export type IntakeStyle = 'caret' | 'dsi' | 'box' | 'chin';
export type NozzleStyle = 'round' | 'serrated-round' | 'rectangular';
export type CanopyStyle = 'low-profile' | 'faceted' | 'bubble';
export type MaterialRole = 'skin' | 'upperSkin' | 'panel' | 'intake' | 'nozzle' | 'accent' | 'secondary';
export type PlanPoint = readonly [x: number, z: number];
export type Vector3Tuple = readonly [x: number, y: number, z: number];

export interface LoftSection {
  x: number;
  height: number;
  width: number;
  centerY?: number;
}

export type FuselageProfile =
  | { kind: 'ellipse'; segments: number }
  | { kind: 'superellipse'; segments: number; exponent: number };

interface SignatureDetailBase {
  id: string;
  material: MaterialRole;
  mirrored?: boolean;
}

export type SignatureDetail =
  | (SignatureDetailBase & {
      kind: 'box';
      position: Vector3Tuple;
      rotation?: Vector3Tuple;
      size: Vector3Tuple;
    })
  | (SignatureDetailBase & {
      kind: 'ellipsoid';
      position: Vector3Tuple;
      scale: Vector3Tuple;
    })
  | (SignatureDetailBase & {
      kind: 'planform';
      elevation: number;
      points: readonly PlanPoint[];
      thickness: number;
    });

export interface AircraftLayout {
  engineCount: number;
  verticalTailCount: number;
  wing: WingPlanform;
  tailless: boolean;
}

export interface AircraftBlueprint<Id extends string = string> {
  id: Id;
  displayName: string;
  baseColor: THREE.ColorRepresentation;
  accentColor: THREE.ColorRepresentation;
  layout: AircraftLayout;
  fuselage: {
    profile: FuselageProfile;
    sections: readonly LoftSection[];
  };
  surfaces: {
    chine: readonly PlanPoint[];
    wing: readonly PlanPoint[];
    stabilizer: readonly PlanPoint[];
    verticalTail: readonly PlanPoint[];
  };
  propulsion: {
    engineOffsets: readonly number[];
    engineRadius: number;
    intakeOffset: number;
    intakeStyle: IntakeStyle;
    nozzleStyle: NozzleStyle;
    nacellePosition: readonly [x: number, y: number];
    nacelleLength: number;
    nozzleX: number;
    exhaustX: number;
  };
  tail: {
    offsets: readonly number[];
    cant: number;
  };
  cockpit: {
    position: readonly [x: number, y: number];
    scale: readonly [x: number, y: number, z: number];
    style: CanopyStyle;
  };
  signatureDetails: readonly SignatureDetail[];
}

export interface AircraftAssemblyOptions {
  accentColor?: THREE.ColorRepresentation;
  baseColor?: THREE.ColorRepresentation;
  callsign?: string;
  livery?: AircraftLivery;
  pilotProfile?: PilotProfile;
  secondaryColor?: THREE.ColorRepresentation;
  weaponLoadout?: WeaponLoadout;
}

type AircraftBlueprintInput<Id extends string> = Omit<AircraftBlueprint<Id>, 'layout'> & {
  layout: Pick<AircraftLayout, 'wing' | 'tailless'>;
};

export function defineAircraftBlueprint<const Id extends string>(
  input: AircraftBlueprintInput<Id>
): AircraftBlueprint<Id> {
  const layout: AircraftLayout = Object.freeze({
    ...input.layout,
    engineCount: input.propulsion.engineOffsets.length,
    verticalTailCount: input.tail.offsets.length,
  });

  if (layout.tailless !== (layout.verticalTailCount === 0)) {
    throw new Error(`Aircraft blueprint ${input.id} has an inconsistent tailless layout`);
  }
  if (layout.engineCount === 0) {
    throw new Error(`Aircraft blueprint ${input.id} requires at least one engine`);
  }
  if (input.fuselage.sections.length < 2 || input.surfaces.wing.length < 3) {
    throw new Error(`Aircraft blueprint ${input.id} requires a fuselage and wing planform`);
  }
  if (layout.verticalTailCount > 0 && input.surfaces.verticalTail.length < 3) {
    throw new Error(`Aircraft blueprint ${input.id} requires a vertical-tail planform`);
  }
  if (input.signatureDetails.length === 0) {
    throw new Error(`Aircraft blueprint ${input.id} requires a visible signature detail`);
  }
  const detailIds = input.signatureDetails.map((detail) => detail.id);
  if (new Set(detailIds).size !== detailIds.length) {
    throw new Error(`Aircraft blueprint ${input.id} has duplicate signature detail ids`);
  }

  return Object.freeze({ ...input, layout });
}
