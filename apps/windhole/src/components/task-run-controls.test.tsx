import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { demoTasks } from '../data/demo-tasks';
import type { BenchController } from '../features/bench/use-bench-controller';
import { DEFAULT_HANGAR_ROSTER } from '../features/hangar/hangar-configuration';
import { createRunCampaignSnapshot, createRunSortieSnapshot, labState } from '../state/lab-state';
import type { BenchTask } from '../types/bench';
import { runProgress, TaskRunControls } from './task-run-controls';

describe('TaskRunControls Candidate deployment readiness', () => {
  beforeEach(() => {
    labState.connection = {
      mode: 'live',
      message: 'Bench ready',
      doctor: { runtime: { provider: 'test', ready: true, detail: 'ready' } },
    };
    labState.run = { stage: 'idle' };
    labState.campaign = { generation: 0, status: 'idle', members: [] };
    labState.hangar.roster = DEFAULT_HANGAR_ROSTER.map((entry) => ({ ...entry }));
    labState.hangar.activeEntryId = DEFAULT_HANGAR_ROSTER[0].id;
    labState.runConfig = {
      candidateLock: './candidate.lock.json',
      deploymentScope: 'single',
      locked: false,
      taskLock: './task.lock.json',
    };
  });

  afterEach(cleanup);

  it('allows the Bench-bundled a3s-code Candidate', () => {
    const actions = controllerStub();
    renderControls(actions);

    expect(screen.getByText('Candidate 就绪')).toBeInTheDocument();
    expect(screen.getByText('使用 Bench 内置 A3S Code Adapter。')).toBeInTheDocument();
    const launchButton = screen.getByRole('button', { name: /部署评测/ });
    expect(launchButton).toBeEnabled();

    fireEvent.click(launchButton);
    expect(actions.startRun).toHaveBeenCalledOnce();
  });

  it('requires an explicit model route for the bundled A3S Code Adapter', () => {
    labState.hangar.roster[0].model = '';

    renderControls(controllerStub());

    expect(screen.getByRole('alert')).toHaveTextContent('provider/model');
    expect(screen.getByRole('button', { name: /需配置 Adapter/ })).toBeDisabled();
  });

  it('blocks deployment when the active Codex preset has no Candidate Adapter', () => {
    const codexEntry = labState.hangar.roster.find((entry) => entry.pilotId === 'codex');
    expect(codexEntry).toBeDefined();
    if (!codexEntry) return;
    labState.hangar.activeEntryId = codexEntry.id;
    const actions = controllerStub();

    renderControls(actions);

    expect(screen.getByRole('alert')).toHaveTextContent('需配置 Candidate Adapter');
    expect(screen.getByRole('alert')).toHaveTextContent('本地相对路径或 oci:// 引用');
    const launchButton = screen.getByRole('button', { name: /需配置 Adapter/ });
    expect(launchButton).toBeDisabled();
    fireEvent.click(launchButton);
    expect(actions.startRun).not.toHaveBeenCalled();
  });

  it('rejects bare Candidate aliases instead of presenting them as runnable', () => {
    const activeEntry = labState.hangar.roster[0];
    activeEntry.candidate = 'codex';

    renderControls(controllerStub());

    expect(screen.getByRole('alert')).toHaveTextContent('Candidate 引用不可用');
    expect(screen.getByRole('alert')).toHaveTextContent('仅支持 a3s-code、本地相对路径或 oci://');
    expect(screen.getByRole('button', { name: /需配置 Adapter/ })).toBeDisabled();
  });

  it('allows a local Adapter reference while stating that Bench will validate it', () => {
    const activeEntry = labState.hangar.roster[0];
    activeEntry.candidate = './agents/codex-gpt-5.6';

    renderControls(controllerStub());

    expect(screen.getByText('Candidate 待 Bench 校验')).toBeInTheDocument();
    expect(screen.getByText('本地 Adapter 将由 Bench 在部署时校验。')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /部署评测/ })).toBeEnabled();
  });

  it('validates locked mode from the two lock inputs instead of the visual preset Candidate', () => {
    const activeEntry = labState.hangar.roster[0];
    activeEntry.candidate = 'codex';
    labState.runConfig.locked = true;
    const blockedTask = demoTasks.find((task) => task.availability === 'blocked');
    expect(blockedTask).toBeDefined();
    if (!blockedTask) return;

    renderControls(controllerStub(), blockedTask);

    expect(screen.getByText('锁文件就绪')).toBeInTheDocument();
    expect(screen.getByText(/Candidate Lock 与 Task Lock 将由 Bench/)).toBeInTheDocument();
    expect(screen.getByRole('note')).toHaveTextContent('当前地图与天气仅为场景预览');
    expect(screen.getByRole('note')).toHaveTextContent('Bench 返回真实 Task ID 后绑定');
    expect(screen.queryByText('Candidate 引用不可用')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /部署评测/ })).toBeEnabled();
  });

  it('blocks locked deployment until both lock references are present', () => {
    labState.runConfig = {
      candidateLock: '',
      deploymentScope: 'single',
      locked: true,
      taskLock: './task.lock.json',
    };

    renderControls(controllerStub());

    expect(screen.getByRole('alert')).toHaveTextContent('需配置锁文件');
    expect(screen.getByRole('alert')).toHaveTextContent('请先填写 Candidate Lock 文件');
    expect(screen.getByRole('button', { name: /需配置锁文件/ })).toBeDisabled();
  });

  it('does not offer a fake deployment while Bench or Runtime is unavailable', () => {
    labState.connection = { mode: 'preview', message: 'Runtime preflight failed' };

    renderControls(controllerStub());

    const launchButton = screen.getByRole('button', { name: /Bench 未就绪/ });
    expect(launchButton).toBeDisabled();
    expect(launchButton).toHaveTextContent('Runtime preflight failed');
  });

  it('blocks maps that require a Judge until Doctor reports one', () => {
    const task = { ...demoTasks[0], availability_reason: 'requires_configured_judge_model' };

    renderControls(controllerStub(), task);

    const launchButton = screen.getByRole('button', { name: /地图不可部署/ });
    expect(launchButton).toBeDisabled();
    expect(launchButton).toHaveTextContent('Judge 模型');
  });

  it('projects the complete hangar roster into campaign preflight and identifies the blocking aircraft', async () => {
    const actions = controllerStub();
    renderControls(actions);

    fireEvent.click(screen.getByRole('button', { name: /全编队 · 3/ }));

    expect(actions.setDeploymentScope).toHaveBeenCalledWith('campaign');
    await waitFor(() => expect(labState.runConfig.deploymentScope).toBe('campaign'));
    expect(screen.getByRole('alert')).toHaveTextContent('CODEX-01 无法部署');
    expect(screen.getByRole('button', { name: /编队不可部署/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /选择 A3S-01/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /选择 CODEX-01/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /选择 CLAUDE-01/ })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /选择 CODEX-01/ }));
    expect(labState.hangar.activeEntryId).toBe(DEFAULT_HANGAR_ROSTER[1].id);
  });

  it('starts the real Campaign path only after every hangar member is deployable', async () => {
    for (const entry of labState.hangar.roster) {
      entry.candidate = `./agents/${entry.id}`;
      entry.model = '';
    }
    const actions = controllerStub();
    renderControls(actions);

    fireEvent.click(screen.getByRole('button', { name: /全编队 · 3/ }));

    await waitFor(() => expect(labState.runConfig.deploymentScope).toBe('campaign'));
    expect(screen.getByText('编队已就绪')).toBeInTheDocument();
    const launchButton = screen.getByRole('button', { name: /部署编队 · 3/ });
    expect(launchButton).toBeEnabled();
    fireEvent.click(launchButton);
    expect(actions.startCampaign).toHaveBeenCalledOnce();
    expect(actions.startRun).not.toHaveBeenCalled();
  });

  it('disables Campaign in reproducible Lock mode instead of reusing one lock for multiple aircraft', () => {
    labState.runConfig.locked = true;

    renderControls(controllerStub());

    const campaignOption = screen.getByRole('button', { name: /全编队 · 3/ });
    expect(campaignOption).toBeDisabled();
    expect(campaignOption).toHaveAttribute('title', '锁文件模式仅支持单机出击。');
  });

  it('shows only real terminal Campaign progress and locks all deployment configuration while running', () => {
    const actions = controllerStub();
    for (const entry of labState.hangar.roster) entry.candidate = `./agents/${entry.id}`;
    const snapshot = createRunCampaignSnapshot(demoTasks[0], labState.hangar.roster);
    labState.runConfig.deploymentScope = 'campaign';
    labState.campaign = {
      generation: 1,
      status: 'running',
      snapshot,
      members: [
        {
          rosterEntryId: snapshot.roster[0].rosterEntry.id,
          sortie: snapshot.roster[0],
          status: 'completed',
          runId: 'run-a3s',
          result: { status: 'completed', run_id: 'run-a3s', score: '0.91' },
        },
        {
          rosterEntryId: snapshot.roster[1].rosterEntry.id,
          sortie: snapshot.roster[1],
          status: 'failed',
          error: 'Adapter exited',
        },
        {
          rosterEntryId: snapshot.roster[2].rosterEntry.id,
          sortie: snapshot.roster[2],
          status: 'running',
          stage: 'candidate_running',
        },
      ],
    };

    renderControls(actions);

    expect(screen.getByText('编队执行中 · 2/3 返回')).toBeInTheDocument();
    expect(screen.getByText('67%')).toBeInTheDocument();
    expect(screen.getByText('0.91')).toBeInTheDocument();
    expect(screen.getByText('Adapter exited')).toBeInTheDocument();
    const stopTrackingButton = screen.getByRole('button', { name: /停止前端跟踪/ });
    expect(stopTrackingButton).toBeEnabled();
    expect(stopTrackingButton).toHaveTextContent('不会取消 Bench 进程');
    expect(stopTrackingButton).toHaveTextContent('真实终态 2/3 · 0 跟踪已停止 · 1 执行中');
    expect(stopTrackingButton.getAttribute('title')).toContain('不会取消已经提交的 Bench 进程');
    fireEvent.click(stopTrackingButton);
    expect(actions.stopCampaignTracking).toHaveBeenCalledOnce();
    expect(screen.getByRole('button', { name: /空载/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /单机先锋/ })).toBeDisabled();
    expect(screen.getByRole('textbox', { name: 'CANDIDATE' })).toBeDisabled();
  });

  it('treats stopped Campaign tracking as unknown rather than a terminal 100 percent result', () => {
    for (const entry of labState.hangar.roster) entry.candidate = `./agents/${entry.id}`;
    const snapshot = createRunCampaignSnapshot(demoTasks[0], labState.hangar.roster);
    labState.runConfig.deploymentScope = 'campaign';
    labState.campaign = {
      generation: 2,
      status: 'tracking_stopped',
      snapshot,
      members: [
        {
          rosterEntryId: snapshot.roster[0].rosterEntry.id,
          sortie: snapshot.roster[0],
          status: 'completed',
          runId: 'run-a3s',
          result: { status: 'completed', run_id: 'run-a3s', score: '0.91' },
        },
        {
          rosterEntryId: snapshot.roster[1].rosterEntry.id,
          sortie: snapshot.roster[1],
          status: 'failed',
          error: 'Adapter exited',
        },
        {
          rosterEntryId: snapshot.roster[2].rosterEntry.id,
          sortie: snapshot.roster[2],
          status: 'tracking_stopped',
          jobId: 'job-claude',
        },
      ],
    };

    const { container } = renderControlsWithContainer(controllerStub());

    expect(screen.getByText('跟踪已停止 · 1 架状态未知')).toBeInTheDocument();
    expect(screen.getByText('状态未知')).toBeInTheDocument();
    expect(screen.queryByText('100%')).not.toBeInTheDocument();
    expect(container.querySelector('.sortie-progress')).toHaveClass('is-tracking-stopped');
    expect(container.querySelector('.sortie-progress i')).not.toHaveAttribute('style');
  });

  it('uses indeterminate progress for every nonterminal single-run stage', () => {
    expect(runProgress('idle')).toBe(0);
    expect(runProgress('planned')).toBeUndefined();
    expect(runProgress('running')).toBeUndefined();
    expect(runProgress('runtime_ready')).toBeUndefined();
    expect(runProgress('candidate_running')).toBeUndefined();
    expect(runProgress('judging')).toBeUndefined();
    expect(runProgress('completed')).toBe(100);
    expect(runProgress('failed')).toBe(100);
    expect(runProgress('running', true)).toBeUndefined();

    labState.run = { stage: 'running', jobId: 'job-live' };
    const { container } = renderControlsWithContainer(controllerStub());

    expect(screen.getByText('执行中')).toBeInTheDocument();
    expect(screen.getByText('实时状态')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /评测进行中/ })).toHaveTextContent('Bench Job 正在执行');
    expect(container.querySelector('.sortie-progress')).toHaveClass('is-indeterminate');
    expect(container.querySelector('.sortie-progress i')).not.toHaveAttribute('style');
  });

  it('shows interrupted tracking explicitly without treating the old Job as active', () => {
    const rosterEntry = labState.hangar.roster[0];
    const sortie = createRunSortieSnapshot(demoTasks[0], rosterEntry, {
      task: demoTasks[0].id,
      candidate: rosterEntry.candidate,
      model: rosterEntry.model,
      locked: false,
    });
    labState.run = {
      stage: 'running',
      jobId: 'job-interrupted',
      trackingStatus: 'tracking_stopped',
      trackingStoppedAt: '2026-07-17T01:00:00.000Z',
      sortie,
    };

    const { container } = renderControlsWithContainer(controllerStub());

    expect(screen.getByText('跟踪已停止 · Job 状态未知')).toBeInTheDocument();
    expect(screen.getByText('跟踪已停止')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /部署评测/ })).toBeEnabled();
    expect(container.querySelector('.sortie-progress')).toHaveClass('is-tracking-stopped');
    expect(container.querySelector('.sortie-progress')).not.toHaveClass('is-indeterminate');
  });
});

