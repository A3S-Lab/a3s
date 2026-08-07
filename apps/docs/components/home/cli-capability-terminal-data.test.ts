import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { cliCapabilityScenes } from './cli-capability-terminal-data';

describe('A3S CLI capability terminal', () => {
  test('covers every public capability group shown in the hero', () => {
    assert.deepEqual(
      cliCapabilityScenes.map((scene) => scene.id),
      ['code', 'web', 'box', 'bench', 'search', 'use', 'observe', 'manage'],
    );
  });

  test('uses concrete localized commands and complete output rows', () => {
    for (const scene of cliCapabilityScenes) {
      assert.equal(scene.command.cn.startsWith('a3s '), true);
      assert.equal(scene.command.en.startsWith('a3s '), true);
      assert.equal(scene.output.length, 3);

      for (const row of scene.output) {
        assert.equal(row.value.cn.length > 0, true);
        assert.equal(row.value.en.length > 0, true);
      }
    }
  });
});
