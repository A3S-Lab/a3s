import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createWorkAgentProposalRequest } from '../work-agent-proposal';
import { WorkAgentProposalReview } from './work-agent-proposal-review';

describe('Work agent proposal review', () => {
  afterEach(cleanup);

  it('requires explicit selection before applying a subset of verified changes', () => {
    const apply = vi.fn(() => ({ appliedTargetIds: ['A1'], conflicts: [] }));
    const request = createWorkAgentProposalRequest({
      id: 'proposal-review-test',
      title: '审阅表格修改',
      description: '预算!A1:B1',
      targets: [
        { id: 'A1', label: '预算!A1', before: '10' },
        { id: 'B1', label: '预算!B1', before: '20' },
      ],
      apply,
    });
    render(
      <WorkAgentProposalReview
        request={request}
        status={{
          state: 'ready',
          proposal: {
            requestId: request.id,
            summary: '修正两处预算数据',
            changes: [
              { id: 'A1', label: '预算!A1', before: '10', after: '12', reason: '更新预测' },
              { id: 'B1', label: '预算!B1', before: '20', after: '24', reason: '更新预测' },
            ],
          },
        }}
        onDismiss={vi.fn()}
      />
    );

    const checkboxes = screen.getAllByRole('checkbox');
    expect(checkboxes).toHaveLength(2);
    fireEvent.click(checkboxes[1]);
    fireEvent.click(screen.getByRole('button', { name: '应用 1 项' }));

    expect(apply).toHaveBeenCalledWith([{ id: 'A1', label: '预算!A1', before: '10', after: '12', reason: '更新预测' }]);
    expect(screen.getByText('已应用 1 项。')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '完成' })).toBeInTheDocument();
  });
});
