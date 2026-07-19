import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { chmod, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { after, before, describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';
import {
  candidateLockArguments,
  normalizeRunRequest,
  runArguments,
  runStageForStatus,
  taskCheckArguments,
  taskLockArguments,
} from './bench-arguments.mjs';

let fakeBenchDirectory;
let fakeBenchPath;

before(async () => {
  fakeBenchDirectory = await mkdtemp(join(tmpdir(), 'a3s-bench-bridge-test-'));
  fakeBenchPath = join(fakeBenchDirectory, 'fake-bench.mjs');
  await writeFile(fakeBenchPath, fakeBenchSource(), 'utf8');
  await chmod(fakeBenchPath, 0o755);
});

after(async () => {
  if (fakeBenchDirectory) await rm(fakeBenchDirectory, { force: true, recursive: true });
});

describe('Bench argument builders', () => {
  it('maps every run option without invoking a shell', () => {
    assert.deepEqual(
      runArguments({ task: 'quick_file_edit', candidate: './candidate', model: 'openai/gpt-5', locked: false }),
      ['run', 'quick_file_edit', '--agent', './candidate', '--model', 'openai/gpt-5', '--json']
    );
    assert.deepEqual(runArguments({ task: './task.lock.json', candidate: './candidate.lock.json', locked: true }), [
      'run',
      './task.lock.json',
      '--agent',
      './candidate.lock.json',
      '--locked',
      '--json',
    ]);
  });

  it('maps Task validation and lock commands', () => {
    assert.deepEqual(taskCheckArguments('./task'), ['advanced', 'check', './task']);
    assert.deepEqual(taskLockArguments({ source: './task', outputPath: '/tmp/task.lock.json' }), [
      'advanced',
      'task',
      'lock',
      './task',
      '--out',
      '/tmp/task.lock.json',
    ]);
  });

  it('maps Candidate locks with an optional model', () => {
    assert.deepEqual(
      candidateLockArguments({
        candidate: './candidate',
        model: 'openai/gpt-5',
        outputPath: '/tmp/candidate.lock.json',
      }),
      ['advanced', 'candidate', 'lock', './candidate', '--model', 'openai/gpt-5', '--out', '/tmp/candidate.lock.json']
    );
  });
});

describe('Bench run request contract', () => {
  it('requires locked to be an explicit boolean', () => {
    for (const locked of [undefined, null, 'true', 'false', 0, 1]) {
      assert.throws(
        () => normalizeRunRequest({ task: 'quick_file_edit', candidate: './candidate', locked }),
        /locked must be a boolean/u
      );
    }
  });

  it('normalizes an ordinary run and accepts a non-empty optional model', () => {
    assert.deepEqual(
      normalizeRunRequest({
        task: '  quick_file_edit  ',
        candidate: '  ./candidate  ',
        model: '  openai/gpt-5  ',
        locked: false,
      }),
      {
        task: 'quick_file_edit',
        candidate: './candidate',
        model: 'openai/gpt-5',
        locked: false,
      }
    );
    assert.deepEqual(normalizeRunRequest({ task: 'quick_file_edit', candidate: './candidate', locked: false }), {
      task: 'quick_file_edit',
      candidate: './candidate',
      locked: false,
    });
  });

  it('rejects empty ordinary model values instead of silently changing their meaning', () => {
    for (const model of ['', '   ', null, false]) {
      assert.throws(
        () => normalizeRunRequest({ task: 'quick_file_edit', candidate: './candidate', model, locked: false }),
        /model is required/u
      );
    }
  });

  it('requires a locked run to omit model entirely', () => {
    assert.deepEqual(
      normalizeRunRequest({
        task: './task.lock.json',
        candidate: './candidate.lock.json',
        locked: true,
      }),
      {
        task: './task.lock.json',
        candidate: './candidate.lock.json',
        locked: true,
      }
    );

    for (const model of ['openai/gpt-5', '', null]) {
      assert.throws(
        () =>
          normalizeRunRequest({
            task: './task.lock.json',
            candidate: './candidate.lock.json',
            model,
            locked: true,
          }),
        /model must be omitted for a locked run/u
      );
    }
  });

  it('rejects fields outside the public run contract', () => {
    assert.throws(
      () =>
        normalizeRunRequest({
          task: 'quick_file_edit',
          candidate: './candidate',
          locked: false,
          taskLock: './task.lock.json',
        }),
      /taskLock is not allowed/u
    );
  });

  it('maps only substantiated job statuses to compatible public stages', () => {
    assert.equal(runStageForStatus('running'), 'running');
    assert.equal(runStageForStatus('completed'), 'completed');
    assert.equal(runStageForStatus('failed'), 'failed');
    assert.throws(() => runStageForStatus('judging'), /Unsupported Bench run status/u);
  });

  it('enforces the input contract and public status mapping at POST /runs', async () => {
    const port = await availablePort();
    assert.ok(fakeBenchPath);
    const bridge = spawn(process.execPath, [fileURLToPath(new URL('./bench-bridge.mjs', import.meta.url))], {
      env: {
        ...process.env,
        A3S_BENCH_API_HOST: '127.0.0.1',
        A3S_BENCH_API_PORT: String(port),
        A3S_BENCH_BIN: fakeBenchPath,
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    });

    try {
      await bridgeReady(bridge);
      for (const [input, field] of [
        [{ task: 'quick_file_edit', candidate: './candidate' }, 'locked'],
        [{ task: 'quick_file_edit', candidate: './candidate', locked: 'false' }, 'locked'],
        [
          {
            task: './task.lock.json',
            candidate: './candidate.lock.json',
            model: 'openai/gpt-5',
            locked: true,
          },
          'model',
        ],
      ]) {
        const response = await fetch(`http://127.0.0.1:${port}/api/v1/bench/runs`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(input),
        });
        const payload = await response.json();
        assert.equal(response.status, 400);
        assert.equal(payload.statusCode, 'INVALID_REQUEST');
        assert.equal(payload.details.field, field);
      }

      for (const input of [
        {
          task: 'quick_file_edit',
          candidate: './candidate',
          model: 'openai/gpt-5',
          locked: false,
        },
        {
          task: './task.lock.json',
          candidate: './candidate.lock.json',
          locked: true,
        },
      ]) {
        const response = await fetch(`http://127.0.0.1:${port}/api/v1/bench/runs`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(input),
        });
        const accepted = await response.json();
        assert.equal(response.status, 202);
        assert.equal(accepted.data.status, 'running');
        assert.equal(accepted.data.stage, 'running');
        assert.notEqual(accepted.data.stage, 'candidate_running');
        assert.equal(accepted.data.locked, input.locked);

        const completed = await waitForJob(port, accepted.data.jobId);
        assert.equal(completed.status, 'completed');
        assert.equal(completed.stage, 'completed');
      }
    } finally {
      bridge.kill('SIGTERM');
      if (bridge.exitCode === null && bridge.signalCode === null) await once(bridge, 'exit');
    }
  });

  it('accepts only JSON POSTs from absent or exact local development Origins', async () => {
    const { bridge, port } = await startTestBridge();
    const endpoint = `http://127.0.0.1:${port}/api/v1/bench/runs`;
    const input = { task: 'quick_file_edit', candidate: './candidate', locked: false };

    try {
      for (const contentType of [undefined, 'text/plain', 'application/x-www-form-urlencoded']) {
        const headers = contentType ? { 'Content-Type': contentType } : undefined;
        const response = await fetch(endpoint, { method: 'POST', headers, body: JSON.stringify(input) });
        const payload = await response.json();
        assert.equal(response.status, 415);
        assert.equal(payload.statusCode, 'UNSUPPORTED_MEDIA_TYPE');
      }

      for (const origin of ['https://evil.example', 'http://127.0.0.1.evil.example:3030', 'null']) {
        const response = await fetch(endpoint, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json', Origin: origin },
          body: JSON.stringify(input),
        });
        const payload = await response.json();
        assert.equal(response.status, 403);
        assert.equal(payload.statusCode, 'ORIGIN_NOT_ALLOWED');
      }

      const maliciousRead = await fetch(`http://127.0.0.1:${port}/api/v1/bench/health`, {
        headers: { Origin: 'https://evil.example' },
      });
      assert.equal(maliciousRead.status, 403);

      for (const origin of ['http://127.0.0.1:3030', 'http://localhost:3030', `http://127.0.0.1:${port}`]) {
        const response = await fetch(endpoint, {
          method: 'POST',
          headers: { 'Content-Type': 'Application/JSON; charset=utf-8', Origin: origin },
          body: JSON.stringify(input),
        });
        const payload = await response.json();
        assert.equal(response.status, 202);
        assert.equal(payload.data.status, 'running');
        await waitForJob(port, payload.data.jobId);
      }
    } finally {
      await stopBridge(bridge);
    }
  });

  it('refuses a CLI whose component identity or protocol is not Bench v1', async () => {
    for (const environment of [{ FAKE_BENCH_COMPONENT: 'gateway' }, { FAKE_BENCH_PROTOCOL: 'a3s-bench-cli/v999' }]) {
      const { bridge, port } = await startTestBridge(environment);
      try {
        const response = await fetch(`http://127.0.0.1:${port}/api/v1/bench/health`);
        const payload = await response.json();
        assert.equal(response.status, 502);
        assert.equal(payload.statusCode, 'BENCH_PROTOCOL_MISMATCH');
        assert.deepEqual(payload.details.expected, {
          component: 'bench',
          cliProtocol: 'a3s-bench-cli/v1',
        });
      } finally {
        await stopBridge(bridge);
      }
    }
  });

  it('preserves only an authentic anchored failed Run ID on the public job', async () => {
    const { bridge, port } = await startTestBridge();
    const endpoint = `http://127.0.0.1:${port}/api/v1/bench/runs`;

    try {
      const authenticResponse = await fetch(endpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ task: 'quick_file_edit', candidate: './fail-real', locked: false }),
      });
      const authenticAccepted = await authenticResponse.json();
      assert.equal(authenticResponse.status, 202);
      const authenticFailure = await waitForJob(port, authenticAccepted.data.jobId);
      assert.equal(authenticFailure.status, 'failed');
      assert.equal(authenticFailure.stage, 'failed');
      assert.equal(authenticFailure.runId, 'local-1721188800000-42-0');
      assert.match(authenticFailure.error, /^run local-1721188800000-42-0 failed:/u);

      const untrustedResponse = await fetch(endpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ task: 'quick_file_edit', candidate: './fail-unanchored', locked: false }),
      });
      const untrustedAccepted = await untrustedResponse.json();
      assert.equal(untrustedResponse.status, 202);
      const untrustedFailure = await waitForJob(port, untrustedAccepted.data.jobId);
      assert.equal(untrustedFailure.status, 'failed');
      assert.equal(Object.hasOwn(untrustedFailure, 'runId'), false);
      assert.match(untrustedFailure.error, /run local-invented-1 failed/u);
    } finally {
      await stopBridge(bridge);
    }
  });
});

