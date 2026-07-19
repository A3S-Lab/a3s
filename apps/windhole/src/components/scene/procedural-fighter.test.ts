import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import { createProceduralFighter, PROCEDURAL_FIGHTER_VARIANTS } from './procedural-fighter';

describe('createProceduralFighter', () => {
  it.each([
    ['j-50', 2, 0],
    ['j-35', 2, 2],
    ['f-35', 1, 2],
    ['f-22', 2, 2],
    ['prototype', 1, 1],
  ] as const)('builds a complete %s airframe', (variant, engineCount, tailCount) => {
    const fighter = createProceduralFighter(variant);
    const engines = fighter.children.filter((child) => child.name.startsWith('engine-nozzle:'));
    const tails = fighter.children.filter((child) => child.name.startsWith('vertical-tail:'));

    expect(fighter.name).toBe(`aircraft:${variant}`);
    expect(fighter.userData.aircraftModelId).toBe(variant);
    expect(engines).toHaveLength(engineCount);
    expect(tails).toHaveLength(tailCount);
    expect(fighter.getObjectByName('fuselage')).toBeInstanceOf(THREE.Mesh);
    expect(fighter.getObjectByName('main-wing:left')).toBeInstanceOf(THREE.Mesh);
    expect(fighter.getObjectByName('main-wing:right')).toBeInstanceOf(THREE.Mesh);
  });

  it('creates finite, scene-ready geometry with shadows enabled', () => {
    const fighter = createProceduralFighter('j-35');
    const bounds = new THREE.Box3().setFromObject(fighter);
    const meshes: THREE.Mesh[] = [];
    fighter.traverse((object) => {
      if (object instanceof THREE.Mesh) meshes.push(object);
    });

    expect(bounds.isEmpty()).toBe(false);
    expect(bounds.min.toArray().every(Number.isFinite)).toBe(true);
    expect(bounds.max.toArray().every(Number.isFinite)).toBe(true);
    expect(meshes.length).toBeGreaterThan(10);
    expect(meshes.every((mesh) => mesh.castShadow && mesh.receiveShadow)).toBe(true);
  });

  it('supports a per-instance accent without changing shared variant definitions', () => {
    const fighter = createProceduralFighter('f-22', { accentColor: '#ff00aa', callsign: 'CLAUDE-01' });
    const marker = fighter.getObjectByName('identity-marker') as THREE.Mesh;
    const material = marker.material as THREE.MeshStandardMaterial;

    expect(material.color.getHexString()).toBe('ff00aa');
    expect(fighter.userData.callsign).toBe('CLAUDE-01');
    expect(PROCEDURAL_FIGHTER_VARIANTS['f-22'].accentColor).not.toBe('#ff00aa');
  });

  it.each(['a3s', 'codex', 'claude', 'generic'] as const)('adds a visible %s identity livery', (livery) => {
    const fighter = createProceduralFighter('prototype', { livery });

    expect(fighter.userData.livery).toBe(livery);
    expect(fighter.getObjectByName(`brand-livery:${livery}`)?.children.length).toBeGreaterThan(0);
  });
});
