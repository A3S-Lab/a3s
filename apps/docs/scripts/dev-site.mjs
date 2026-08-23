import { spawn } from 'node:child_process';

const locale = process.argv[2] === 'en' ? 'en' : 'cn';
const child = spawn(process.execPath, ['x', 'rspress', 'dev', ...process.argv.slice(3)], {
  cwd: process.cwd(),
  env: {
    ...process.env,
    PUBLIC_SITE_LOCALE: locale,
    SITE_LOCALE: locale,
  },
  stdio: 'inherit',
});

child.on('error', (error) => {
  throw error;
});

child.on('exit', (code) => {
  process.exitCode = code ?? 1;
});
