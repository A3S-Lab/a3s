import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { describe, test } from 'node:test';
import { architectureProjects } from './architecture';
import { featuredProjectSites } from './project-sites';

describe('featured project site previews', () => {
  test('point to known projects and committed screenshots', () => {
    const projectIds = new Set(architectureProjects.map((project) => project.id));

    assert.equal(featuredProjectSites.length, 8);
    assert.equal(new Set(featuredProjectSites.map((site) => site.id)).size, 8);

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

  test('features the Box site and a truthful Form build preview', () => {
    const box = featuredProjectSites.find((site) => site.id === 'box');
    const form = featuredProjectSites.find((site) => site.id === 'form');

    assert.equal(box?.href, 'https://a3s-lab.github.io/Box/');
    assert.equal(box?.mode, 'live');
    assert.equal(form?.captureUrl, 'https://a3s-lab.github.io/Form/playground/');
    assert.equal(form?.href, 'https://github.com/A3S-Lab/Form');
    assert.equal(form?.mode, 'build');
    assert.equal(featuredProjectSites.map((site) => String(site.id)).includes('site'), false);
  });

  test('install Chinese fonts before build-time screenshots are captured', () => {
    const workflow = readFileSync(
      new URL('../../../../.github/workflows/site.yml', import.meta.url),
      'utf8',
    );
    const fontInstall = workflow.indexOf('fonts-noto-cjk');
    const screenshotCapture = workflow.indexOf('bun run capture:sites');

    assert.notEqual(fontInstall, -1, 'Pages must install a CJK font for project screenshots');
    assert.notEqual(screenshotCapture, -1, 'Pages must refresh project screenshots');
    assert.equal(fontInstall < screenshotCapture, true, 'CJK fonts must be installed before screenshots are captured');
  });

  test('checks preview HTTP status before Chrome can replace a committed screenshot', () => {
    const captureScript = readFileSync(
      new URL('../../scripts/capture-project-sites.ts', import.meta.url),
      'utf8',
    );
    const healthCheck = captureScript.indexOf('await assertCaptureUrlIsHealthy(captureUrl)');
    const chromeCapture = captureScript.indexOf('await captureWithChrome(captureUrl');

    assert.notEqual(healthCheck, -1);
    assert.notEqual(chromeCapture, -1);
    assert.equal(healthCheck < chromeCapture, true);
    assert.equal(captureScript.includes('response.ok'), true);
    assert.equal(captureScript.includes("client.send('Page.captureScreenshot'"), true);
    assert.equal(captureScript.includes('--virtual-time-budget'), false);
  });
});
