import { spawn } from 'node:child_process';
import { cp, mkdir, rm } from 'node:fs/promises';
import path from 'node:path';

const root = process.cwd();
const buildRoot = path.join(root, 'site_build');
const output = path.join(root, 'out');

function runRspress(locale) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ['x', 'rspress', 'build'], {
      cwd: root,
      env: {
        ...process.env,
        PUBLIC_SITE_LOCALE: locale,
        SITE_LOCALE: locale,
        SITE_OUT_DIR: path.join('site_build', locale),
      },
      stdio: 'inherit',
    });

    child.on('error', reject);
    child.on('exit', (code) => {
      if (code === 0) resolve();
      else reject(new Error(`Rspress ${locale} build exited with code ${code}`));
    });
  });
}

await rm(buildRoot, { force: true, recursive: true });
await rm(output, { force: true, recursive: true });

await runRspress('cn');
await runRspress('en');

await cp(path.join(buildRoot, 'cn'), output, { recursive: true });
await mkdir(path.join(output, 'en'), { recursive: true });
await cp(path.join(buildRoot, 'en'), path.join(output, 'en'), { recursive: true });

for (const route of ['download', 'en/download']) {
  const routeDirectory = path.join(output, route);
  await mkdir(routeDirectory, { recursive: true });
  await cp(`${routeDirectory}.html`, path.join(routeDirectory, 'index.html'));
}

console.log('Assembled Chinese and English Rspress builds in out/.');
