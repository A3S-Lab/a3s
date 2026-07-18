import type { StartBenchRunInput } from '../types/bench';

export interface BenchRunConfiguration {
  taskId: string;
  candidate: string;
  candidateLock: string;
  model: string;
  taskLock: string;
  locked: boolean;
}

export function createBenchRunInput(configuration: BenchRunConfiguration): StartBenchRunInput {
  const locked = configuration.locked;
  return {
    task: (locked ? configuration.taskLock : configuration.taskId).trim(),
    candidate: (locked ? configuration.candidateLock : configuration.candidate).trim(),
    model: locked ? undefined : configuration.model.trim() || undefined,
    locked,
  };
}
