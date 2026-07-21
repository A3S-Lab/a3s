import { beforeEach, describe, expect, it, vi } from 'vitest';
import { benchApi } from './api';
import { createBenchRunInput } from './bench-run';

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal('fetch', fetchMock);
  fetchMock.mockImplementation(() => Promise.resolve(successResponse({})));
});

describe('benchApi', () => {
  it('maps catalog and task detail operations', async () => {
    await benchApi.health();
    await benchApi.tasks(true);
    await benchApi.task('./local task', false);

    expect(fetchMock.mock.calls.map(([path]) => path)).toEqual([
      '/api/v1/bench/health',
      '/api/v1/bench/tasks?all=true',
      '/api/v1/bench/tasks/.%2Flocal%20task?all=false',
    ]);
  });

  it('maps run and result operations', async () => {
    await benchApi.startRun({
      task: 'quick_file_edit',
      candidate: './candidate',
      model: 'openai/gpt-5.6',
      locked: false,
    });
    await benchApi.run('windhole-job');
    await benchApi.result('local-123');
    await benchApi.latestResult();

    expect(fetchMock.mock.calls.map(([path]) => path)).toEqual([
      '/api/v1/bench/runs',
      '/api/v1/bench/runs/windhole-job',
      '/api/v1/bench/results/local-123',
      '/api/v1/bench/results/latest',
    ]);
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({
      task: 'quick_file_edit',
      candidate: './candidate',
      model: 'openai/gpt-5.6',
      locked: false,
    });
  });

  it('serializes the exact locked input produced by createBenchRunInput', async () => {
    const input = createBenchRunInput({
      taskId: 'portfolio_risk_calibration',
      candidate: 'claude-code',
      model: 'anthropic/claude-opus-4.6',
      candidateLock: '  ./locks/claude.candidate.lock.json  ',
      taskLock: '  ./locks/risk-map.task.lock.json  ',
      locked: true,
    });

    await benchApi.startRun(input);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0][0]).toBe('/api/v1/bench/runs');
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({
      task: './locks/risk-map.task.lock.json',
      candidate: './locks/claude.candidate.lock.json',
      locked: true,
    });
  });

  it('preserves a structured Run ID from a failed Bridge job', async () => {
    fetchMock.mockResolvedValueOnce(
      successResponse({
        jobId: 'windhole-job-failed',
        task: 'quick_file_edit',
        candidate: './candidate',
        locked: false,
        status: 'failed',
        stage: 'failed',
        runId: 'local-1721188800000-42-0',
        startedAt: '2026-07-17T00:00:00.000Z',
        completedAt: '2026-07-17T00:00:01.000Z',
        error: 'run local-1721188800000-42-0 failed: Candidate failed',
      })
    );

    await expect(benchApi.run('windhole-job-failed')).resolves.toMatchObject({
      status: 'failed',
      runId: 'local-1721188800000-42-0',
    });
  });

  it('maps doctor, validation, and both lock operations', async () => {
    await benchApi.doctor();
    await benchApi.checkTask('./task');
    await benchApi.createTaskLock({ source: './task', outputPath: './task.lock.json' });
    await benchApi.createCandidateLock({
      candidate: './candidate',
      model: 'openai/gpt-5',
      outputPath: './candidate.lock.json',
    });

    expect(fetchMock.mock.calls.map(([path]) => path)).toEqual([
      '/api/v1/bench/doctor',
      '/api/v1/bench/tasks/check',
      '/api/v1/bench/locks/task',
      '/api/v1/bench/locks/candidate',
    ]);
  });
});

function successResponse(data: unknown): Response {
  return new Response(
    JSON.stringify({
      code: 200,
      message: 'Success',
      data,
    }),
    {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    }
  );
}
