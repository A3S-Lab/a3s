import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { describe, test } from 'node:test';
import { architectureProjects } from './architecture';
import { featuredProjectSites, getFeaturedProjectSiteHref } from './project-sites';

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

  test('features the Box and Power sites', () => {
    const box = featuredProjectSites.find((site) => site.id === 'box');
    const power = featuredProjectSites.find((site) => site.id === 'power');

    assert.equal(box?.href, 'https://a3s-lab.github.io/Box/');
    assert.equal(box?.mode, 'live');
    assert.equal(power?.captureUrl, 'https://a3s-lab.github.io/a3s/power/');
    assert.equal(power?.href, '/power/');
    assert.equal(power && getFeaturedProjectSiteHref(power, 'cn'), '/power/');
    assert.equal(power && getFeaturedProjectSiteHref(power, 'en'), '/power/en/');
    assert.equal(power?.mode, 'live');
    assert.equal(power?.destination, 'site');
    assert.equal(featuredProjectSites.map((site) => String(site.id)).includes('site'), false);
  });

  test('gives animated product demos enough time to reach a useful frame', () => {
    const settleTimes = new Map(featuredProjectSites.map((site) => [site.id, site.settleMs]));

    assert.equal((settleTimes.get('cloud') ?? 0) >= 10_000, true);
    assert.equal((settleTimes.get('code') ?? 0) >= 12_000, true);
    assert.equal((settleTimes.get('box') ?? 0) >= 4_000, true);
    assert.equal((settleTimes.get('power') ?? 0) >= 5_000, true);
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

  test('builds the pinned Power site without publishing a nested Form playground', () => {
    const workflow = readFileSync(
      new URL('../../../../.github/workflows/site.yml', import.meta.url),
      'utf8',
    );
    const screenshotCapture = workflow.indexOf('bun run capture:sites');
    const siteBuild = workflow.indexOf('run: bun run build');
    const powerSubmodule = workflow.indexOf('submodule update --init --depth 1 crates/power');
    const powerInstall = workflow.indexOf('npm ci --prefix crates/power/site');
    const powerTypecheck = workflow.indexOf('bun run typecheck:power');

    assert.notEqual(screenshotCapture, -1, 'Pages must refresh project screenshots');
    assert.notEqual(siteBuild, -1, 'Pages must build the Rspress site');
    assert.notEqual(powerSubmodule, -1, 'Pages must initialize the pinned Power source');
    assert.notEqual(powerInstall, -1, 'Pages must install the pinned Power site dependencies');
    assert.notEqual(powerTypecheck, -1, 'Pages must typecheck the pinned Power site');
    assert.equal(powerSubmodule < powerInstall, true);
    assert.equal(powerInstall < powerTypecheck, true);
    assert.equal(powerTypecheck < siteBuild, true);
    assert.equal(screenshotCapture < siteBuild, true);
    assert.equal(workflow.includes('UI_REPOSITORY'), false);
    assert.equal(workflow.includes('UI_REVISION'), false);
    assert.equal(workflow.includes('A3S_FORM'), false);
    assert.equal(workflow.includes('form:playground:build'), false);
    assert.equal(workflow.includes('apps/docs/out/form'), false);
    assert.equal(workflow.includes('REQUIRE_FORM_PREVIEW'), false);
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
