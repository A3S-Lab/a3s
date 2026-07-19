import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import { createPilotFigure } from './pilot-figure';
import { PILOT_PROFILES } from './pilot-profile';

describe('pilot figure', () => {
  it.each([
    ['a3s-code', 'spectrum-bars'],
    ['codex', 'orbit-ring'],
    ['claude-code', 'triple-ray'],
    ['unknown', 'test-chevron'],
  ] as const)('renders distinct %s flight attire', (candidateFamily, pattern) => {
    const profile = PILOT_PROFILES[candidateFamily];
    const figure = createPilotFigure(profile);
    const flightSuit = figure.getObjectByName('pilot-flight-suit') as THREE.Mesh;
    const material = flightSuit.material as THREE.MeshStandardMaterial;

    expect(figure.name).toBe(`pilot:${profile.id}`);
    expect(figure.getObjectByName(`pilot-marking:${pattern}`)).toBeInstanceOf(THREE.Group);
    expect(material.color.getHex()).toBe(profile.attire.flightSuit);
    expect(figure.userData.pilot.helmetCode).toBe(profile.marking.helmetCode);
  });
});
