import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { BenchCampaignMemberStatus, BenchRunStage } from '../../../types/bench';
import { CampaignReportList, type CampaignReportMember, type CampaignReportRosterSortie } from './campaign-report-list';

describe('CampaignReportList', () => {
  afterEach(cleanup);

  it('keeps the frozen roster order and opens a completed member by its exact Run ID', () => {
    const onOpenResult = vi.fn();
    const roster = [sortie('lead', 'LEAD-01', 'j-35'), sortie('wing', 'WING-02', 'f-35')];
    const members = [
      member('wing', 'completed', 'run-wing', '84.25'),
      member('lead', 'completed', 'run-lead', '91.50'),
    ];

    render(<CampaignReportList roster={roster} members={members} status='completed' onOpenResult={onOpenResult} />);

    const aircraft = screen.getAllByRole('button');
    expect(aircraft.map((button) => within(button).getByRole('strong').textContent)).toEqual(['LEAD-01', 'WING-02']);
    expect(within(aircraft[0]).getByText('J-35')).toBeInTheDocument();
    expect(within(aircraft[0]).getByText('run-lead')).toBeInTheDocument();
    expect(within(aircraft[1]).getByText('F-35 Lightning II')).toBeInTheDocument();

    fireEvent.click(aircraft[1]);

    expect(onOpenResult).toHaveBeenCalledOnce();
    expect(onOpenResult).toHaveBeenCalledWith('run-wing');
  });

  it('shows real queued, running, and failed facts without manufacturing results or an aggregate score', () => {
    const roster = [
      sortie('queued', 'QUEUE-01', 'j-50'),
      sortie('running', 'RUN-02', 'f-22'),
      sortie('failed', 'FAIL-03', 'j-35'),
      sortie('done', 'DONE-04', 'f-35'),
    ];
    const members = [
      member('queued', 'queued'),
      member('running', 'running', undefined, '99.00', undefined, 'judging'),
      member('failed', 'failed', 'run-failed', '100.00', 'adapter exited with code 7'),
      member('done', 'completed', 'run-done', '76.00'),
    ];

    render(
      <CampaignReportList roster={roster} members={members} status='completed_with_failures' onOpenResult={vi.fn()} />
    );

    expect(screen.getByText('等待跑道')).toBeInTheDocument();
    expect(screen.getByText('裁定中')).toBeInTheDocument();
    expect(screen.getByText('adapter exited with code 7')).toBeInTheDocument();
    expect(screen.getByText('run-failed')).toBeInTheDocument();
    expect(screen.getByLabelText('DONE-04 真实评分')).toHaveTextContent('76.00');
    expect(screen.queryByLabelText('RUN-02 真实评分')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('FAIL-03 真实评分')).not.toBeInTheDocument();
    expect(screen.queryByText('99.00')).not.toBeInTheDocument();
    expect(screen.queryByText('100.00')).not.toBeInTheDocument();
    expect(screen.queryByText(/average|平均分/iu)).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /QUEUE-01/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /RUN-02/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /FAIL-03/ })).toBeEnabled();
  });

  it('does not attribute a score from a result carrying another Run ID', () => {
    const completed = {
      ...member('lead', 'completed', 'run-expected'),
      result: { status: 'completed' as const, run_id: 'run-other', score: '88.00' },
    };

    render(
      <CampaignReportList
        roster={[sortie('lead', 'LEAD-01', 'j-35')]}
        members={[completed]}
        status='completed'
        onOpenResult={vi.fn()}
      />
    );

    expect(screen.getByText('run-expected')).toBeInTheDocument();
    expect(screen.queryByLabelText('LEAD-01 真实评分')).not.toBeInTheDocument();
    expect(screen.queryByText('88.00')).not.toBeInTheDocument();
    expect(screen.getByText('公开战报未返回评分')).toBeInTheDocument();
  });

  it('allows an interrupted member with an exact Run ID to verify only that result', () => {
    const onOpenResult = vi.fn();
    render(
      <CampaignReportList
        roster={[sortie('wing', 'WING-02', 'f-35')]}
        members={[member('wing', 'tracking_stopped', 'run-interrupted')]}
        status='tracking_stopped'
        onOpenResult={onOpenResult}
      />
    );

    const button = screen.getByRole('button', { name: /WING-02.*按该机 Run ID 核验战报/ });
    expect(button).toBeEnabled();
    expect(screen.getByText('已有 Run ID，可按该 ID 核验终态')).toBeInTheDocument();

    fireEvent.click(button);

    expect(onOpenResult).toHaveBeenCalledOnce();
    expect(onOpenResult).toHaveBeenCalledWith('run-interrupted');
  });
});

function sortie(id: string, callsign: string, airframeId: string): CampaignReportRosterSortie {
  return { rosterEntry: { id, callsign, airframeId } };
}

function member(
  rosterEntryId: string,
  status: BenchCampaignMemberStatus,
  runId?: string,
  score?: string,
  error?: string,
  stage?: BenchRunStage
): CampaignReportMember & { result?: { status: BenchRunStage; run_id: string; score?: string } } {
  return {
    rosterEntryId,
    status,
    runId,
    error,
    stage,
    result: score
      ? {
          status: 'completed',
          run_id: runId ?? `result-${rosterEntryId}`,
          score,
        }
      : undefined,
  };
}
