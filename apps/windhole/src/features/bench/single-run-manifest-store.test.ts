import { beforeEach, describe, expect, it } from 'vitest';
import { demoTasks } from '../../data/demo-tasks';
import {
  createRunSortieSnapshot,
  isBenchRunActive,
  isSingleRunActive,
  type LabRunState,
  labState,
} from '../../state/lab-state';
import type { BenchRunResult } from '../../types/bench';
import { DEFAULT_HANGAR_ROSTER } from '../hangar/hangar-configuration';
import {
  restoreSingleRunManifest,
  SINGLE_RUN_MANIFEST_STORAGE_KEY,
  startSingleRunManifestPersistence,
} from './single-run-manifest-store';

class MemoryStorage {
  readonly values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }

  removeItem(key: string): void {
    this.values.delete(key);
  }
}

beforeEach(() => {
  labState.run = { stage: 'idle' };
  labState.campaign = { generation: 0, status: 'idle', members: [] };
});

describe('single run manifest store', () => {
  it('restores interrupted exact-result tracking explicitly without losing frozen ownership identifiers', () => {
    const storage = new MemoryStorage();
    labState.run = awaitingExactResult();
    persist(storage, '2026-07-17T00:03:10.000Z');

    labState.run = { stage: 'idle' };
    const restoredAt = '2026-07-17T00:04:00.000Z';
    expect(restoreSingleRunManifest(storage, () => restoredAt)).toBe(true);

    expect(labState.run).toMatchObject({
      mode: 'live',
      stage: 'completed',
      trackingStatus: 'tracking_stopped',
      trackingStoppedAt: restoredAt,
      jobId: 'job-single-1',
      runId: 'run-single-1',
      startedAt: '2026-07-17T00:00:00.000Z',
      completedAt: '2026-07-17T00:03:00.000Z',
      sortie: {
        task: { id: demoTasks[0].id },
        rosterEntry: { id: DEFAULT_HANGAR_ROSTER[0].id },
        input: {
          task: demoTasks[0].id,
          candidate: DEFAULT_HANGAR_ROSTER[0].candidate,
          model: DEFAULT_HANGAR_ROSTER[0].model,
          locked: false,
        },
      },
    });
    expect(labState.run.result).toBeUndefined();
    expect(isSingleRunActive()).toBe(false);
    expect(isBenchRunActive()).toBe(false);
    expect(Object.isFrozen(labState.run.sortie)).toBe(true);
    expect(Object.isFrozen(labState.run.sortie?.task)).toBe(true);
    expect(Object.isFrozen(labState.run.sortie?.rosterEntry)).toBe(true);
    expect(Object.isFrozen(labState.run.sortie?.input)).toBe(true);
  });

  it('restores completed and failed records with their exact terminal data', () => {
    const completedStorage = new MemoryStorage();
    labState.run = completedRun();
    persist(completedStorage, '2026-07-17T00:03:10.000Z');

    labState.run = { stage: 'idle' };
    expect(restoreSingleRunManifest(completedStorage)).toBe(true);
    expect(labState.run).toMatchObject({
      stage: 'completed',
      jobId: 'job-single-1',
      runId: 'run-single-1',
      completedAt: '2026-07-17T00:03:00.000Z',
      result: { status: 'completed', run_id: 'run-single-1', score: '0.9625' },
    });
    expect(labState.run.trackingStatus).toBeUndefined();
    expect(labState.run.trackingStoppedAt).toBeUndefined();

    const failedStorage = new MemoryStorage();
    labState.run = failedRun();
    persist(failedStorage, '2026-07-17T00:06:10.000Z');

    labState.run = { stage: 'idle' };
    expect(restoreSingleRunManifest(failedStorage)).toBe(true);
    expect(labState.run).toMatchObject({
      stage: 'failed',
      jobId: 'job-single-failed',
      runId: 'local-1721188800000-42-0',
      startedAt: '2026-07-17T00:05:00.000Z',
      completedAt: '2026-07-17T00:06:00.000Z',
      error: 'Candidate Adapter exited with code 17',
    });
    expect(labState.run.result).toBeUndefined();
    expect(labState.run.trackingStatus).toBeUndefined();
  });

  it('round-trips nullable optional model usage from a completed Bench result', () => {
    const storage = new MemoryStorage();
    labState.run = {
      ...completedRun(),
      result: completedResultWithNullableUsage(),
    };
    persist(storage, '2026-07-17T00:03:10.000Z');

    labState.run = { stage: 'idle' };
    expect(restoreSingleRunManifest(storage)).toBe(true);
    expect(labState.run.result?.model_usage).toEqual({
      prompt_tokens: 10,
      completion_tokens: 20,
      total_tokens: 30,
      cache_read_tokens: null,
      cache_write_tokens: null,
      tool_calls_count: null,
    });
  });

  it('observes replacement and nested Job transitions, then preserves the first interruption timestamp', async () => {
    const storage = new MemoryStorage();
    const stop = startSingleRunManifestPersistence(storage, () => '2026-07-17T00:02:00.000Z');
    labState.run = {
      mode: 'live',
      stage: 'planned',
      startedAt: '2026-07-17T00:00:00.000Z',
      sortie: sortie(),
    };
    await Promise.resolve();
    labState.run.jobId = 'job-single-live';
    labState.run.stage = 'running';
    await Promise.resolve();
    stop();

    const stored = storedManifest(storage);
    expect(stored.run).toMatchObject({
      status: 'active',
      stage: 'running',
      jobId: 'job-single-live',
    });

    labState.run = { stage: 'idle' };
    expect(restoreSingleRunManifest(storage, () => '2026-07-17T00:03:00.000Z')).toBe(true);
    expect(labState.run).toMatchObject({
      trackingStatus: 'tracking_stopped',
      trackingStoppedAt: '2026-07-17T00:03:00.000Z',
      jobId: 'job-single-live',
      stage: 'running',
    });

    persist(storage, '2026-07-17T00:03:10.000Z');
    labState.run = { stage: 'idle' };
    expect(restoreSingleRunManifest(storage, () => '2026-07-17T00:09:00.000Z')).toBe(true);
    expect(labState.run.trackingStoppedAt).toBe('2026-07-17T00:03:00.000Z');
  });

  it('rejects malformed ownership, inconsistent lifecycle state, extra fields, and invalid timestamps', () => {
    const mutations: Array<(manifest: Record<string, unknown>) => void> = [
      (manifest) => {
        manifest.untrusted = true;
      },
      (manifest) => {
        runRecord(manifest).status = 'active';
      },
      (manifest) => {
        const run = runRecord(manifest);
        (run.result as Record<string, unknown>).run_id = 'another-run';
      },
      (manifest) => {
        const run = runRecord(manifest);
        (run.result as Record<string, unknown>).task_id = 'another-task';
      },
      (manifest) => {
        const run = runRecord(manifest);
        const sortieRecord = run.sortie as Record<string, unknown>;
        (sortieRecord.rosterEntry as Record<string, unknown>).model = 'provider/changed';
      },
      (manifest) => {
        runRecord(manifest).trackingStoppedAt = '2026-07-17T00:02:00.000Z';
      },
      (manifest) => {
        const run = runRecord(manifest);
        const sortieRecord = run.sortie as Record<string, unknown>;
        (sortieRecord.rosterEntry as Record<string, unknown>).airframeId = 'unknown-aircraft';
      },
      (manifest) => {
        runRecord(manifest).completedAt = '2026-07-16T23:59:59.000Z';
      },
    ];

    for (const mutate of mutations) {
      const storage = storedCompletedRun();
      const manifest = storedManifest(storage);
      mutate(manifest);
      storage.setItem(SINGLE_RUN_MANIFEST_STORAGE_KEY, JSON.stringify(manifest));
      labState.run = { stage: 'idle' };

      expect(restoreSingleRunManifest(storage)).toBe(false);
      expect(labState.run).toEqual({ stage: 'idle' });
    }
  });

  it('rejects unsupported versions and oversized payloads, and clears persistence after an explicit idle reset', () => {
    const unsupported = storedCompletedRun();
    const manifest = storedManifest(unsupported);
    manifest.version = 2;
    unsupported.setItem(SINGLE_RUN_MANIFEST_STORAGE_KEY, JSON.stringify(manifest));
    expect(restoreSingleRunManifest(unsupported)).toBe(false);

    const oversized = new MemoryStorage();
    oversized.setItem(SINGLE_RUN_MANIFEST_STORAGE_KEY, 'x'.repeat(128 * 1_024 + 1));
    expect(restoreSingleRunManifest(oversized)).toBe(false);

    const cleared = storedCompletedRun();
    labState.run = { stage: 'idle' };
    const stop = startSingleRunManifestPersistence(cleared);
    stop();
    expect(cleared.getItem(SINGLE_RUN_MANIFEST_STORAGE_KEY)).toBeNull();
  });
});

