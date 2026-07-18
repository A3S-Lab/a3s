import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { demoTasks } from '../../../data/demo-tasks';
import { createRunCampaignSnapshot, createRunSortieSnapshot, labState } from '../../../state/lab-state';
import { DEFAULT_HANGAR_ROSTER } from '../../hangar/hangar-configuration';
import type { ResultController } from '../use-result-controller';
import { ResultsWorkspace } from './results-workspace';

const actions: ResultController = {
  setRunId: vi.fn(),
  loadResult: vi.fn(async () => undefined),
  loadLatest: vi.fn(async () => undefined),
  openCurrentRun: vi.fn(async () => undefined),
};

afterEach(() => {
  vi.clearAllMocks();
  labState.results = { runId: '', loading: false };
  labState.run = { stage: 'idle' };
  labState.campaign = { generation: 0, status: 'idle', members: [] };
});

describe('ResultsWorkspace', () => {
  it('renders a nonterminal run journal without inventing a score', () => {
    labState.results.record = {
      status: 'judging',
      run_id: 'local-123',
      task_reference: './task',
    };

    render(<ResultsWorkspace actions={actions} />);

    expect(screen.getByRole('heading', { name: '战绩大厅' })).toBeInTheDocument();
    expect(screen.getByText('战报仍在生成')).toBeInTheDocument();
    expect(screen.getByText('./task')).toBeInTheDocument();
    expect(screen.queryByText('综合战绩')).not.toBeInTheDocument();
  });

  it('attributes the authoritative score to the frozen map and hangar sortie', () => {
    const result = {
      status: 'completed' as const,
      governance_status: 'local_unofficial' as const,
      run_id: 'local-snapshot-123',
      task_id: demoTasks[0].id,
      score: '1',
      primary_metric: 'correctness',
    };
    const sortie = createRunSortieSnapshot(demoTasks[0], DEFAULT_HANGAR_ROSTER[0], {
      task: demoTasks[0].id,
      candidate: 'a3s-code',
      model: 'anthropic/glm-5.2',
      locked: false,
    });
    labState.run = { stage: 'completed', runId: result.run_id, result, sortie };
    labState.results.record = result;

    render(<ResultsWorkspace actions={actions} />);

    expect(screen.getByText('出击数据快照')).toBeInTheDocument();
    expect(screen.getByText(/J-50 · A3S-01/)).toBeInTheDocument();
    expect(screen.getByText(/A3S Flight Lead/)).toBeInTheDocument();
    expect(screen.getByText(/a3s-code/)).toBeInTheDocument();
    expect(screen.getByText(/地图、飞机和执行输入在部署时冻结/)).toBeInTheDocument();
  });

  it('keeps an exact restored single-run debrief entry while terminal verification is pending', () => {
    const sortie = createRunSortieSnapshot(demoTasks[1], DEFAULT_HANGAR_ROSTER[0], {
      task: demoTasks[1].id,
      candidate: 'a3s-code',
      model: 'anthropic/glm-5.2',
      locked: false,
    });
    labState.run = {
      stage: 'running',
      trackingStatus: 'tracking_stopped',
      runId: 'local-restored-exact-456',
      sortie,
    };

    render(<ResultsWorkspace actions={actions} />);

    const currentRun = screen.getByRole('button', {
      name: new RegExp(
        `${demoTasks[1].name}.*${sortie.rosterEntry.callsign}.*J-50.*local-restored-exact-456.*跟踪已停止.*待精确核验`
      ),
    });
    expect(currentRun).toBeInTheDocument();

    fireEvent.click(currentRun);

    expect(actions.openCurrentRun).toHaveBeenCalledOnce();
    expect(actions.loadLatest).not.toHaveBeenCalled();
  });

  it('does not present nullable optional model usage as zero', () => {
    labState.results.record = {
      status: 'completed',
      run_id: 'local-nullable-usage',
      task_id: demoTasks[0].id,
      score: '1',
      model_usage: {
        prompt_tokens: 101,
        completion_tokens: 11,
        total_tokens: 112,
        cache_read_tokens: null,
        cache_write_tokens: null,
        tool_calls_count: null,
      },
    };

    render(<ResultsWorkspace actions={actions} />);

    expect(screen.getByText('输入 101')).toBeInTheDocument();
    expect(screen.getByText('输出 11')).toBeInTheDocument();
    expect(screen.queryByText(/缓存读取/)).not.toBeInTheDocument();
    expect(screen.queryByText(/缓存写入/)).not.toBeInTheDocument();
    expect(screen.queryByText(/工具调用/)).not.toBeInTheDocument();
  });

  it('opens a campaign member through the ResultController with that member Run ID', () => {
    const roster = [DEFAULT_HANGAR_ROSTER[0], DEFAULT_HANGAR_ROSTER[1]];
    const snapshot = createRunCampaignSnapshot(demoTasks[0], roster);
    const leadResult = {
      status: 'completed' as const,
      run_id: 'run-formation-lead',
      task_id: demoTasks[0].id,
      score: '93.00',
    };
    labState.campaign = {
      generation: 1,
      status: 'completed_with_failures',
      snapshot,
      members: [
        {
          rosterEntryId: roster[0].id,
          sortie: snapshot.roster[0],
          status: 'completed',
          runId: leadResult.run_id,
          result: leadResult,
        },
        {
          rosterEntryId: roster[1].id,
          sortie: snapshot.roster[1],
          status: 'failed',
          error: 'Candidate Adapter 启动失败',
        },
      ],
    };

    render(<ResultsWorkspace actions={actions} />);

    expect(screen.getByRole('heading', { name: '本次编队' })).toBeInTheDocument();
    expect(screen.getByText('Candidate Adapter 启动失败')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /A3S-01，已归档/ }));

    expect(actions.loadResult).toHaveBeenCalledOnce();
    expect(actions.loadResult).toHaveBeenCalledWith('run-formation-lead');
    expect(actions.loadLatest).not.toHaveBeenCalled();
  });
});
