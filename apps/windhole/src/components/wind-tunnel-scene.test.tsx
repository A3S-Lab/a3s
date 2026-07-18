import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_HANGAR_ROSTER } from '../features/hangar/hangar-configuration';
import { createRunCampaignSnapshot, createRunSortieSnapshot, labState } from '../state/lab-state';
import type { BenchCampaignMemberStatus, BenchRunStage } from '../types/bench';
import type { AircraftHoverEvent } from './scene/aircraft-interaction';
import { WindTunnelScene } from './wind-tunnel-scene';

const runtimeHarness = vi.hoisted(() => ({
  onSelect: undefined as ((entryId: string) => void) | undefined,
  onHover: undefined as ((event?: AircraftHoverEvent) => void) | undefined,
  formation: undefined as readonly { instanceId: string }[] | undefined,
  taskId: undefined as string | undefined,
  taskCategory: undefined as string | undefined,
  dispose: vi.fn(),
  resetAircraft: vi.fn(),
  setTask: vi.fn(),
  syncFormation: vi.fn(),
}));

vi.mock('./scene/wind-tunnel-runtime', () => ({
  createWindTunnelRuntime: vi.fn(
    (
      _container: HTMLDivElement,
      options: {
        formation: readonly { instanceId: string }[];
        taskId: string;
        taskCategory?: string;
        onHover: (event?: AircraftHoverEvent) => void;
        onSelect: (entryId: string) => void;
      }
    ) => {
      runtimeHarness.onSelect = options.onSelect;
      runtimeHarness.onHover = options.onHover;
      runtimeHarness.formation = options.formation;
      runtimeHarness.taskId = options.taskId;
      runtimeHarness.taskCategory = options.taskCategory;
      return runtimeHarness;
    }
  ),
}));

