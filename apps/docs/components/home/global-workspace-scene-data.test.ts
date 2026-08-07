import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, test } from 'node:test';
import {
  organizationNodes,
  workspaceSceneCopy,
} from './global-workspace-scene-data';

describe('global workspace scene', () => {
  test('distributes collaboration nodes across the inhabited world', () => {
    assert.deepEqual(
      [...new Set(organizationNodes.map((node) => node.region))].sort(),
      ['africa', 'americas', 'asia', 'europe', 'oceania'],
    );

    const longitudes = organizationNodes.map((node) => node.longitude);
    const latitudes = organizationNodes.map((node) => node.latitude);

    assert.equal(Math.max(...longitudes) - Math.min(...longitudes) > 260, true);
    assert.equal(Math.min(...latitudes) < 0, true);
    assert.equal(Math.max(...latitudes) > 0, true);
    assert.equal(new Set(organizationNodes.map((node) => node.code)).size, organizationNodes.length);

    for (const node of organizationNodes) {
      assert.equal(node.phase >= 0 && node.phase < 1, true);
    }
  });

  test('keeps the complete organization model explicit in both languages', () => {
    for (const lang of ['cn', 'en'] as const) {
      const copy = workspaceSceneCopy[lang];
      const completeCopy = Object.values(copy).join(' ');

      for (const concept of ['Human', 'Agent', 'Shared Workspace', 'A3S OS', 'Edge', 'Cloud']) {
        assert.equal(completeCopy.toLowerCase().includes(concept.toLowerCase()), true);
      }

      assert.equal(copy.handoffSteps.length, 3);
      assert.equal(copy.handoffSteps[0].includes('Human'), true);
      assert.equal(copy.handoffSteps[1].includes('Agent'), true);
      assert.equal(copy.handoffSteps[2].includes('Edge'), true);
      assert.equal(copy.handoffSteps[2].includes('Cloud'), true);
    }
  });

  test('keeps the raster A3S logo inside the workspace app bar', () => {
    const styles = readFileSync(
      new URL('./styles/global-workspace-scene.css', import.meta.url),
      'utf8',
    );

    assert.equal(styles.includes('.a3s-global-workspace__brand img {'), true);
    assert.equal(styles.includes('.a3s-global-workspace__brand svg {'), false);
  });
});