async function startTestBridge(environment = {}) {
  assert.ok(fakeBenchPath);
  const port = await availablePort();
  const bridge = spawn(process.execPath, [fileURLToPath(new URL('./bench-bridge.mjs', import.meta.url))], {
    env: {
      ...process.env,
      A3S_BENCH_API_HOST: '127.0.0.1',
      A3S_BENCH_API_PORT: String(port),
      A3S_BENCH_BIN: fakeBenchPath,
      ...environment,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  try {
    await bridgeReady(bridge);
    return { bridge, port };
  } catch (error) {
    await stopBridge(bridge);
    throw error;
  }
}

async function stopBridge(bridge) {
  if (bridge.exitCode !== null || bridge.signalCode !== null) return;
  bridge.kill('SIGTERM');
  await once(bridge, 'exit');
}

async function waitForJob(port, jobId) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    const response = await fetch(`http://127.0.0.1:${port}/api/v1/bench/runs/${encodeURIComponent(jobId)}`);
    const payload = await response.json();
    assert.equal(response.status, 200);
    if (payload.data.status !== 'running') return payload.data;
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 5));
  }
  throw new Error(`Bench job ${jobId} did not finish`);
}

async function availablePort() {
  const server = createServer();
  await new Promise((resolvePromise, rejectPromise) => {
    server.once('error', rejectPromise);
    server.listen(0, '127.0.0.1', resolvePromise);
  });
  const address = server.address();
  assert.notEqual(address, null);
  assert.equal(typeof address, 'object');
  const port = address.port;
  await new Promise((resolvePromise, rejectPromise) =>
    server.close((error) => (error ? rejectPromise(error) : resolvePromise()))
  );
  return port;
}

