import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { getProjectPrimaryHref, getProjectRepositoryHref } from './project-links';

describe('project links', () => {
  test('opens the published Form playground while keeping GitHub separate', () => {
    assert.equal(getProjectPrimaryHref('form'), '/form/');
    assert.equal(getProjectRepositoryHref('form'), 'https://github.com/A3S-Lab/Form');
  });
});
