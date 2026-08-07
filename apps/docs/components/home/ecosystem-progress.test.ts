import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { architectureProjects } from './architecture';
import { getProjectProgress, progressProjectIds } from './ecosystem-progress';

describe('ecosystem development progress', () => {
  test('covers every project in the ecosystem directory', () => {
    assert.deepEqual(
      [...progressProjectIds].sort(),
      architectureProjects.map((project) => project.id).sort(),
    );
  });

  test('uses bounded, localized lifecycle stages', () => {
    for (const project of architectureProjects) {
      const chinese = getProjectProgress(project.id, 'cn');
      const english = getProjectProgress(project.id, 'en');

      assert.ok(chinese.value >= 0 && chinese.value <= 100);
      assert.equal(chinese.value, english.value);
      assert.notEqual(chinese.label, english.label);
    }
  });
});
