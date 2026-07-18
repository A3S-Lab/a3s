import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { labState } from '../../../state/lab-state';
import type { EngineeringController } from '../use-engineering-controller';
import { EngineeringWorkspace } from './engineering-workspace';

const actions: EngineeringController = {
  setTaskSource: vi.fn(),
  setTaskLockOutput: vi.fn(),
  setCandidate: vi.fn(),
  setCandidateModel: vi.fn(),
  setCandidateLockOutput: vi.fn(),
  runDoctor: vi.fn(async () => undefined),
  checkTask: vi.fn(async () => undefined),
  createTaskLock: vi.fn(async () => undefined),
  createCandidateLock: vi.fn(async () => undefined),
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  labState.connection = { mode: 'checking', message: '正在连接本机 A3S Bench…' };
  labState.run = { stage: 'idle' };
  labState.campaign = { generation: 0, status: 'idle', members: [] };
  labState.engineering.activeOperation = undefined;
});

describe('EngineeringWorkspace', () => {
  it('shows the Doctor Runtime provider, version detail, and configured Judge model', () => {
    labState.connection = {
      mode: 'live',
      message: '本机能力已连接',
      doctor: {
        runtime: { provider: 'docker', ready: true, detail: '27.5.1' },
        judge_model: 'openai/o3-judge',
      },
    };

    render(<EngineeringWorkspace actions={actions} />);

    expect(screen.getByText('Runtime Provider')).toBeInTheDocument();
    expect(screen.getByText('docker')).toBeInTheDocument();
    expect(screen.getByText('Runtime 版本 / 详情')).toBeInTheDocument();
    expect(screen.getByText('27.5.1')).toBeInTheDocument();
    expect(screen.getByText('Judge Model')).toBeInTheDocument();
    expect(screen.getByText('openai/o3-judge')).toBeInTheDocument();
  });

  it('labels a missing Judge model as not configured after Doctor completes', () => {
    labState.connection = {
      mode: 'live',
      message: '本机能力已连接',
      doctor: {
        runtime: { provider: 'os-runtime', ready: true, detail: 'authenticated A3S OS at http://127.0.0.1' },
        judge_model: null,
      },
    };

    render(<EngineeringWorkspace actions={actions} />);

    expect(screen.getByText('未配置')).toBeInTheDocument();
  });

  it('presents every advanced operation as an engineering bay module', () => {
    labState.connection = { mode: 'live', message: '本机能力已连接' };

    render(<EngineeringWorkspace actions={actions} />);

    expect(screen.getByRole('heading', { name: '工程舱' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '航电自检' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '任务模块校验' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '任务封装舱' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '智能体封装舱' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '启动自检' }));
    fireEvent.click(screen.getByRole('button', { name: '扫描任务模块' }));
    fireEvent.click(screen.getByRole('button', { name: '封装 Task Lock' }));
    fireEvent.click(screen.getByRole('button', { name: '封装 Candidate Lock' }));

    expect(actions.runDoctor).toHaveBeenCalledOnce();
    expect(actions.checkTask).toHaveBeenCalledOnce();
    expect(actions.createTaskLock).toHaveBeenCalledOnce();
    expect(actions.createCandidateLock).toHaveBeenCalledOnce();
  });

  it.each([
    ['single-sortie', { runStage: 'candidate_running' as const, campaignStatus: 'idle' as const }],
    ['Campaign', { runStage: 'idle' as const, campaignStatus: 'running' as const }],
  ])('blocks file-producing controls during an active %s evaluation but keeps read-only diagnostics available', (_, state) => {
    labState.connection = { mode: 'live', message: '本机能力已连接' };
    labState.run = { stage: state.runStage };
    labState.campaign = { generation: 1, status: state.campaignStatus, members: [] };

    render(<EngineeringWorkspace actions={actions} />);

    const doctor = screen.getByRole('button', { name: '启动自检' });
    const check = screen.getByRole('button', { name: '扫描任务模块' });
    const taskLock = screen.getByRole('button', { name: '封装 Task Lock' });
    const candidateLock = screen.getByRole('button', { name: '封装 Candidate Lock' });

    expect(screen.getByText('评测中 · 工程写入锁定')).toBeInTheDocument();
    expect(doctor).toBeEnabled();
    expect(check).toBeEnabled();
    expect(taskLock).toBeDisabled();
    expect(candidateLock).toBeDisabled();
    expect(taskLock).toHaveAccessibleDescription('评测运行中，Lock 文件写入与出击配置同步已锁定。');
    expect(candidateLock).toHaveAccessibleDescription('评测运行中，Lock 文件写入与出击配置同步已锁定。');

    fireEvent.click(doctor);
    fireEvent.click(check);
    fireEvent.click(taskLock);
    fireEvent.click(candidateLock);

    expect(actions.runDoctor).toHaveBeenCalledOnce();
    expect(actions.checkTask).toHaveBeenCalledOnce();
    expect(actions.createTaskLock).not.toHaveBeenCalled();
    expect(actions.createCandidateLock).not.toHaveBeenCalled();
  });
});
