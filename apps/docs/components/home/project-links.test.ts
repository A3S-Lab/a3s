import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { getProjectPrimaryHref, getProjectRepositoryHref } from './project-links';

describe('project links', () => {
  test('opens the published Form playground while keeping GitHub separate', () => {
    assert.equal(getProjectPrimaryHref('form'), '/form/');
    assert.equal(getProjectRepositoryHref('form'), 'https://github.com/A3S-Lab/Form');
  });

  test('opens the Site project at the ecosystem homepage', () => {
    assert.equal(getProjectPrimaryHref('site'), '/');
    assert.equal(getProjectRepositoryHref('site'), 'https://github.com/A3S-Lab/a3s/tree/main/apps/docs');
  });

  test('opens Desktop at its root-owned application source', () => {
    assert.equal(
      getProjectPrimaryHref('desktop'),
      'https://github.com/A3S-Lab/a3s/tree/main/apps/desktop',
    );
  });
});
