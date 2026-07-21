import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { existsSync } from 'node:fs';
import { createServer } from 'node:http';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  BenchRunContractError,
  candidateLockArguments,
  normalizeRunRequest,
  runArguments,
  runStageForStatus,
  taskCheckArguments,
  taskLockArguments,
} from './bench-arguments.mjs';

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(scriptDirectory, '..');
const repositoryRoot = resolve(appRoot, '../..');
const host = process.env.A3S_BENCH_API_HOST ?? '127.0.0.1';
const port = Number(process.env.A3S_BENCH_API_PORT ?? 29655);
const developmentPort = Number(process.env.A3S_WINDHOLE_DEV_PORT ?? 3030);
const workingDirectory = resolve(process.env.A3S_BENCH_CWD ?? repositoryRoot);
const maxOutputBytes = 8 * 1024 * 1024;
const maxBodyBytes = 64 * 1024;
const expectedBenchComponent = 'bench';
const expectedBenchProtocol = 'a3s-bench-cli/v1';
const jobs = new Map();
const allowedBrowserOrigins = localBrowserOrigins(port, developmentPort);

if (!['127.0.0.1', '::1', 'localhost'].includes(host)) {
  throw new Error('Windhole Bench bridge must bind to a loopback address');
}
if (!Number.isInteger(port) || port < 1 || port > 65535) {
  throw new Error('A3S_BENCH_API_PORT must be a valid TCP port');
}
if (!Number.isInteger(developmentPort) || developmentPort < 1 || developmentPort > 65535) {
  throw new Error('A3S_WINDHOLE_DEV_PORT must be a valid TCP port');
}

class BridgeError extends Error {
  constructor(message, status = 500, statusCode = 'INTERNAL_ERROR', details) {
    super(message);
    this.name = 'BridgeError';
    this.status = status;
    this.statusCode = statusCode;
    this.details = details;
  }
}

function resolveBenchCommand() {
  if (process.env.A3S_BENCH_BIN) {
    return { command: process.env.A3S_BENCH_BIN, prefix: [] };
  }

  for (const candidate of [
    resolve(repositoryRoot, 'crates/bench/target/debug/a3s-bench'),
    resolve(repositoryRoot, 'crates/bench/target/release/a3s-bench'),
  ]) {
    if (existsSync(candidate)) return { command: candidate, prefix: [] };
  }

  return {
    command: process.env.A3S_BIN ?? 'a3s',
    prefix: ['bench'],
  };
}

const benchCommand = resolveBenchCommand();
let componentInfoPromise;

function executeBench(args, options = {}) {
  const timeoutMs = options.timeoutMs ?? 30_000;
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(benchCommand.command, [...benchCommand.prefix, ...args], {
      cwd: workingDirectory,
      env: process.env,
      shell: false,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    let outputBytes = 0;
    let settled = false;

    const finish = (callback) => {
      if (settled) return;
      settled = true;
      if (timer) clearTimeout(timer);
      callback();
    };

    const collect = (target, chunk) => {
      outputBytes += chunk.length;
      if (outputBytes > maxOutputBytes) {
        child.kill('SIGTERM');
        finish(() =>
          rejectPromise(new BridgeError('Bench output exceeded the local bridge limit', 502, 'OUTPUT_LIMIT'))
        );
        return target;
      }
      return target + chunk.toString('utf8');
    };

    child.stdout.on('data', (chunk) => {
      stdout = collect(stdout, chunk);
    });
    child.stderr.on('data', (chunk) => {
      stderr = collect(stderr, chunk);
    });
    child.on('error', (error) => {
      finish(() => {
        rejectPromise(
          new BridgeError(`Could not start A3S Bench: ${error.message}`, 503, 'BENCH_UNAVAILABLE', {
            command: benchCommand.command,
          })
        );
      });
    });
    child.on('close', (code, signal) => {
      finish(() => {
        const payload = parseBenchOutput(stdout);
        if (code === 0) {
          resolvePromise(payload ?? { stdout: stdout.trim() });
          return;
        }
        const message =
          cliEnvelopeErrorMessage(payload) ??
          lastNonEmptyLine(stderr) ??
          lastNonEmptyLine(stdout) ??
          `A3S Bench exited with ${signal ? `signal ${signal}` : `code ${code}`}`;
        const runId = args[0] === 'run' ? failedRunId(message) : undefined;
        rejectPromise(
          new BridgeError(message, 502, 'BENCH_COMMAND_FAILED', {
            command: args[0] ?? 'component-info',
            exitCode: code,
            ...(runId ? { runId } : {}),
          })
        );
      });
    });

    const timer =
      timeoutMs > 0
        ? setTimeout(() => {
            child.kill('SIGTERM');
            finish(() => rejectPromise(new BridgeError('A3S Bench command timed out', 504, 'BENCH_TIMEOUT')));
          }, timeoutMs)
        : undefined;
  });
}

