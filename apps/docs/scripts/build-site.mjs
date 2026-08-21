import { spawn } from 'node:child_process';
import { access, cp, mkdir, rm } from 'node:fs/promises';
import path from 'node:path';

const root = process.cwd();
const repositoryRoot = path.resolve(root, '../..');
const buildRoot = path.join(root, 'site_build');
const output = path.join(root, 'out');
const powerSiteRoot = path.join(repositoryRoot, 'crates', 'power', 'site');
const powerPackage = path.join(powerSiteRoot, 'package.json');
const powerBuildRoot = path.join(powerSiteRoot, 'doc_build');
const publicSite = new URL(process.env.SITE_URL ?? 'https://a3s.dev/');
const deploymentPath = publicSite.pathname.replace(/\/+$/, '');
const powerBase = `${deploymentPath}/power/`.replace(/\/+/g, '/');
const npmCommand = process.platform === 'win32' ? 'npm.cmd' : 'npm';

function run(command, args, { cwd = root, env = {}, label }) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      env: {
        ...process.env,
        ...env,
      },
      stdio: 'inherit',
    });

    child.on('error', reject);
    child.on('exit', (code) => {
      if (code === 0) resolve();
      else reject(new Error(`${label} exited with code ${code}`));
    });
  });
}

function runRspress(locale) {
  return run(process.execPath, ['x', 'rspress', 'build'], {
    env: {
      PUBLIC_SITE_LOCALE: locale,
      SITE_LOCALE: locale,
      SITE_OUT_DIR: path.join('site_build', locale),
    },
    label: `Rspress ${locale} build`,
  });
}

async function buildPowerSite() {
  try {
    await access(powerPackage);
  } catch {
    throw new Error(
      'Pinned Power site is missing. Run `git submodule update --init crates/power` before building.',
    );
  }

  await run(npmCommand, ['run', 'build'], {
    cwd: powerSiteRoot,
    env: {
      DOCS_BASE: powerBase,
      DOCS_ORIGIN: publicSite.origin,
    },
    label: 'Power site build',
  });
  await run(npmCommand, ['run', 'check:site'], {
    cwd: powerSiteRoot,
    label: 'Power site validation',
  });
}

await rm(buildRoot, { force: true, recursive: true });
await rm(output, { force: true, recursive: true });

await runRspress('cn');
await runRspress('en');
await buildPowerSite();

await cp(path.join(buildRoot, 'cn'), output, { recursive: true });
await mkdir(path.join(output, 'en'), { recursive: true });
await cp(path.join(buildRoot, 'en'), path.join(output, 'en'), { recursive: true });
await mkdir(path.join(output, 'power'), { recursive: true });
await cp(powerBuildRoot, path.join(output, 'power'), { recursive: true });

console.log('Assembled the A3S ecosystem and pinned Power sites in out/.');
