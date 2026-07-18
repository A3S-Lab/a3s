import { describe, expect, it } from 'vitest';
import { AIRCRAFT_BLUEPRINTS } from './aircraft-blueprints';

const EXPECTED_BLUEPRINT_IDS = ['j-50', 'j-35', 'f-35', 'f-22', 'prototype'] as const;

describe('AIRCRAFT_BLUEPRINTS', () => {
  it('contains every supported airframe exactly once', () => {
    const registeredIds = Object.keys(AIRCRAFT_BLUEPRINTS).sort();

    expect(registeredIds).toEqual([...EXPECTED_BLUEPRINT_IDS].sort());
    for (const id of EXPECTED_BLUEPRINT_IDS) expect(AIRCRAFT_BLUEPRINTS[id].id).toBe(id);
  });

  it.each([
    ['j-50', 2, 0, 'cranked-delta', true],
    ['j-35', 2, 2, 'trapezoid', false],
    ['f-35', 1, 2, 'trapezoid', false],
    ['f-22', 2, 2, 'diamond', false],
    ['prototype', 1, 1, 'forward-swept', false],
  ] as const)('describes the structural layout of %s', (id, engineCount, verticalTailCount, wing, tailless) => {
    expect(AIRCRAFT_BLUEPRINTS[id].layout).toEqual({
      engineCount,
      verticalTailCount,
      wing,
      tailless,
    });
  });

  it.each(EXPECTED_BLUEPRINT_IDS)('keeps %s layout counts aligned with its modular part arrays', (id) => {
    const blueprint = AIRCRAFT_BLUEPRINTS[id];

    expect(blueprint.propulsion.engineOffsets).toHaveLength(blueprint.layout.engineCount);
    expect(blueprint.tail.offsets).toHaveLength(blueprint.layout.verticalTailCount);
    expect(blueprint.surfaces.wing.length).toBeGreaterThanOrEqual(3);
    expect(blueprint.fuselage.sections.length).toBeGreaterThanOrEqual(2);
  });

  it.each([
    ['j-50', 'superellipse', 'caret', 'serrated-round', 'low-profile'],
    ['j-35', 'superellipse', 'dsi', 'round', 'faceted'],
    ['f-35', 'superellipse', 'dsi', 'serrated-round', 'bubble'],
    ['f-22', 'superellipse', 'box', 'rectangular', 'low-profile'],
    ['prototype', 'ellipse', 'chin', 'round', 'bubble'],
  ] as const)('defines distinctive fuselage, propulsion, and cockpit styles for %s', (id, fuselageKind, intakeStyle, nozzleStyle, cockpitStyle) => {
    const blueprint = AIRCRAFT_BLUEPRINTS[id];

    expect(blueprint.fuselage.profile.kind).toBe(fuselageKind);
    expect(blueprint.propulsion.intakeStyle).toBe(intakeStyle);
    expect(blueprint.propulsion.nozzleStyle).toBe(nozzleStyle);
    expect(blueprint.cockpit.style).toBe(cockpitStyle);
  });

  it.each(EXPECTED_BLUEPRINT_IDS)('gives %s non-empty signature details with unique IDs', (id) => {
    const detailIds = AIRCRAFT_BLUEPRINTS[id].signatureDetails.map((detail) => detail.id);

    expect(detailIds.length).toBeGreaterThan(0);
    expect(new Set(detailIds).size).toBe(detailIds.length);
  });

  it('represents the J-50 as the only tailless blueprint', () => {
    const taillessIds = Object.values(AIRCRAFT_BLUEPRINTS)
      .filter((blueprint) => blueprint.layout.tailless)
      .map((blueprint) => blueprint.id);

    expect(taillessIds).toEqual(['j-50']);
    expect(AIRCRAFT_BLUEPRINTS['j-50'].tail.offsets).toHaveLength(0);
    expect(AIRCRAFT_BLUEPRINTS['j-50'].surfaces.stabilizer).toHaveLength(0);
  });

  it('uses canards as a signature detail on the generic prototype', () => {
    expect(AIRCRAFT_BLUEPRINTS.prototype.signatureDetails.some((detail) => detail.id === 'canard')).toBe(true);
  });

  it('keeps the F-35 as the only single-engine named airframe', () => {
    const singleEngineIds = EXPECTED_BLUEPRINT_IDS.filter((id) => id !== 'prototype').filter(
      (id) => AIRCRAFT_BLUEPRINTS[id].layout.engineCount === 1
    );

    expect(singleEngineIds).toEqual(['f-35']);
  });
});
