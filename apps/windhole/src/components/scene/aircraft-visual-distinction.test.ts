import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import { assembleAircraft } from './aircraft-assembler';
import { AIRCRAFT_BLUEPRINTS } from './aircraft-blueprints';

const AIRCRAFT_IDS = ['j-50', 'j-35', 'f-35', 'f-22', 'prototype'] as const;
const INTAKE_STYLES = ['caret', 'dsi', 'box', 'chin'] as const;
const NOZZLE_STYLES = ['round', 'serrated-round', 'rectangular'] as const;

describe('aircraft visual distinction', () => {
  it.each(AIRCRAFT_IDS)('gives %s one finite, visible signature feature', (id) => {
    const aircraft = assembleAircraft(AIRCRAFT_BLUEPRINTS[id]);
    const signatures: THREE.Object3D[] = [];
    aircraft.traverse((object) => {
      if (object.name.startsWith('airframe-signature:')) signatures.push(object);
    });

    expect(signatures).toHaveLength(1);
    const signature = signatures[0];
    expect(signature).toBeInstanceOf(THREE.Group);
    expect(signature.name).toBe(`airframe-signature:${id}`);

    const visibleMeshes: THREE.Mesh[] = [];
    signature.traverse((object) => {
      if (object instanceof THREE.Mesh && object.visible) visibleMeshes.push(object);
    });
    const bounds = new THREE.Box3().setFromObject(signature);

    expect(signature.visible).toBe(true);
    expect(visibleMeshes.length).toBeGreaterThan(0);
    expect(bounds.isEmpty()).toBe(false);
    expect([...bounds.min.toArray(), ...bounds.max.toArray()].every(Number.isFinite)).toBe(true);
  });

  it('declares the supported intake and nozzle styles with real fleet diversity', () => {
    const intakeStyles = AIRCRAFT_IDS.map((id) => AIRCRAFT_BLUEPRINTS[id].propulsion.intakeStyle);
    const nozzleStyles = AIRCRAFT_IDS.map((id) => AIRCRAFT_BLUEPRINTS[id].propulsion.nozzleStyle);

    for (const style of intakeStyles) expect(INTAKE_STYLES).toContain(style);
    for (const style of nozzleStyles) expect(NOZZLE_STYLES).toContain(style);
    expect(new Set(intakeStyles).size).toBe(INTAKE_STYLES.length);
    expect(new Set(nozzleStyles).size).toBe(NOZZLE_STYLES.length);
  });

  it('keeps every assembled wing outline fingerprint unique', () => {
    const fingerprints = AIRCRAFT_IDS.map((id) => {
      const aircraft = assembleAircraft(AIRCRAFT_BLUEPRINTS[id]);
      return wingFingerprint(aircraft);
    });

    expect(new Set(fingerprints).size).toBe(AIRCRAFT_IDS.length);
  });

  it('keeps normalized silhouettes separated without locking absolute dimensions', () => {
    const silhouettes = AIRCRAFT_IDS.map((id) => {
      const aircraft = assembleAircraft(AIRCRAFT_BLUEPRINTS[id]);
      const size = new THREE.Box3().setFromObject(aircraft).getSize(new THREE.Vector3());
      return { id, value: [size.z / size.x, size.y / size.x] as const };
    });

    for (let leftIndex = 0; leftIndex < silhouettes.length; leftIndex += 1) {
      for (let rightIndex = leftIndex + 1; rightIndex < silhouettes.length; rightIndex += 1) {
        const left = silhouettes[leftIndex];
        const right = silhouettes[rightIndex];
        const distance = Math.hypot(left.value[0] - right.value[0], left.value[1] - right.value[1]);

        expect(distance, `${left.id} and ${right.id} silhouettes are too similar`).toBeGreaterThan(0.04);
      }
    }
  });

  it.each(AIRCRAFT_IDS)('assembles the %s engine and tail structure described by its blueprint', (id) => {
    const blueprint = AIRCRAFT_BLUEPRINTS[id];
    const aircraft = assembleAircraft(blueprint);
    const nacelles = namedDescendants(aircraft, 'engine-nacelle:');
    const nozzles = namedDescendants(aircraft, 'engine-nozzle:');
    const tails = namedDescendants(aircraft, 'vertical-tail:');

    expect(nacelles).toHaveLength(blueprint.layout.engineCount);
    expect(nozzles).toHaveLength(blueprint.layout.engineCount);
    expect(tails).toHaveLength(blueprint.layout.verticalTailCount);
    expect(sidePattern(nozzles)).toEqual(sidePatternFromOffsets(blueprint.propulsion.engineOffsets));
    expect(sidePattern(tails)).toEqual(sidePatternFromOffsets(blueprint.tail.offsets));
  });
});

function wingFingerprint(aircraft: THREE.Group): string {
  const wing = aircraft.getObjectByName('main-wing:right');
  if (!(wing instanceof THREE.Mesh) || !(wing.geometry instanceof THREE.BufferGeometry)) {
    throw new Error(`${aircraft.name} is missing its right main-wing geometry`);
  }

  const positions = wing.geometry.getAttribute('position');
  let minimumX = Number.POSITIVE_INFINITY;
  let maximumX = Number.NEGATIVE_INFINITY;
  let minimumZ = Number.POSITIVE_INFINITY;
  let maximumZ = Number.NEGATIVE_INFINITY;
  for (let index = 0; index < positions.count; index += 1) {
    minimumX = Math.min(minimumX, positions.getX(index));
    maximumX = Math.max(maximumX, positions.getX(index));
    minimumZ = Math.min(minimumZ, positions.getZ(index));
    maximumZ = Math.max(maximumZ, positions.getZ(index));
  }

  const width = maximumX - minimumX;
  const span = maximumZ - minimumZ;
  if (!(width > 0) || !(span > 0)) throw new Error(`${aircraft.name} has a degenerate main-wing outline`);

  const points = new Set<string>();
  for (let index = 0; index < positions.count; index += 1) {
    const normalizedX = (positions.getX(index) - minimumX) / width;
    const normalizedZ = (positions.getZ(index) - minimumZ) / span;
    points.add(`${quantize(normalizedX)}:${quantize(normalizedZ)}`);
  }
  return [...points].sort().join('|');
}

function quantize(value: number): number {
  return Math.round(value * 20);
}

function namedDescendants(root: THREE.Object3D, prefix: string): THREE.Object3D[] {
  const matches: THREE.Object3D[] = [];
  root.traverse((object) => {
    if (object.name.startsWith(prefix)) matches.push(object);
  });
  return matches;
}

function sidePattern(objects: readonly THREE.Object3D[]): number[] {
  const position = new THREE.Vector3();
  return objects.map((object) => side(object.getWorldPosition(position).z)).sort((left, right) => left - right);
}

function sidePatternFromOffsets(offsets: readonly number[]): number[] {
  return offsets.map(side).sort((left, right) => left - right);
}

function side(value: number): number {
  if (value < -1e-6) return -1;
  if (value > 1e-6) return 1;
  return 0;
}