function renderControls(actions: BenchController, task: BenchTask = demoTasks[0]): void {
  render(<TaskRunControls actions={actions} task={task} />);
}

function renderControlsWithContainer(actions: BenchController, task: BenchTask = demoTasks[0]) {
  return render(<TaskRunControls actions={actions} task={task} />);
}

function controllerStub(): BenchController {
  return {
    refresh: vi.fn().mockResolvedValue(undefined),
    selectTask: vi.fn().mockResolvedValue(undefined),
    setQuery: vi.fn(),
    setCategory: vi.fn(),
    setIncludeBlocked: vi.fn(),
    setCandidate: vi.fn(),
    setCandidateLock: vi.fn(),
    setModel: vi.fn(),
    setEffort: vi.fn(),
    setTaskLock: vi.fn(),
    setLocked: vi.fn(),
    setDeploymentScope: vi.fn((scope) => {
      labState.runConfig.deploymentScope = scope;
    }),
    setTunnelParameter: vi.fn() as BenchController['setTunnelParameter'],
    resetTunnel: vi.fn(),
    startRun: vi.fn().mockResolvedValue(undefined),
    startCampaign: vi.fn().mockResolvedValue(true),
    stopCampaignTracking: vi.fn().mockReturnValue(true),
    dismissNotice: vi.fn(),
  };
}
