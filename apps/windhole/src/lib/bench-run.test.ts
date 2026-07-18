import { describe, expect, it } from 'vitest';
import { createBenchRunInput } from './bench-run';

const configuration = {
  taskId: 'quick_file_edit',
  candidate: './candidate',
  candidateLock: './candidate.lock.json',
  model: 'openai/gpt-5',
  taskLock: './task.lock.json',
};

describe('createBenchRunInput', () => {
  it('uses the selected Task, Candidate adapter, and optional model for an ordinary run', () => {
    expect(createBenchRunInput({ ...configuration, locked: false })).toEqual({
      task: 'quick_file_edit',
      candidate: './candidate',
      model: 'openai/gpt-5',
      locked: false,
    });
  });

  it('uses both lock paths and omits the model for a locked run', () => {
    expect(createBenchRunInput({ ...configuration, locked: true })).toEqual({
      task: './task.lock.json',
      candidate: './candidate.lock.json',
      model: undefined,
      locked: true,
    });
  });
});