describe('WindTunnelScene roster activation bridge', () => {
  beforeEach(() => {
    runtimeHarness.onSelect = undefined;
    runtimeHarness.onHover = undefined;
    runtimeHarness.formation = undefined;
    runtimeHarness.taskId = undefined;
    runtimeHarness.taskCategory = undefined;
    runtimeHarness.dispose.mockClear();
    runtimeHarness.resetAircraft.mockClear();
    runtimeHarness.setTask.mockClear();
    runtimeHarness.syncFormation.mockClear();
    labState.hangar.roster = DEFAULT_HANGAR_ROSTER.map((entry) => ({ ...entry }));
    labState.hangar.activeEntryId = DEFAULT_HANGAR_ROSTER[0].id;
    labState.catalog.selectedTaskId = labState.catalog.tasks[0].id;
    labState.run = { stage: 'idle' };
    labState.campaign = { generation: 0, status: 'idle', members: [] };
  });

  afterEach(cleanup);

  it('reports an interacted roster aircraft to the external activation owner', async () => {
    const onActivateRosterEntry = vi.fn();
    render(<WindTunnelScene onActivateRosterEntry={onActivateRosterEntry} />);
    await waitFor(() => expect(runtimeHarness.onSelect).toBeTypeOf('function'));
    const wingId = DEFAULT_HANGAR_ROSTER[1].id;

    act(() => runtimeHarness.onSelect?.(wingId));

    expect(onActivateRosterEntry).toHaveBeenCalledOnce();
    expect(onActivateRosterEntry).toHaveBeenCalledWith(wingId);
  });

  it('does not activate an external entry during formation sync or for a non-roster id', async () => {
    const onActivateRosterEntry = vi.fn();
    render(<WindTunnelScene onActivateRosterEntry={onActivateRosterEntry} />);
    await waitFor(() => expect(runtimeHarness.onSelect).toBeTypeOf('function'));
    runtimeHarness.syncFormation.mockClear();
    const wingId = DEFAULT_HANGAR_ROSTER[1].id;

    act(() => {
      labState.hangar.activeEntryId = wingId;
    });
    await waitFor(() => expect(runtimeHarness.syncFormation).toHaveBeenCalledWith(expect.any(Array), wingId));
    expect(onActivateRosterEntry).not.toHaveBeenCalled();

    act(() => runtimeHarness.onSelect?.('non-roster-aircraft'));
    expect(onActivateRosterEntry).not.toHaveBeenCalled();
  });

  it('builds the rendered formation only from the hangar roster', async () => {
    labState.hangar.roster = DEFAULT_HANGAR_ROSTER.slice(0, 2).map((entry) => ({ ...entry }));

    render(<WindTunnelScene />);

    await waitFor(() => expect(runtimeHarness.formation).toBeDefined());
    expect(runtimeHarness.formation?.map((aircraft) => aircraft.instanceId)).toEqual(
      labState.hangar.roster.map((entry) => entry.id)
    );
  });

  it('does not render airflow direction labels or arrows over the battlefield', () => {
    const view = render(<WindTunnelScene />);

    expect(view.queryByText(/FREE STREAM/i)).not.toBeInTheDocument();
    expect(view.queryByText(/X\+ FLOW/i)).not.toBeInTheDocument();
    expect(view.container.querySelector('.flow-direction')).not.toBeInTheDocument();
  });

  it('integrates aircraft reset into the selected identity HUD and forwards the click to the runtime', async () => {
    const view = render(<WindTunnelScene />);
    await waitFor(() => expect(runtimeHarness.formation).toBeDefined());

    const reset = view.getByRole('button', { name: '复位当前飞机观察角度' });
    expect(reset.closest('.specimen-label')).toBeInTheDocument();
    expect(view.container.querySelector('.interaction-hint')).not.toBeInTheDocument();

    fireEvent.click(reset);

    expect(runtimeHarness.resetAircraft).toHaveBeenCalledOnce();
    expect(runtimeHarness.resetAircraft).toHaveBeenCalledWith(DEFAULT_HANGAR_ROSTER[0].id);
  });

  it('does not present the selected Task name as an aircraft model when the Adapter owns model resolution', async () => {
    labState.hangar.activeEntryId = DEFAULT_HANGAR_ROSTER[1].id;
    const view = render(<WindTunnelScene />);
    await waitFor(() => expect(runtimeHarness.formation).toBeDefined());

    const identity = view.container.querySelector('.specimen-label');
    expect(identity).not.toBeNull();
    expect(identity).toHaveTextContent('MODEL MANAGED BY ADAPTER');
    expect(identity).not.toHaveTextContent(labState.catalog.tasks[0].name);
  });

  it('keeps runtime theater resolution aligned with the selected Task category', async () => {
    const initialTask = labState.catalog.tasks[0];
    const nextTask = labState.catalog.tasks[1];
    render(<WindTunnelScene />);

    await waitFor(() => expect(runtimeHarness.taskId).toBe(initialTask.id));
    expect(runtimeHarness.taskCategory).toBe(initialTask.category);

    act(() => {
      labState.catalog.selectedTaskId = nextTask.id;
    });
    await waitFor(() => expect(runtimeHarness.setTask).toHaveBeenCalledWith(nextTask.id, nextTask.category));
  });

  it('reports missing Candidate Adapters as NEEDS CONFIG in the idle aircraft HUD', async () => {
    const view = render(<WindTunnelScene />);
    await waitFor(() => expect(runtimeHarness.onHover).toBeTypeOf('function'));

    hoverAircraft(DEFAULT_HANGAR_ROSTER[1].id);

    expect(view.getByText('NEEDS CONFIG')).toBeInTheDocument();
    expect(view.getByText(/需配置 Candidate Adapter/)).toBeInTheDocument();
    expect(view.queryByText('READY')).not.toBeInTheDocument();
  });

  it('uses full Candidate run readiness, including the bundled Adapter model requirement', async () => {
    labState.hangar.roster[0].model = '';
    const view = render(<WindTunnelScene />);
    await waitFor(() => expect(runtimeHarness.onHover).toBeTypeOf('function'));

    hoverAircraft(DEFAULT_HANGAR_ROSTER[0].id);

    expect(view.getByText('NEEDS CONFIG')).toBeInTheDocument();
    expect(view.getByText(/A3S Code Adapter 需要配置可用的 provider\/model/)).toBeInTheDocument();
  });

  it('projects a single-run result only onto its exact frozen roster entry and map', async () => {
    startSingleRun('completed');
    labState.hangar.activeEntryId = DEFAULT_HANGAR_ROSTER[1].id;
    const view = render(<WindTunnelScene />);
    await waitFor(() => expect(runtimeHarness.onHover).toBeTypeOf('function'));

    hoverAircraft(DEFAULT_HANGAR_ROSTER[0].id);
    expect(view.getByText('COMPLETE')).toBeInTheDocument();
    expect(view.getByText('SCORE 94.25')).toBeInTheDocument();

    hoverAircraft(DEFAULT_HANGAR_ROSTER[1].id);
    expect(view.getByText('NEEDS CONFIG')).toBeInTheDocument();
    expect(view.queryByText('COMPLETE')).not.toBeInTheDocument();

    act(() => {
      labState.catalog.selectedTaskId = labState.catalog.tasks[1].id;
    });
    hoverAircraft(DEFAULT_HANGAR_ROSTER[0].id);
    expect(view.getByText('READY')).toBeInTheDocument();
    expect(view.queryByText('SCORE 94.25')).not.toBeInTheDocument();
  });

  it('shows restored single-run tracking interruption only while the exact frozen configuration still matches', async () => {
    startSingleRun('tracking_stopped');
    const view = render(<WindTunnelScene />);
    await waitFor(() => expect(runtimeHarness.onHover).toBeTypeOf('function'));

    hoverAircraft(DEFAULT_HANGAR_ROSTER[0].id);
    expect(view.getByText('TRACKING STOPPED')).toBeInTheDocument();
    expect(view.getByText('前端跟踪已停止，Bench Job 可能仍在运行')).toBeInTheDocument();

    act(() => {
      labState.hangar.roster[0].effort = 'minimal';
    });
    hoverAircraft(DEFAULT_HANGAR_ROSTER[0].id);
    expect(view.getByText('READY')).toBeInTheDocument();
    expect(view.queryByText('TRACKING STOPPED')).not.toBeInTheDocument();
  });

  it('keeps aircraft selection local while a Campaign is running', async () => {
    const onActivateRosterEntry = vi.fn();
    const view = render(<WindTunnelScene onActivateRosterEntry={onActivateRosterEntry} />);
    await waitFor(() => expect(runtimeHarness.onSelect).toBeTypeOf('function'));
    const wing = DEFAULT_HANGAR_ROSTER[1];

    act(() => startCampaign('queued'));
    act(() => runtimeHarness.onSelect?.(wing.id));

    expect(onActivateRosterEntry).not.toHaveBeenCalled();
    expect(labState.hangar.activeEntryId).toBe(DEFAULT_HANGAR_ROSTER[0].id);
    expect(view.getByText('SELECTED SPECIMEN // CODEX')).toBeInTheDocument();
  });

  it('projects every Campaign member state onto the matching roster aircraft HUD', async () => {
    const view = render(<WindTunnelScene />);
    await waitFor(() => expect(runtimeHarness.onHover).toBeTypeOf('function'));
    const entryId = DEFAULT_HANGAR_ROSTER[0].id;
    act(() => startCampaign('queued'));
    hoverAircraft(entryId);
    expect(view.getByText('QUEUED')).toBeInTheDocument();
    expect(view.getByText('等待跑道')).toBeInTheDocument();

    setCampaignMember('starting');
    await waitFor(() => expect(view.getByText('STARTING')).toBeInTheDocument());
    expect(view.getByText('正在提交到 Bench')).toBeInTheDocument();

    setCampaignMember('running', { stage: 'judging' });
    await waitFor(() => expect(view.getByText('RUNNING')).toBeInTheDocument());
    expect(view.getByText('JUDGING')).toBeInTheDocument();

    setCampaignMember('completed', { stage: 'completed', score: '87.25' });
    await waitFor(() => expect(view.getByText('COMPLETE')).toBeInTheDocument());
    expect(view.getByText('SCORE 87.25')).toBeInTheDocument();

    setCampaignMember('failed', { stage: 'failed', error: 'candidate adapter failed' });
    await waitFor(() => expect(view.getByText('FAILED')).toBeInTheDocument());
    expect(view.getByText('candidate adapter failed')).toBeInTheDocument();

    setCampaignMember('tracking_stopped');
    await waitFor(() => expect(view.getByText('TRACKING STOPPED')).toBeInTheDocument());
    expect(view.getByText('前端跟踪已停止，Bench Job 可能仍在运行')).toBeInTheDocument();
  });

  it('matches a Campaign HUD member by roster entry id instead of member position', async () => {
    const view = render(<WindTunnelScene />);
    await waitFor(() => expect(runtimeHarness.onHover).toBeTypeOf('function'));
    act(() => {
      startCampaign('queued');
      const wingMember = labState.campaign.members[1];
      wingMember.status = 'failed';
      wingMember.stage = 'failed';
      wingMember.error = 'wing adapter failed with exit code 17';
    });

    hoverAircraft(DEFAULT_HANGAR_ROSTER[1].id);

    expect(view.getByText('FAILED')).toBeInTheDocument();
    expect(view.getByText('wing adapter failed with exit code 17')).toBeInTheDocument();
    expect(view.queryByText('等待跑道')).not.toBeInTheDocument();
  });

  it('does not invent a score when a completed Campaign result has none', async () => {
    const view = render(<WindTunnelScene />);
    await waitFor(() => expect(runtimeHarness.onHover).toBeTypeOf('function'));

    act(() => startCampaign('completed'));
    hoverAircraft(DEFAULT_HANGAR_ROSTER[0].id);

    expect(view.getByText('Bench 已完成，但战报未返回评分')).toBeInTheDocument();
    expect(view.queryByText(/^SCORE /)).not.toBeInTheDocument();
  });

  it('does not project a historical Campaign result onto a changed map or aircraft configuration', async () => {
    const view = render(<WindTunnelScene />);
    await waitFor(() => expect(runtimeHarness.onHover).toBeTypeOf('function'));
    const entryId = DEFAULT_HANGAR_ROSTER[0].id;

    act(() => {
      startCampaign('completed');
      const member = labState.campaign.members[0];
      member.result = { status: 'completed', run_id: 'historical-run', score: '99.99' };
      labState.hangar.roster[0].model = 'provider/changed-after-campaign';
    });
    hoverAircraft(entryId);

    expect(view.getByText('READY')).toBeInTheDocument();
    expect(view.queryByText('SCORE 99.99')).not.toBeInTheDocument();
  });
});