async function bridgeReady(bridge) {
  let stdout = '';
  let stderr = '';
  bridge.stdout.on('data', (chunk) => {
    stdout += chunk.toString('utf8');
  });
  bridge.stderr.on('data', (chunk) => {
    stderr += chunk.toString('utf8');
  });

  await new Promise((resolvePromise, rejectPromise) => {
    const cleanup = () => {
      clearTimeout(timer);
      bridge.stdout.off('data', onOutput);
      bridge.off('exit', onExit);
    };
    const onOutput = (chunk) => {
      if (!chunk.toString('utf8').includes('Bench bridge listening')) return;
      cleanup();
      resolvePromise();
    };
    const onExit = (code, signal) => {
      cleanup();
      rejectPromise(new Error(`Bench bridge exited before startup (${code ?? signal}): ${stderr || stdout}`));
    };
    const timer = setTimeout(() => {
      cleanup();
      rejectPromise(new Error(`Timed out waiting for Bench bridge: ${stderr || stdout}`));
    }, 5_000);
    bridge.stdout.on('data', onOutput);
    bridge.once('exit', onExit);
  });
}

function fakeBenchSource() {
  return [
    '#!/usr/bin/env node',
    'const args = process.argv.slice(2);',
    "if (args.join(' ') === '--component-info --json') {",
    "  process.stdout.write(JSON.stringify({ component: process.env.FAKE_BENCH_COMPONENT ?? 'bench', version: 'test', target: 'test', cli_protocol: process.env.FAKE_BENCH_PROTOCOL ?? 'a3s-bench-cli/v1' }) + '\\n');",
    '} else {',
    "  const command = args[0] ?? 'unknown';",
    "  const agentIndex = args.indexOf('--agent');",
    "  const candidate = agentIndex >= 0 ? args[agentIndex + 1] : '';",
    "  if (command === 'run' && candidate.includes('fail')) {",
    "    const runId = 'local-1721188800000-42-0';",
    "    const message = candidate.includes('unanchored') ? 'Candidate mentioned run local-invented-1 failed' : 'run ' + runId + ' failed: Candidate Adapter exited';",
    "    process.stdout.write(JSON.stringify({ schema: 'a3s.bench.output.v1', command, ok: false, error: { code: 'command_failed', message } }) + '\\n');",
    '    process.exitCode = 2;',
    '  } else {',
    "    const data = command === 'run' ? { status: 'completed', run_id: 'local-1721188800001-42-1' } : {};",
    "    process.stdout.write(JSON.stringify({ schema: 'a3s.bench.output.v1', command, ok: true, data }) + '\\n');",
    '  }',
    '}',
    '',
  ].join('\n');
}
