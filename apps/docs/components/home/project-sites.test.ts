import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { describe, test } from 'node:test';
import { architectureProjects } from './architecture';
import { featuredProjectSites } from './project-sites';

describe('featured project site previews', () => {
  test('point to known projects and committed screenshots', () => {
    const projectIds = new Set(architectureProjects.map((project) => project.id));

    assert.equal(featuredProjectSites.length, 7);
    assert.equal(new Set(featuredProjectSites.map((site) => site.id)).size, 7);

    for (const site of featuredProjectSites) {
      assert.equal(projectIds.has(site.id), true);
      assert.equal(site.captureUrl.startsWith('https://'), true);
      assert.equal(
        existsSync(new URL(`../../public/${site.screenshot.slice(1)}`, import.meta.url)),
        true,
        `Missing screenshot for ${site.id}`,
      );
    }
  });
});
