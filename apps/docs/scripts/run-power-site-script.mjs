import { spawn } from 'node:child_process';
import path from 'node:path';

const supportedScripts = new Set(['typecheck']);
const script = process.argv[2];

if (!supportedScripts.has(script)) {
  throw new Error(`Unsupported Power site script: ${script ?? '(missing)'}`);
}

const powerSiteRoot = path.resolve(process.cwd(), '../../crates/power/site');
const npmCommand = process.platform === 'win32' ? 'npm.cmd' : 'npm';
const child = spawn(npmCommand, ['run', script], {
  cwd: powerSiteRoot,
  stdio: 'inherit',
});

child.on('error', (error) => {
  throw error;
});

child.on('exit', (code) => {
  process.exitCode = code ?? 1;
});