function sortie() {
  return createRunSortieSnapshot(demoTasks[0], DEFAULT_HANGAR_ROSTER[0], {
    task: demoTasks[0].id,
    candidate: DEFAULT_HANGAR_ROSTER[0].candidate,
    model: DEFAULT_HANGAR_ROSTER[0].model,
    locked: false,
  });
}

function awaitingExactResult(): LabRunState {
  return {
    mode: 'live',
    stage: 'completed',
    jobId: 'job-single-1',
    runId: 'run-single-1',
    startedAt: '2026-07-17T00:00:00.000Z',
    completedAt: '2026-07-17T00:03:00.000Z',
    sortie: sortie(),
  };
}

function completedRun(): LabRunState {
  return {
    ...awaitingExactResult(),
    result: completedResult(),
  };
}

function failedRun(): LabRunState {
  return {
    mode: 'live',
    stage: 'failed',
    jobId: 'job-single-failed',
    runId: 'local-1721188800000-42-0',
    startedAt: '2026-07-17T00:05:00.000Z',
    completedAt: '2026-07-17T00:06:00.000Z',
    sortie: sortie(),
    error: 'Candidate Adapter exited with code 17',
  };
}

function completedResult(): BenchRunResult {
  return {
    status: 'completed',
    governance_status: 'local_unofficial',
    run_id: 'run-single-1',
    task_id: demoTasks[0].id,
    score: '0.9625',
    model: DEFAULT_HANGAR_ROSTER[0].model,
    model_usage: {
      prompt_tokens: 10,
      completion_tokens: 20,
      total_tokens: 30,
      tool_calls_count: 2,
    },
  };
}

function completedResultWithNullableUsage(): BenchRunResult {
  return {
    ...completedResult(),
    model_usage: {
      prompt_tokens: 10,
      completion_tokens: 20,
      total_tokens: 30,
      cache_read_tokens: null,
      cache_write_tokens: null,
      tool_calls_count: null,
    },
  };
}

function persist(storage: MemoryStorage, now: string): void {
  const stop = startSingleRunManifestPersistence(storage, () => now);
  stop();
}

function storedCompletedRun(): MemoryStorage {
  const storage = new MemoryStorage();
  labState.run = completedRun();
  persist(storage, '2026-07-17T00:03:10.000Z');
  return storage;
}

function storedManifest(storage: MemoryStorage): Record<string, unknown> {
  const serialized = storage.getItem(SINGLE_RUN_MANIFEST_STORAGE_KEY);
  expect(serialized).not.toBeNull();
  return JSON.parse(serialized as string) as Record<string, unknown>;
}

function runRecord(manifest: Record<string, unknown>): Record<string, unknown> {
  return manifest.run as Record<string, unknown>;
}
