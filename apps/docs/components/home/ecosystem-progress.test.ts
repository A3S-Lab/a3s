import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { architectureProjects } from './architecture';
import { getProjectProgress, progressProjectIds, progressVerifiedAt } from './ecosystem-progress';

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

      assert.ok([25, 50, 75, 100].includes(chinese.position));
      assert.equal(chinese.position, english.position);
      assert.equal(chinese.release, english.release);
      assert.ok(chinese.release.length > 0);
      assert.notEqual(chinese.label, english.label);
    }
  });

  test('records the current release snapshot and delivered projects', () => {
    assert.equal(progressVerifiedAt, '2026-08-09');
    assert.equal(getProjectProgress('cli', 'en').release, 'v0.10.14');
    assert.equal(getProjectProgress('flow', 'en').release, 'v0.11.0');
    assert.equal(getProjectProgress('form', 'en').stage, 'preview');
    assert.equal(getProjectProgress('windhole', 'en').stage, 'preview');
    assert.equal(getProjectProgress('gui', 'en').stage, 'preview');
    assert.equal(getProjectProgress('updater', 'en').stage, 'released');
    assert.equal(getProjectProgress('updater', 'en').release, 'v0.3.0');
  });
});
