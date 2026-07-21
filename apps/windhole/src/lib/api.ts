import type {
  BenchDoctorResult,
  BenchHealth,
  BenchOperationResult,
  BenchRunJob,
  BenchRunResult,
  BenchTask,
  CandidateLockInput,
  StartBenchRunInput,
  TaskLockInput,
} from '../types/bench';

interface ApiEnvelope<T> {
  code: number;
  message: string;
  data: T;
  requestId?: string;
  timestamp?: string;
}

export class ApiError extends Error {
  readonly status: number;
  readonly details?: unknown;

  constructor(message: string, status: number, details?: unknown) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.details = details;
  }
}

export function unwrapApiResponse<T>(value: unknown): T {
  if (
    value !== null &&
    typeof value === 'object' &&
    'code' in value &&
    'data' in value &&
    typeof (value as Partial<ApiEnvelope<T>>).code === 'number'
  ) {
    return (value as ApiEnvelope<T>).data;
  }
  return value as T;
}

async function parseResponse(response: Response): Promise<unknown> {
  const text = await response.text();
  if (!text) return undefined;
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return text;
  }
}

function errorMessage(value: unknown, fallback: string): string {
  if (typeof value === 'string' && value.trim()) return value;
  if (value && typeof value === 'object') {
    const message = (value as Record<string, unknown>).message;
    if (typeof message === 'string' && message.trim()) return message;
  }
  return fallback;
}

export async function apiRequest<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set('Accept', 'application/json');
  if (init.body !== undefined && !headers.has('Content-Type')) headers.set('Content-Type', 'application/json');
  const response = await fetch(path, { ...init, headers });
  const payload = await parseResponse(response);
  if (!response.ok) {
    throw new ApiError(errorMessage(payload, `Request failed with HTTP ${response.status}`), response.status, payload);
  }
  return unwrapApiResponse<T>(payload);
}

function jsonBody(value: unknown): Pick<RequestInit, 'body' | 'headers'> {
  return {
    body: JSON.stringify(value),
    headers: { 'Content-Type': 'application/json' },
  };
}

export const benchApi = {
  health: () => apiRequest<BenchHealth>('/api/v1/bench/health'),
  tasks: (includeBlocked = true) =>
    apiRequest<{ tasks: BenchTask[] }>(`/api/v1/bench/tasks?all=${includeBlocked ? 'true' : 'false'}`),
  task: (taskId: string, includeBlocked = true) =>
    apiRequest<{ task: BenchTask }>(
      `/api/v1/bench/tasks/${encodeURIComponent(taskId)}?all=${includeBlocked ? 'true' : 'false'}`
    ),
  doctor: () =>
    apiRequest<BenchDoctorResult>('/api/v1/bench/doctor', {
      method: 'POST',
      ...jsonBody({}),
    }),
  startRun: (input: StartBenchRunInput, signal?: AbortSignal) =>
    apiRequest<BenchRunJob>('/api/v1/bench/runs', {
      method: 'POST',
      signal,
      ...jsonBody(input),
    }),
  run: (jobId: string, signal?: AbortSignal) =>
    apiRequest<BenchRunJob>(`/api/v1/bench/runs/${encodeURIComponent(jobId)}`, { signal }),
  result: (runId: string, signal?: AbortSignal) =>
    apiRequest<BenchRunResult>(`/api/v1/bench/results/${encodeURIComponent(runId)}`, { signal }),
  latestResult: () => apiRequest<BenchRunResult>('/api/v1/bench/results/latest'),
  checkTask: (source: string) =>
    apiRequest<BenchOperationResult>('/api/v1/bench/tasks/check', {
      method: 'POST',
      ...jsonBody({ source }),
    }),
  createTaskLock: (input: TaskLockInput) =>
    apiRequest<BenchOperationResult>('/api/v1/bench/locks/task', {
      method: 'POST',
      ...jsonBody(input),
    }),
  createCandidateLock: (input: CandidateLockInput) =>
    apiRequest<BenchOperationResult>('/api/v1/bench/locks/candidate', {
      method: 'POST',
      ...jsonBody(input),
    }),
};
