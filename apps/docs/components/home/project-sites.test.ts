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
        site.settleMs >= 1_500,
        true,
        `${site.id} must let its entrance animation settle before capture`,
      );
      assert.equal(
        existsSync(new URL(`../../public/${site.screenshot.slice(1)}`, import.meta.url)),
        true,
        `Missing screenshot for ${site.id}`,
      );
    }
  });

  test('features the Box site and the published Form playground', () => {
    const box = featuredProjectSites.find((site) => site.id === 'box');
    const form = featuredProjectSites.find((site) => site.id === 'form');

    assert.equal(box?.href, 'https://a3s-lab.github.io/Box/');
    assert.equal(box?.mode, 'live');
    assert.equal(form?.captureUrl, 'https://a3s-lab.github.io/a3s/form/');
    assert.equal(form?.href, '/form/');
    assert.equal(form?.mode, 'live');
    assert.equal(form?.destination, 'site');
    assert.equal(featuredProjectSites.map((site) => String(site.id)).includes('site'), false);
  });

  test('gives animated product demos enough time to reach a useful frame', () => {
    const settleTimes = new Map(featuredProjectSites.map((site) => [site.id, site.settleMs]));

    assert.equal((settleTimes.get('cloud') ?? 0) >= 10_000, true);
    assert.equal((settleTimes.get('code') ?? 0) >= 12_000, true);
    assert.equal((settleTimes.get('box') ?? 0) >= 4_000, true);
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

  test('builds and publishes the pinned Form playground', () => {
    const workflow = readFileSync(
      new URL('../../../../.github/workflows/site.yml', import.meta.url),
      'utf8',
    );
    const formRevision = workflow.indexOf('FORM_REVISION:');
    const formBase = workflow.indexOf('A3S_FORM_BASE: ./');
    const formBuild = workflow.indexOf('playground:build');
    const formPreviewUrl = workflow.indexOf('A3S_FORM_PREVIEW_URL: http://127.0.0.1:4176/');
    const screenshotCapture = workflow.indexOf('bun run capture:sites');
    const siteBuild = workflow.indexOf('run: bun run build');
    const formPublish = workflow.indexOf('cp -R "$RUNNER_TEMP/a3s-form/playground-dist" apps/docs/out/form');
    const formValidation = workflow.indexOf("REQUIRE_FORM_PREVIEW: '1'");

    assert.notEqual(formRevision, -1, 'Form preview source must be pinned');
    assert.notEqual(formBase, -1, 'Form playground assets must use relative paths');
    assert.notEqual(formBuild, -1, 'Form playground must be built');
    assert.notEqual(formPreviewUrl, -1, 'Capture must use the local Form build');
    assert.notEqual(formPublish, -1, 'Form playground must be included in the Pages artifact');
    assert.notEqual(formValidation, -1, 'Pages validation must require the Form playground');
    assert.equal(formRevision < formBuild, true);
    assert.equal(formBase < formBuild, true);
    assert.equal(formBuild < screenshotCapture, true);
    assert.equal(formPreviewUrl < screenshotCapture, true);
    assert.equal(siteBuild < formPublish, true);
    assert.equal(formPublish < formValidation, true);
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
    assert.equal(captureScript.includes('site.settleMs'), true);
    assert.equal(captureScript.includes('await waitForPageTarget(port, chromeProcess)'), true);
    assert.equal(captureScript.includes('animation-play-state: paused'), true);
    assert.equal(captureScript.includes("document.querySelectorAll('video')"), true);
    assert.equal(captureScript.includes('--virtual-time-budget'), false);
  });
});
