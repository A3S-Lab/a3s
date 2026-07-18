import { useCallback } from 'react';
import { benchApi } from '../../lib/api';
import { isEvaluationActive, labState } from '../../state/lab-state';
import type { BenchOperationResult } from '../../types/bench';

export interface EngineeringController {
  setTaskSource: (value: string) => void;
  setTaskLockOutput: (value: string) => void;
  setCandidate: (value: string) => void;
  setCandidateModel: (value: string) => void;
  setCandidateLockOutput: (value: string) => void;
  runDoctor: () => Promise<void>;
  checkTask: () => Promise<void>;
  createTaskLock: () => Promise<void>;
  createCandidateLock: () => Promise<void>;
}

export function useEngineeringController(): EngineeringController {
  const ensureLive = (): boolean => {
    if (labState.connection.mode === 'live') return true;
    labState.engineering.error = '工程舱需要连接本机 A3S Bench。模拟舱不会写入文件。';
    return false;
  };

  const execute = useCallback(
    async (
      operation: NonNullable<typeof labState.engineering.activeOperation>,
      action: () => Promise<BenchOperationResult>,
      onSuccess?: (result: BenchOperationResult) => void,
      mutatesRunConfig = false
    ) => {
      if (!ensureLive()) return;
      if (mutatesRunConfig && !ensureEngineeringWriteAllowed()) return;
      labState.engineering.activeOperation = operation;
      labState.engineering.lastOperation = undefined;
      labState.engineering.error = undefined;
      try {
        const result = await action();
        labState.engineering.lastOperation = result;
        if (mutatesRunConfig && !ensureRunConfigMutableAfterWrite()) return;
        onSuccess?.(result);
      } catch (error) {
        labState.engineering.error = readableError(error, '工程操作失败');
      } finally {
        labState.engineering.activeOperation = undefined;
      }
    },
    []
  );

  const runDoctor = useCallback(async () => {
    if (!ensureLive()) return;
    labState.engineering.activeOperation = 'doctor';
    labState.engineering.lastOperation = undefined;
    labState.engineering.error = undefined;
    try {
      const result = await benchApi.doctor();
      labState.connection.doctor = result;
      labState.engineering.lastOperation = {
        message: `Runtime ${result.runtime.provider} is ready (${result.runtime.detail})`,
      };
    } catch (error) {
      labState.engineering.error = readableError(error, '运行时检查失败');
    } finally {
      labState.engineering.activeOperation = undefined;
    }
  }, []);

  return {
    setTaskSource: (value) => {
      labState.engineering.taskSource = value;
    },
    setTaskLockOutput: (value) => {
      labState.engineering.taskLockOutput = value;
    },
    setCandidate: (value) => {
      labState.engineering.candidate = value;
    },
    setCandidateModel: (value) => {
      labState.engineering.candidateModel = value;
    },
    setCandidateLockOutput: (value) => {
      labState.engineering.candidateLockOutput = value;
    },
    runDoctor,
    checkTask: () =>
      execute('check', () =>
        benchApi.checkTask(requiredValue(labState.engineering.taskSource, '请输入 TaskBundle 路径。'))
      ),
    createTaskLock: () =>
      execute(
        'task-lock',
        () =>
          benchApi.createTaskLock({
            source: requiredValue(labState.engineering.taskSource, '请输入 Task 来源。'),
            outputPath: requiredValue(labState.engineering.taskLockOutput, '请输入 Task Lock 输出路径。'),
          }),
        (result) => {
          labState.runConfig.taskLock = result.outputPath ?? labState.engineering.taskLockOutput.trim();
        },
        true
      ),
    createCandidateLock: () =>
      execute(
        'candidate-lock',
        () =>
          benchApi.createCandidateLock({
            candidate: requiredValue(labState.engineering.candidate, '请输入 Candidate 适配器。'),
            model: labState.engineering.candidateModel.trim() || undefined,
            outputPath: requiredValue(labState.engineering.candidateLockOutput, '请输入 Candidate Lock 输出路径。'),
          }),
        (result) => {
          labState.runConfig.candidateLock = result.outputPath ?? labState.engineering.candidateLockOutput.trim();
        },
        true
      ),
  };
}

function ensureEngineeringWriteAllowed(): boolean {
  if (!isEvaluationActive(labState.run.stage, labState.campaign.status)) return true;
  labState.engineering.error = '评测运行中，不能创建 Lock 或修改当前出击配置。';
  return false;
}

function ensureRunConfigMutableAfterWrite(): boolean {
  if (!isEvaluationActive(labState.run.stage, labState.campaign.status)) return true;
  labState.engineering.error = '评测已启动；Lock 已生成，但当前出击配置保持不变。';
  return false;
}

function requiredValue(value: string, message: string): string {
  const normalized = value.trim();
  if (!normalized) throw new Error(message);
  return normalized;
}

function readableError(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback;
}