function benchComponentInfo() {
  if (!componentInfoPromise) {
    componentInfoPromise = executeBench(['--component-info', '--json'])
      .then(validateBenchComponentInfo)
      .catch((error) => {
        componentInfoPromise = undefined;
        throw error;
      });
  }
  return componentInfoPromise;
}

function validateBenchComponentInfo(value) {
  const component = typeof value?.component === 'string' ? value.component : undefined;
  const protocol = typeof value?.cli_protocol === 'string' ? value.cli_protocol : undefined;
  if (component !== expectedBenchComponent || protocol !== expectedBenchProtocol) {
    throw new BridgeError('Configured CLI is not a compatible A3S Bench component', 502, 'BENCH_PROTOCOL_MISMATCH', {
      expected: { component: expectedBenchComponent, cliProtocol: expectedBenchProtocol },
      received: { component, cliProtocol: protocol },
    });
  }
  return value;
}

function parseBenchOutput(output) {
  const lines = output
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter(Boolean)
    .reverse();
  for (const line of lines) {
    try {
      const value = JSON.parse(line);
      if (value && typeof value === 'object') return value;
    } catch {
      // Bench may emit a human-readable setup line before its JSON envelope.
    }
  }
  return undefined;
}

function cliEnvelopeErrorMessage(value) {
  return typeof value?.error?.message === 'string' && value.error.message.trim()
    ? value.error.message.trim()
    : undefined;
}

function failedRunId(message) {
  if (typeof message !== 'string') return undefined;
  const match = message.trim().match(/^(?:a3s bench:\s*)?run (local-[A-Za-z0-9-]{1,122}) failed(?::|$)/u);
  return match?.[1];
}

function lastNonEmptyLine(value) {
  return value
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter(Boolean)
    .at(-1);
}

function unwrapCliEnvelope(value) {
  if (value?.schema !== 'a3s.bench.output.v1' || typeof value.ok !== 'boolean') {
    throw new BridgeError('A3S Bench returned an incompatible JSON envelope', 502, 'BENCH_PROTOCOL_MISMATCH');
  }
  if (!value.ok) {
    throw new BridgeError(cliEnvelopeErrorMessage(value) ?? 'A3S Bench command failed', 502, 'BENCH_COMMAND_FAILED');
  }
  return value.data;
}

function localBrowserOrigins(...ports) {
  const origins = new Set();
  for (const localPort of ports) {
    origins.add(`http://127.0.0.1:${localPort}`);
    origins.add(`http://localhost:${localPort}`);
    origins.add(`http://[::1]:${localPort}`);
  }
  return origins;
}

function assertAllowedOrigin(request) {
  const origin = request.headers.origin;
  if (origin === undefined) return;
  if (typeof origin !== 'string' || !allowedBrowserOrigins.has(origin)) {
    throw new BridgeError('Browser Origin is not allowed', 403, 'ORIGIN_NOT_ALLOWED');
  }
}

function assertJsonContentType(request) {
  const contentType = request.headers['content-type'];
  const mediaType = typeof contentType === 'string' ? contentType.split(';', 1)[0].trim().toLowerCase() : '';
  if (mediaType !== 'application/json') {
    throw new BridgeError('POST requests require Content-Type application/json', 415, 'UNSUPPORTED_MEDIA_TYPE');
  }
}

function requiredString(value, name, maxLength = 1024) {
  if (typeof value !== 'string' || !value.trim()) {
    throw new BridgeError(`${name} is required`, 400, 'INVALID_REQUEST');
  }
  const normalized = value.trim();
  if (normalized.length > maxLength || /[\0\r\n]/u.test(normalized)) {
    throw new BridgeError(`${name} is invalid`, 400, 'INVALID_REQUEST');
  }
  return normalized;
}

function optionalString(value, name, maxLength = 1024) {
  if (value === undefined || value === null || value === '') return undefined;
  return requiredString(value, name, maxLength);
}

