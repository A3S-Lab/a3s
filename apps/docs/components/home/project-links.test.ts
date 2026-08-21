import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { getProjectPrimaryHref, getProjectRepositoryHref } from './project-links';

describe('project links', () => {
  test('opens the Power website and links its source repository', () => {
    assert.equal(getProjectPrimaryHref('power'), 'https://a3s-lab.github.io/Power/');
    assert.equal(getProjectRepositoryHref('power'), 'https://github.com/A3S-Lab/Power');
  });

  test('opens the Site project at the ecosystem homepage', () => {
    assert.equal(getProjectPrimaryHref('site'), '/');
    assert.equal(getProjectRepositoryHref('site'), 'https://github.com/A3S-Lab/a3s/tree/main/apps/docs');
  });
});
