import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import {
  createBattlefieldEnvironment,
  disposeBattlefieldEnvironment,
  setBattlefieldTheater,
  updateBattlefieldEnvironment,
} from './battlefield-environment';

describe('battlefield environment', () => {
  it.each([
    ['quick_file_edit', 'training-runway'],
    ['ann_vector_search_qps', 'littoral-lighthouse'],
    ['bipedalwalker_locomotion_rl', 'mountain-massif'],
    ['portfolio_risk_calibration', 'desert-dune'],
    ['rust_multicrate_reconstruction', 'frozen-lake'],
    ['warehouse_forklift_routing', 'industrial-building'],
    ['college_english_exam_bank', 'forest-tree-crowns'],
    ['wireless_electricity_layout', 'offshore-platform'],
  ] as const)('builds a task-specific outdoor landmark for %s', (taskId, landmark) => {
    const scene = new THREE.Scene();
    const environment = createBattlefieldEnvironment(scene, taskId);

    expect(environment.group.name).toMatch(/^battlefield-theater:/);
    expect(environment.group.getObjectByName('battlefield-sky-dome')).toBeDefined();
    expect(environment.group.getObjectByName(landmark)).toBeDefined();
    expect(environment.group.getObjectByName('three-dimensional-test-section')).toBeUndefined();
    expect(environment.group.getObjectByName('wind-tunnel-fan')).toBeUndefined();

    disposeBattlefieldEnvironment(environment);
  });

  it('replaces a theater safely and disposes the previous GPU resources', () => {
    const scene = new THREE.Scene();
    const environment = createBattlefieldEnvironment(scene, 'quick_file_edit');
    const previousGroup = environment.group;
    const previousMesh = firstMesh(previousGroup);
    const previousSun = previousGroup.getObjectByName('battlefield-sun');
    if (!(previousSun instanceof THREE.DirectionalLight)) throw new Error('Expected a battlefield sun');
    const shadowMap = new THREE.WebGLRenderTarget(2, 2);
    const shadowMapPass = new THREE.WebGLRenderTarget(2, 2);
    previousSun.shadow.map = shadowMap;
    previousSun.shadow.mapPass = shadowMapPass;
    let geometryDisposed = false;
    let materialDisposed = false;
    let shadowMapDisposed = false;
    let shadowMapPassDisposed = false;
    previousMesh.geometry.addEventListener('dispose', () => {
      geometryDisposed = true;
    });
    firstMaterial(previousMesh).addEventListener('dispose', () => {
      materialDisposed = true;
    });
    shadowMap.addEventListener('dispose', () => {
      shadowMapDisposed = true;
    });
    shadowMapPass.addEventListener('dispose', () => {
      shadowMapPassDisposed = true;
    });

    expect(setBattlefieldTheater(environment, 'ann_vector_search_qps')).toBe(true);
    expect(environment.profile.id).toBe('littoral-front');
    expect(environment.group).not.toBe(previousGroup);
    expect(previousGroup.parent).toBeNull();
    expect(environment.group.parent).toBe(scene);
    expect(geometryDisposed).toBe(true);
    expect(materialDisposed).toBe(true);
    expect(shadowMapDisposed).toBe(true);
    expect(shadowMapPassDisposed).toBe(true);
    expect(setBattlefieldTheater(environment, 'ann_vector_search_qps')).toBe(false);

    updateBattlefieldEnvironment(environment, 3, 0.016);
    disposeBattlefieldEnvironment(environment);
  });

  it('restores scene-owned atmosphere and makes disposal idempotent', () => {
    const scene = new THREE.Scene();
    const originalBackground = new THREE.Color(0x123456);
    const originalFog = new THREE.FogExp2(0x654321, 0.02);
    scene.background = originalBackground;
    scene.fog = originalFog;
    const environment = createBattlefieldEnvironment(scene, 'rust_multicrate_reconstruction');

    expect(scene.background).not.toBe(originalBackground);
    expect(originalFog.color.getHex()).not.toBe(0x654321);
    disposeBattlefieldEnvironment(environment);
    disposeBattlefieldEnvironment(environment);

    expect(environment.disposed).toBe(true);
    expect(environment.group.parent).toBeNull();
    expect(scene.background).toBe(originalBackground);
    expect(scene.fog).toBe(originalFog);
    expect(originalFog.color.getHex()).toBe(0x654321);
    expect(setBattlefieldTheater(environment, 'quick_file_edit')).toBe(false);
  });

  it('re-resolves an unknown Task when its category changes', () => {
    const scene = new THREE.Scene();
    const environment = createBattlefieldEnvironment(scene, 'future_bench_task', 'Systems & Software Engineering');
    expect(environment.profile.id).toBe('industrial-city');

    expect(setBattlefieldTheater(environment, 'future_bench_task', 'Professional Knowledge Work')).toBe(true);
    expect(environment.profile.id).toBe('forest-valley');
    disposeBattlefieldEnvironment(environment);
  });
});

function firstMesh(root: THREE.Object3D): THREE.Mesh {
  let result: THREE.Mesh | undefined;
  root.traverse((object) => {
    if (!result && object instanceof THREE.Mesh) result = object;
  });
  if (!result) throw new Error('Expected the battlefield to contain at least one mesh');
  return result;
}

function firstMaterial(mesh: THREE.Mesh): THREE.Material {
  return Array.isArray(mesh.material) ? mesh.material[0] : mesh.material;
}
