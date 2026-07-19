import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { BenchCampaignMemberStatus, BenchRunResult } from '../../../types/bench';
import { createHangarDraft, type HangarRosterEntry } from '../../hangar/hangar-configuration';
import { CampaignRosterStrip, type CampaignRosterMemberView } from './campaign-roster-strip';

describe('CampaignRosterStrip', () => {
  afterEach(cleanup);

  it('keeps hangar roster order, projects Candidate readiness, and selects the clicked aircraft', () => {
    const roster = [
      rosterEntry('lead', 'LEAD-01', 'a3s-code', 'zai/glm-5.2'),
      rosterEntry('wing', 'WING-02', '', ''),
      rosterEntry('tail', 'TAIL-03', './agents/tail', ''),
    ];
    const onSelectEntry = vi.fn();

    render(<CampaignRosterStrip roster={roster} activeEntryId='wing' onSelectEntry={onSelectEntry} />);

    const buttons = screen.getAllByRole('button');
    expect(buttons.map((button) => within(button).getByRole('strong').textContent)).toEqual([
      'LEAD-01',
      'WING-02',
      'TAIL-03',
    ]);
    expect(within(buttons[0]).getByText('READY')).toBeInTheDocument();
    expect(within(buttons[1]).getByText('NEEDS CONFIG')).toBeInTheDocument();
    expect(within(buttons[2]).getByText('READY')).toBeInTheDocument();
    expect(buttons[1]).toHaveAttribute('aria-pressed', 'true');

    fireEvent.click(buttons[2]);
    expect(onSelectEntry).toHaveBeenCalledOnce();
    expect(onSelectEntry).toHaveBeenCalledWith('tail');
  });

  it('shows authoritative campaign states in roster order and never invents a failed score', () => {
    const roster = [
      rosterEntry('queued', 'QUEUE-01'),
      rosterEntry('starting', 'START-02'),
      rosterEntry('running', 'RUN-03'),
      rosterEntry('completed', 'DONE-04'),
      rosterEntry('failed', 'FAIL-05'),
    ];
    const members: CampaignRosterMemberView[] = [
      member('failed', 'failed', { score: '99.99' }, 'candidate adapter failed'),
      member('completed', 'completed', { score: '87.25' }),
      member('running', 'running', undefined, undefined, 'candidate_running'),
      member('starting', 'starting'),
      member('queued', 'queued'),
    ];
    render(<CampaignRosterStrip roster={roster} campaignMembers={members} onSelectEntry={vi.fn()} />);

    const buttons = screen.getAllByRole('button');
    expect(within(buttons[0]).getByText('QUEUED')).toBeInTheDocument();
    expect(within(buttons[1]).getByText('STARTING')).toBeInTheDocument();
    expect(within(buttons[2]).getByText('RUNNING')).toBeInTheDocument();
    expect(within(buttons[2]).getByText('CANDIDATE RUNNING')).toBeInTheDocument();
    expect(within(buttons[3]).getByText('COMPLETED')).toBeInTheDocument();
    expect(within(buttons[3]).getByLabelText('DONE-04 \u771f\u5b9e\u8bc4\u5206')).toHaveTextContent('87.25');
    expect(within(buttons[4]).getByText('FAILED')).toBeInTheDocument();
    expect(within(buttons[4]).getByText('candidate adapter failed')).toBeInTheDocument();
    expect(within(buttons[4]).queryByText('99.99')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('FAIL-05 \u771f\u5b9e\u8bc4\u5206')).not.toBeInTheDocument();
  });

  it('distinguishes stopped tracking from a failed Bench run', () => {
    const roster = [rosterEntry('stopped', 'HOLD-01')];

    render(
      <CampaignRosterStrip
        roster={roster}
        campaignMembers={[member('stopped', 'tracking_stopped')]}
        onSelectEntry={vi.fn()}
      />
    );

    const aircraft = screen.getByRole('button', { name: /\u9009\u62e9 HOLD-01/ });
    expect(within(aircraft).getByText('TRACKING STOPPED')).toBeInTheDocument();
    expect(within(aircraft).getByText(/Bench Job \u53ef\u80fd\u4ecd\u5728\u8fd0\u884c/)).toBeInTheDocument();
    expect(within(aircraft).queryByText('FAILED')).not.toBeInTheDocument();
  });

  it('does not fabricate a score when a completed result omits it', () => {
    const roster = [rosterEntry('completed', 'DONE-01')];

    render(
      <CampaignRosterStrip
        roster={roster}
        campaignMembers={[member('completed', 'completed', {})]}
        onSelectEntry={vi.fn()}
      />
    );

    expect(screen.getByText(/\u672a\u8fd4\u56de\u8bc4\u5206/)).toBeInTheDocument();
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    expect(screen.queryByText(/--|0\.00/)).not.toBeInTheDocument();
  });
});

function rosterEntry(id: string, callsign: string, candidate = 'a3s-code', model = 'zai/glm-5.2'): HangarRosterEntry {
  return {
    id,
    ...createHangarDraft('a3s'),
    callsign,
    candidate,
    model,
  };
}

function member(
  rosterEntryId: string,
  status: BenchCampaignMemberStatus,
  resultPatch?: Partial<BenchRunResult>,
  error?: string,
  stage?: CampaignRosterMemberView['stage']
): CampaignRosterMemberView {
  return {
    rosterEntryId,
    status,
    error,
    stage,
    result: resultPatch
      ? {
          status: 'completed',
          run_id: `run-${rosterEntryId}`,
          ...resultPatch,
        }
      : undefined,
  };
}
