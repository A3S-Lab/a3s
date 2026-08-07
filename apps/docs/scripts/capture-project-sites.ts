import { execFile } from 'node:child_process';
import { constants } from 'node:fs';
import { access, mkdir, mkdtemp, readdir, rename, rm, stat } from 'node:fs/promises';
import { homedir, platform, tmpdir } from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import { featuredProjectSites } from '../components/home/project-sites';

const execute = promisify(execFile);
const root = process.cwd();
const outputDirectory = path.join(root, 'public', 'ecosystem-sites');

async function isExecutable(candidate: string) {
  try {
    await access(candidate, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

async function findOnPath(command: string) {
  try {
    const { stdout } = await execute('which', [command]);
    return stdout.trim() || undefined;
  } catch {
    return undefined;
  }
}

async function findAgentBrowserChrome() {
  const browserRoot = path.join(homedir(), '.agent-browser', 'browsers');

  try {
    const versions = (await readdir(browserRoot)).sort().reverse();
    for (const version of versions) {
      const candidate = platform() === 'darwin'
        ? path.join(browserRoot, version, 'Google Chrome for Testing.app', 'Contents', 'MacOS', 'Google Chrome for Testing')
        : path.join(browserRoot, version, 'chrome');
      if (await isExecutable(candidate)) return candidate;
    }
  } catch {
    return undefined;
  }

  return undefined;
}

async function findChrome() {
  const explicit = process.env.CHROME_BIN;
  if (explicit && await isExecutable(explicit)) return explicit;

  const platformCandidates = platform() === 'darwin'
    ? [
        '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
        '/Applications/Chromium.app/Contents/MacOS/Chromium',
      ]
    : [];

  for (const candidate of platformCandidates) {
    if (await isExecutable(candidate)) return candidate;
  }

  for (const command of ['google-chrome-stable', 'google-chrome', 'chromium', 'chromium-browser']) {
    const candidate = await findOnPath(command);
    if (candidate) return candidate;
  }

  return findAgentBrowserChrome();
}

async function fileIsUsable(target: string) {
  try {
    return (await stat(target)).size > 20_000;
  } catch {
    return false;
  }
}

const requestedSite = process.argv.find((argument) => argument.startsWith('--site='))?.split('=')[1];
const sites = requestedSite
  ? featuredProjectSites.filter((site) => site.id === requestedSite)
  : [...featuredProjectSites];

if (requestedSite && sites.length === 0) {
  throw new Error(`Unknown project site: ${requestedSite}`);
}

const chrome = await findChrome();
if (!chrome) {
  throw new Error('Chrome was not found. Set CHROME_BIN or install Chrome/Chromium before capturing project sites.');
}
const chromeExecutable: string = chrome;

await mkdir(outputDirectory, { recursive: true });
const temporaryDirectory = await mkdtemp(path.join(tmpdir(), 'a3s-project-sites-'));
const failures: string[] = [];

async function capture(site: (typeof featuredProjectSites)[number]) {
  const output = path.join(outputDirectory, path.basename(site.screenshot));
  const temporaryOutput = path.join(temporaryDirectory, path.basename(site.screenshot));
  const profile = path.join(temporaryDirectory, `${site.id}-profile`);
  const captureUrl = site.id === 'site'
    ? process.env.A3S_SITE_PREVIEW_URL ?? site.captureUrl
    : site.captureUrl;

  try {
    await execute(chromeExecutable, [
      '--headless=new',
      '--disable-background-networking',
      '--disable-dev-shm-usage',
      '--disable-gpu',
      '--hide-scrollbars',
      '--no-default-browser-check',
      '--no-first-run',
      '--no-sandbox',
      '--run-all-compositor-stages-before-draw',
      '--force-device-scale-factor=1',
      '--window-size=1280,800',
      '--virtual-time-budget=8000',
      `--user-data-dir=${profile}`,
      `--screenshot=${temporaryOutput}`,
      captureUrl,
    ], { timeout: 45_000, maxBuffer: 4 * 1024 * 1024 });

    if (!await fileIsUsable(temporaryOutput)) {
      throw new Error('Chrome produced no usable screenshot');
    }

    await rename(temporaryOutput, output);
    console.log(`Captured ${site.id}: ${captureUrl}`);
  } catch (error) {
    if (await fileIsUsable(temporaryOutput)) {
      await rename(temporaryOutput, output);
      console.warn(`Captured ${site.id}: ${captureUrl} (Chrome did not exit cleanly)`);
      return;
    }

    if (await fileIsUsable(output)) {
      console.warn(`Keeping the committed ${site.id} screenshot: ${String(error)}`);
      return;
    }

    failures.push(`${site.id}: ${String(error)}`);
  }
}

try {
  let nextIndex = 0;
  async function worker() {
    while (nextIndex < sites.length) {
      const site = sites[nextIndex];
      nextIndex += 1;
      await capture(site);
    }
  }

  await Promise.all([worker(), worker()]);
} finally {
  await rm(temporaryDirectory, { force: true, recursive: true });
}

if (failures.length > 0) {
  throw new Error(`Project site capture failed:\n${failures.join('\n')}`);
}