async function readJsonBody(request) {
  let size = 0;
  const chunks = [];
  for await (const chunk of request) {
    size += chunk.length;
    if (size > maxBodyBytes) throw new BridgeError('Request body is too large', 413, 'PAYLOAD_TOO_LARGE');
    chunks.push(chunk);
  }
  if (!chunks.length) return {};
  let value;
  try {
    value = JSON.parse(Buffer.concat(chunks).toString('utf8'));
  } catch {
    throw new BridgeError('Request body must be valid JSON', 400, 'INVALID_JSON');
  }
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new BridgeError('Request body must be a JSON object', 400, 'INVALID_JSON');
  }
  return value;
}

function publicJob(job) {
  return {
    jobId: job.jobId,
    task: job.task,
    candidate: job.candidate,
    model: job.model,
    locked: job.locked,
    status: job.status,
    stage: runStageForStatus(job.status),
    runId: job.runId,
    startedAt: job.startedAt,
    completedAt: job.completedAt,
    result: job.result,
    error: job.error,
  };
}

function startRun(input) {
  let runInput;
  try {
    runInput = normalizeRunRequest(input);
  } catch (error) {
    if (error instanceof BenchRunContractError) {
      throw new BridgeError(error.message, 400, 'INVALID_REQUEST', error.field ? { field: error.field } : undefined);
    }
    throw error;
  }
  const { task, candidate, model, locked } = runInput;
  if ([...jobs.values()].filter((job) => job.status === 'running').length >= 2) {
    throw new BridgeError('Two Bench runs are already active', 429, 'RUN_LIMIT_REACHED');
  }

  const job = {
    jobId: `windhole-${randomUUID()}`,
    task,
    candidate,
    model,
    locked,
    status: 'running',
    runId: undefined,
    startedAt: new Date().toISOString(),
    completedAt: undefined,
    result: undefined,
    error: undefined,
  };
  jobs.set(job.jobId, job);
  trimJobs();

  const args = runArguments(runInput);

  executeBench(args, { timeoutMs: 0 })
    .then((payload) => {
      job.status = 'completed';
      job.completedAt = new Date().toISOString();
      job.result = unwrapCliEnvelope(payload);
    })
    .catch((error) => {
      job.status = 'failed';
      job.runId = bridgeErrorRunId(error);
      job.completedAt = new Date().toISOString();
      job.error = error instanceof Error ? error.message : String(error);
    });

  return publicJob(job);
}

function bridgeErrorRunId(error) {
  if (!(error instanceof BridgeError) || error.details === null || typeof error.details !== 'object') return undefined;
  const runId = error.details.runId;
  return typeof runId === 'string' ? runId : undefined;
}

function trimJobs() {
  const completed = [...jobs.values()]
    .filter((job) => job.status !== 'running')
    .sort((left, right) => left.startedAt.localeCompare(right.startedAt));
  while (jobs.size > 100 && completed.length) {
    jobs.delete(completed.shift().jobId);
  }
}

function sendSuccess(response, data, code = 200, message = 'Success') {
  sendJson(response, code, {
    code,
    message,
    data,
    requestId: randomUUID(),
    timestamp: new Date().toISOString(),
  });
}

function sendError(response, error) {
  const bridgeError =
    error instanceof BridgeError
      ? error
      : new BridgeError(error instanceof Error ? error.message : 'Unexpected bridge error');
  sendJson(response, bridgeError.status, {
    code: bridgeError.status,
    statusCode: bridgeError.statusCode,
    message: bridgeError.message,
    details: bridgeError.details ?? {},
    requestId: randomUUID(),
    timestamp: new Date().toISOString(),
  });
}

function sendJson(response, status, value) {
  response.writeHead(status, {
    'Cache-Control': 'no-store',
    'Content-Type': 'application/json; charset=utf-8',
    'X-Content-Type-Options': 'nosniff',
  });
  response.end(JSON.stringify(value));
}

