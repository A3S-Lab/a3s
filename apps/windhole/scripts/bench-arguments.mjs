const RUN_REQUEST_KEYS = new Set(['task', 'candidate', 'model', 'locked']);

export class BenchRunContractError extends TypeError {
  constructor(message, field) {
    super(message);
    this.name = 'BenchRunContractError';
    this.field = field;
  }
}

export function normalizeRunRequest(input) {
  if (input === null || typeof input !== 'object' || Array.isArray(input)) {
    throw new BenchRunContractError('run input must be an object');
  }

  for (const key of Object.keys(input)) {
    if (!RUN_REQUEST_KEYS.has(key)) {
      throw new BenchRunContractError(`${key} is not allowed`, key);
    }
  }

  if (typeof input.locked !== 'boolean') {
    throw new BenchRunContractError('locked must be a boolean', 'locked');
  }

  const task = normalizedString(input.task, 'task');
  const candidate = normalizedString(input.candidate, 'candidate');
  const hasModel = Object.hasOwn(input, 'model');

  if (input.locked && hasModel) {
    throw new BenchRunContractError('model must be omitted for a locked run', 'model');
  }

  if (!hasModel) return { task, candidate, locked: input.locked };
  return { task, candidate, model: normalizedString(input.model, 'model', 256), locked: input.locked };
}

export function runStageForStatus(status) {
  switch (status) {
    case 'running':
      return 'running';
    case 'completed':
      return 'completed';
    case 'failed':
      return 'failed';
    default:
      throw new TypeError(`Unsupported Bench run status: ${String(status)}`);
  }
}

export function runArguments(input) {
  const { task, candidate, model, locked } = normalizeRunRequest(input);
  const args = ['run', task, '--agent', candidate];
  if (model) args.push('--model', model);
  if (locked) args.push('--locked');
  args.push('--json');
  return args;
}

function normalizedString(value, name, maxLength = 1024) {
  if (typeof value !== 'string' || !value.trim()) {
    throw new BenchRunContractError(`${name} is required`, name);
  }
  const normalized = value.trim();
  if (normalized.length > maxLength || /[\0\r\n]/u.test(normalized)) {
    throw new BenchRunContractError(`${name} is invalid`, name);
  }
  return normalized;
}

export function taskCheckArguments(source) {
  return ['advanced', 'check', source];
}

export function taskLockArguments({ source, outputPath }) {
  return ['advanced', 'task', 'lock', source, '--out', outputPath];
}

export function candidateLockArguments({ candidate, model, outputPath }) {
  const args = ['advanced', 'candidate', 'lock', candidate];
  if (model) args.push('--model', model);
  args.push('--out', outputPath);
  return args;
}
