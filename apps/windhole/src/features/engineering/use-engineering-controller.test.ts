import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { benchApi } from '../../lib/api';
import { labState } from '../../state/lab-state';
import { useEngineeringController } from './use-engineering-controller';

vi.mock('../../lib/api', () => ({
  benchApi: {
    doctor: vi.fn(),
    checkTask: vi.fn(),
    createTaskLock: vi.fn(),
    createCandidateLock: vi.fn(),
  },
}));

beforeEach(() => {
  vi.clearAllMocks();
  labState.connection = { mode: 'live', message: '本机能力已连接' };
  labState.run = { stage: 'idle' };
  labState.campaign = { generation: 0, status: 'idle', members: [] };
  labState.runConfig = {
    candidateLock: './candidate.lock.json',
    deploymentScope: 'single',
    locked: false,
    taskLock: './task.lock.json',
  };
  labState.engineering = {
    taskSource: './task',
    taskLockOutput: './new-task.lock.json',
    candidate: './candidate',
    candidateModel: 'provider/model',
    candidateLockOutput: './new-candidate.lock.json',
  };
  vi.mocked(benchApi.doctor).mockResolvedValue({ runtime: { provider: 'test', ready: true, detail: 'ready' } });
  vi.mocked(benchApi.checkTask).mockResolvedValue({ message: 'Task is valid' });
  vi.mocked(benchApi.createTaskLock).mockResolvedValue({
    message: 'Task Lock created',
    outputPath: './resolved-task.lock.json',
  });
  vi.mocked(benchApi.createCandidateLock).mockResolvedValue({
    message: 'Candidate Lock created',
    outputPath: './resolved-candidate.lock.json',
  });
});

afterEach(() => {
  cleanup();
});

describe('useEngineeringController evaluation guards', () => {
  it.each([
    ['single-sortie', { runStage: 'judging' as const, campaignStatus: 'idle' as const }],
    ['Campaign', { runStage: 'idle' as const, campaignStatus: 'running' as const }],
  ])('rejects Task and Candidate Lock writes during an active %s evaluation', async (_, state) => {
    labState.run = { stage: state.runStage };
    labState.campaign = { generation: 1, status: state.campaignStatus, members: [] };
    const originalRunConfig = { ...labState.runConfig };
    const { result } = renderHook(() => useEngineeringController());

    await act(async () => {
      await result.current.createTaskLock();
      await result.current.createCandidateLock();
    });

    expect(benchApi.createTaskLock).not.toHaveBeenCalled();
    expect(benchApi.createCandidateLock).not.toHaveBeenCalled();
    expect(labState.runConfig).toEqual(originalRunConfig);
    expect(labState.engineering.error).toBe('评测运行中，不能创建 Lock 或修改当前出击配置。');
  });

  it('keeps Doctor and Task check available as read-only operations during a Campaign', async () => {
    labState.campaign = { generation: 2, status: 'running', members: [] };
    const originalRunConfig = { ...labState.runConfig };
    const { result } = renderHook(() => useEngineeringController());

    await act(async () => {
      await result.current.runDoctor();
      await result.current.checkTask();
    });

    expect(benchApi.doctor).toHaveBeenCalledOnce();
    expect(benchApi.checkTask).toHaveBeenCalledWith('./task');
    expect(labState.runConfig).toEqual(originalRunConfig);
    expect(labState.engineering.error).toBeUndefined();
  });

  it('does not mutate runConfig if an evaluation starts before a Lock request resolves', async () => {
    let resolveLock: ((value: { message: string; outputPath: string }) => void) | undefined;
    vi.mocked(benchApi.createTaskLock).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveLock = resolve;
        })
    );
    const originalRunConfig = { ...labState.runConfig };
    const { result } = renderHook(() => useEngineeringController());

    let lockPromise: Promise<void> | undefined;
    act(() => {
      lockPromise = result.current.createTaskLock();
    });
    expect(benchApi.createTaskLock).toHaveBeenCalledOnce();

    labState.run = { stage: 'candidate_running' };
    resolveLock?.({ message: 'Task Lock created', outputPath: './racing-task.lock.json' });
    await act(async () => {
      await lockPromise;
    });

    expect(labState.runConfig).toEqual(originalRunConfig);
    expect(labState.engineering.lastOperation?.message).toBe('Task Lock created');
    expect(labState.engineering.error).toBe('评测已启动；Lock 已生成，但当前出击配置保持不变。');
  });
});