async function route(request, response) {
  const url = new URL(request.url ?? '/', `http://${host}:${port}`);
  const path = url.pathname.replace(/\/$/u, '') || '/';
  assertAllowedOrigin(request);
  if (request.method === 'POST') assertJsonContentType(request);

  if (request.method === 'GET' && path === '/api/v1/bench/health') {
    const component = await benchComponentInfo();
    sendSuccess(response, {
      connected: true,
      component: component.component,
      version: component.version,
      target: component.target,
      cliProtocol: component.cli_protocol,
      workingDirectory,
    });
    return;
  }

  if (request.method === 'GET' && path === '/api/v1/bench/tasks') {
    await benchComponentInfo();
    const args = ['list'];
    if (url.searchParams.get('all') === 'true') args.push('--all');
    args.push('--json');
    sendSuccess(response, unwrapCliEnvelope(await executeBench(args)));
    return;
  }

  const taskMatch = path.match(/^\/api\/v1\/bench\/tasks\/([^/]+)$/u);
  if (request.method === 'GET' && taskMatch) {
    await benchComponentInfo();
    const args = ['info', requiredString(decodeURIComponent(taskMatch[1]), 'task')];
    if (url.searchParams.get('all') === 'true') args.push('--all');
    args.push('--json');
    sendSuccess(response, unwrapCliEnvelope(await executeBench(args)));
    return;
  }

  if (request.method === 'POST' && path === '/api/v1/bench/doctor') {
    await benchComponentInfo();
    sendSuccess(
      response,
      unwrapCliEnvelope(await executeBench(['advanced', 'doctor', '--json'], { timeoutMs: 60_000 }))
    );
    return;
  }

  if (request.method === 'POST' && path === '/api/v1/bench/tasks/check') {
    await benchComponentInfo();
    const input = await readJsonBody(request);
    const source = requiredString(input.source, 'source');
    const output = await executeBench(taskCheckArguments(source), { timeoutMs: 60_000 });
    sendSuccess(response, { message: output.stdout ?? 'Task is valid' });
    return;
  }

  if (request.method === 'POST' && path === '/api/v1/bench/locks/task') {
    await benchComponentInfo();
    const input = await readJsonBody(request);
    const source = requiredString(input.source, 'source');
    const outputPath = requiredString(input.outputPath, 'outputPath');
    const output = await executeBench(taskLockArguments({ source, outputPath }), { timeoutMs: 0 });
    sendSuccess(response, { message: output.stdout ?? 'Task lock created', outputPath });
    return;
  }

  if (request.method === 'POST' && path === '/api/v1/bench/locks/candidate') {
    await benchComponentInfo();
    const input = await readJsonBody(request);
    const candidate = requiredString(input.candidate, 'candidate');
    const model = optionalString(input.model, 'model', 256);
    const outputPath = requiredString(input.outputPath, 'outputPath');
    const args = candidateLockArguments({ candidate, model, outputPath });
    const output = await executeBench(args, { timeoutMs: 0 });
    sendSuccess(response, { message: output.stdout ?? 'Candidate lock created', outputPath });
    return;
  }

  if (request.method === 'POST' && path === '/api/v1/bench/runs') {
    await benchComponentInfo();
    sendSuccess(response, startRun(await readJsonBody(request)), 202, 'Bench run accepted');
    return;
  }

  const runMatch = path.match(/^\/api\/v1\/bench\/runs\/([^/]+)$/u);
  if (request.method === 'GET' && runMatch) {
    const jobId = requiredString(decodeURIComponent(runMatch[1]), 'jobId', 128);
    const job = jobs.get(jobId);
    if (!job) throw new BridgeError('Bench run job was not found', 404, 'RUN_NOT_FOUND');
    sendSuccess(response, publicJob(job));
    return;
  }

  if (request.method === 'GET' && path === '/api/v1/bench/results/latest') {
    await benchComponentInfo();
    sendSuccess(response, unwrapCliEnvelope(await executeBench(['result', '--json'])));
    return;
  }

  const resultMatch = path.match(/^\/api\/v1\/bench\/results\/([^/]+)$/u);
  if (request.method === 'GET' && resultMatch) {
    await benchComponentInfo();
    const runId = requiredString(decodeURIComponent(resultMatch[1]), 'runId', 128);
    sendSuccess(response, unwrapCliEnvelope(await executeBench(['result', runId, '--json'])));
    return;
  }

  throw new BridgeError('Route not found', 404, 'NOT_FOUND');
}

const server = createServer((request, response) => {
  route(request, response).catch((error) => sendError(response, error));
});

server.listen(port, host, () => {
  process.stdout.write(`Windhole Bench bridge listening on http://${host}:${port}\n`);
  process.stdout.write(`Bench working directory: ${workingDirectory}\n`);
});
