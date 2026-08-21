import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { getProjectPrimaryHref, getProjectRepositoryHref } from './project-links';

describe('project links', () => {
  test('opens the published Form playground and links source to UI', () => {
    assert.equal(getProjectPrimaryHref('form'), '/form/');
    assert.equal(
      getProjectRepositoryHref('form'),
      'https://github.com/A3S-Lab/UI/tree/main/modules/form',
    );
  });

  test('opens the Site project at the ecosystem homepage', () => {
    assert.equal(getProjectPrimaryHref('site'), '/');
    assert.equal(getProjectRepositoryHref('site'), 'https://github.com/A3S-Lab/a3s/tree/main/apps/docs');
  });
});
