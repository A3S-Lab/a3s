import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  filterArchitectureProjects,
  replacementArchitectureProject,
} from './architecture-filter';

describe('homepage architecture search', () => {
  test('matches project names, node names, and node descriptions', () => {
    assert.deepEqual(
      filterArchitectureProjects('all', 'Office').map(
        (project) => project.name,
      ),
      ['A3S', 'Web', 'Parser', 'Use', 'Office'],
    );
    assert.deepEqual(
      filterArchitectureProjects('all', 'PDFium').map(
        (project) => project.name,
      ),
      ['Parser', 'Office'],
    );
    assert.deepEqual(
      filterArchitectureProjects('all', '终态不可变').map(
        (project) => project.name,
      ),
      ['Runtime'],
    );
    assert.deepEqual(
      filterArchitectureProjects('all', 'SIGINT').map(
        (project) => project.name,
      ),
      ['Test'],
    );
  });

  test('replaces a selected project that is absent from non-empty results', () => {
    const matches = filterArchitectureProjects('all', 'PDFium');

    assert.equal(
      replacementArchitectureProject(matches, 'a3s-system')?.name,
      'Parser',
    );
    assert.equal(replacementArchitectureProject(matches, 'office'), undefined);
    assert.equal(replacementArchitectureProject([], 'a3s-system'), undefined);
  });

  test('prefers an exact project name over broader component matches', () => {
    const matches = filterArchitectureProjects('all', 'Office');

    assert.equal(
      replacementArchitectureProject(matches, 'a3s-system', 'Office')?.name,
      'Office',
    );
    assert.equal(
      replacementArchitectureProject(matches, 'office', 'Office'),
      undefined,
    );
  });
});
