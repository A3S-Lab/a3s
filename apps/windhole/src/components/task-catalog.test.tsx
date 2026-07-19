import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { demoTasks } from '../data/demo-tasks';
import type { BenchController } from '../features/bench/use-bench-controller';
import { DEFAULT_HANGAR_ROSTER } from '../features/hangar/hangar-configuration';
import { labState } from '../state/lab-state';
import { missionDifficulty, TaskCatalog } from './task-catalog';

describe('TaskCatalog mission map', () => {
  beforeEach(() => {
    labState.catalog.tasks = demoTasks.map((task) => ({ ...task }));
    labState.catalog.selectedTaskId = demoTasks[0].id;
    labState.catalog.query = '';
    labState.catalog.category = 'all';
    labState.catalog.includeBlocked = false;
    labState.connection.mode = 'preview';
    labState.run.stage = 'idle';
    labState.run.result = undefined;
    labState.campaign = { generation: 0, status: 'idle', members: [] };
    labState.hangar.roster = DEFAULT_HANGAR_ROSTER.map((entry) => ({ ...entry }));
    labState.hangar.activeEntryId = DEFAULT_HANGAR_ROSTER[0].id;
    labState.runConfig.locked = false;
    labState.runConfig.deploymentScope = 'single';
    const activeEntry = labState.hangar.roster.find((entry) => entry.id === labState.hangar.activeEntryId);
    if (activeEntry) activeEntry.effort = 'medium';
  });

  afterEach(cleanup);

  it('selects the exact Bench task represented by a map card', () => {
    const actions = controllerStub();
    render(<TaskCatalog actions={actions} />);

    fireEvent.click(screen.getByRole('button', { name: /ANN Vector Search QPS/ }));

    expect(actions.selectTask).toHaveBeenCalledWith('ann_vector_search_qps');
    expect(screen.getByText('选择作战地图')).toBeInTheDocument();
    expect(screen.getByText('战区关卡')).toBeInTheDocument();
  });

  it('uses the shared battlefield theater profile for level identity', () => {
    const { container } = render(<TaskCatalog actions={controllerStub()} />);

    expect(screen.getAllByText(/鹰隘训练基地/).length).toBeGreaterThan(0);
    expect(screen.getByText(/翡翠海岸 ·/)).toBeInTheDocument();
    expect(container.querySelector('[data-theater="training-range"]')).toBeInTheDocument();
    expect(container.querySelector('[data-theater="littoral-front"]')).toBeInTheDocument();
  });

  it('keeps Candidate fields in a collapsed advanced configuration disclosure', () => {
    render(<TaskCatalog actions={controllerStub()} />);

    const sortieSummary = screen.getByText('出击配置').closest('summary');
    expect(sortieSummary?.parentElement).not.toHaveAttribute('open');
    if (sortieSummary) fireEvent.click(sortieSummary);
    const advancedSummary = screen.getByText('高级配置').closest('summary');
    expect(advancedSummary?.parentElement).not.toHaveAttribute('open');
    expect(screen.getByRole('button', { name: /满载/ })).toBeInTheDocument();
  });

  it('shows the exact active hangar combination in the selected map briefing', () => {
    const codex = labState.hangar.roster[1];
    labState.hangar.activeEntryId = codex.id;

    const { container } = render(<TaskCatalog actions={controllerStub()} />);

    const combination = container.querySelector('.active-sortie-combination');
    expect(combination).toBeInstanceOf(HTMLElement);
    const combinationQueries = within(combination as HTMLElement);
    expect(combinationQueries.getByText('当前出击组合')).toBeInTheDocument();
    expect(combinationQueries.getByText(codex.callsign)).toBeInTheDocument();
    expect(combinationQueries.getByText(/F-35 Lightning II · Codex Systems Pilot/)).toBeInTheDocument();
  });

  it('derives honest map difficulty from availability and admission', () => {
    const blockedTask = demoTasks.find((task) => task.availability === 'blocked');
    expect(blockedTask).toBeDefined();
    if (!blockedTask) return;

    expect(missionDifficulty(demoTasks[0])).toEqual({ label: '训练', tone: 'low' });
    expect(missionDifficulty(demoTasks[1])).toEqual({ label: '高危', tone: 'high' });
    expect(missionDifficulty(blockedTask)).toEqual({ label: '封锁', tone: 'locked' });
  });

  it('locks map selection while the frozen sortie is running', () => {
    const actions = controllerStub();
    labState.run = { stage: 'candidate_running' };
    render(<TaskCatalog actions={actions} />);

    const mapButton = screen.getByRole('button', { name: /ANN Vector Search QPS/ });
    expect(mapButton).toBeDisabled();
    fireEvent.click(mapButton);
    expect(actions.selectTask).not.toHaveBeenCalled();
  });

  it('locks map selection while a frozen Campaign formation is running', () => {
    const actions = controllerStub();
    labState.campaign = { generation: 1, status: 'running', members: [] };
    render(<TaskCatalog actions={actions} />);

    const mapButton = screen.getByRole('button', { name: /ANN Vector Search QPS/ });
    expect(mapButton).toBeDisabled();
    fireEvent.click(mapButton);
    expect(actions.selectTask).not.toHaveBeenCalled();
  });
});

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
    setDeploymentScope: vi.fn(),
    setTunnelParameter: vi.fn() as BenchController['setTunnelParameter'],
    resetTunnel: vi.fn(),
    startRun: vi.fn().mockResolvedValue(undefined),
    startCampaign: vi.fn().mockResolvedValue(true),
    stopCampaignTracking: vi.fn().mockReturnValue(true),
    dismissNotice: vi.fn(),
  };
}