function startCampaign(status: BenchCampaignMemberStatus): void {
  const snapshot = createRunCampaignSnapshot(labState.catalog.tasks[0], labState.hangar.roster);
  labState.campaign = {
    generation: 1,
    status:
      status === 'tracking_stopped'
        ? 'tracking_stopped'
        : status === 'completed'
          ? 'completed'
          : status === 'failed'
            ? 'failed'
            : 'running',
    snapshot,
    members: snapshot.roster.map((sortie) => ({
      rosterEntryId: sortie.rosterEntry.id,
      sortie,
      status,
      result:
        status === 'completed'
          ? {
              status: 'completed',
              run_id: `run-${sortie.rosterEntry.id}`,
            }
          : undefined,
    })),
  };
}

function startSingleRun(status: 'completed' | 'tracking_stopped'): void {
  const task = labState.catalog.tasks.find((entry) => entry.id === labState.catalog.selectedTaskId);
  const entry = labState.hangar.roster[0];
  if (!task || !entry) throw new Error('Single-run HUD test requires one selected Task and roster entry');
  const sortie = createRunSortieSnapshot(task, entry, {
    task: task.id,
    candidate: entry.candidate,
    model: entry.model || undefined,
    locked: false,
  });
  labState.run = {
    mode: 'live',
    stage: status === 'completed' ? 'completed' : 'candidate_running',
    trackingStatus: status === 'tracking_stopped' ? 'tracking_stopped' : undefined,
    trackingStoppedAt: status === 'tracking_stopped' ? '2026-07-17T00:04:00.000Z' : undefined,
    jobId: 'job-single-hud',
    runId: status === 'completed' ? 'run-single-hud' : undefined,
    startedAt: '2026-07-17T00:00:00.000Z',
    completedAt: status === 'completed' ? '2026-07-17T00:03:00.000Z' : undefined,
    sortie,
    result:
      status === 'completed'
        ? { status: 'completed', run_id: 'run-single-hud', task_id: task.id, score: '94.25' }
        : undefined,
  };
}

function setCampaignMember(
  status: BenchCampaignMemberStatus,
  options: { stage?: BenchRunStage; score?: string; error?: string } = {}
): void {
  act(() => {
    const member = labState.campaign.members[0];
    member.status = status;
    member.stage = options.stage;
    member.error = options.error;
    member.result =
      options.score === undefined
        ? undefined
        : {
            status: 'completed',
            run_id: `run-${member.rosterEntryId}`,
            score: options.score,
          };
  });
}

function hoverAircraft(entryId: string): void {
  act(() => {
    runtimeHarness.onHover?.({
      id: entryId,
      x: 120,
      y: 80,
      placement: 'right',
      hitPart: 'fuselage',
    });
  });
}
