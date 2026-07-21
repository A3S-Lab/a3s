import { describe, expect, it, vi } from 'vitest';
import {
  createWorkAgentProposalRequest,
  WORK_AGENT_PROPOSAL_PROTOCOL,
  workAgentProposalInstruction,
  workAgentProposalStatus,
} from './work-agent-proposal';

describe('Work agent proposal protocol', () => {
  it('adds a bounded target manifest and request identifier to the Copilot draft', () => {
    const request = proposalRequest();

    const instruction = workAgentProposalInstruction('请改写。', request);

    expect(instruction).toContain('请改写。');
    expect(instruction).toContain(request.id);
    expect(instruction).toContain(WORK_AGENT_PROPOSAL_PROTOCOL);
    expect(instruction).toContain('"targetId": "selection"');
    expect(instruction).toContain('"before": "旧内容"');
  });

  it('does not create an apply manifest for an oversized source value', () => {
    const request = createWorkAgentProposalRequest({
      id: 'work-proposal-large',
      title: '审阅改写',
      description: '大段文本',
      targets: [{ id: 'selection', label: '选中文本', before: 'a'.repeat(12_001) }],
      apply: vi.fn(() => ({ appliedTargetIds: [], conflicts: [] })),
    });

    expect(request.targets).toEqual([]);
    expect(workAgentProposalInstruction('请改写。', request)).toBe('请改写。');
  });

  it('waits for the matching sent request before accepting a structured response', () => {
    const request = proposalRequest();

    expect(workAgentProposalStatus([], request)).toEqual({ state: 'waiting', phase: 'draft' });
    expect(
      workAgentProposalStatus(
        [
          { role: 'user', content: `请求 ID：${request.id}` },
          { role: 'assistant', content: '', pending: true },
        ],
        request
      )
    ).toEqual({ state: 'waiting', phase: 'response' });
  });

  it('maps a matching assistant payload back to trusted current values', () => {
    const request = proposalRequest();
    const status = workAgentProposalStatus(
      [
        { role: 'user', content: `请求 ID：${request.id}` },
        {
          role: 'assistant',
          content: [
            '建议如下。',
            '```json',
            JSON.stringify({
              protocol: WORK_AGENT_PROPOSAL_PROTOCOL,
              requestId: request.id,
              summary: '表达更简洁',
              changes: [{ targetId: 'selection', after: '新内容', reason: '删除重复表达' }],
            }),
            '```',
          ].join('\n'),
        },
      ],
      request
    );

    expect(status).toEqual({
      state: 'ready',
      proposal: {
        requestId: request.id,
        summary: '表达更简洁',
        changes: [
          {
            id: 'selection',
            label: '选中文本',
            before: '旧内容',
            after: '新内容',
            reason: '删除重复表达',
          },
        ],
      },
    });
  });

  it('rejects payloads that target content outside the approved manifest', () => {
    const request = proposalRequest();
    const status = workAgentProposalStatus(
      [
        { role: 'user', content: `请求 ID：${request.id}` },
        {
          role: 'assistant',
          content: JSON.stringify({
            protocol: WORK_AGENT_PROPOSAL_PROTOCOL,
            requestId: request.id,
            changes: [{ targetId: 'outside-selection', after: '不安全修改' }],
          }),
        },
      ],
      request
    );

    expect(status.state).toBe('invalid');
  });
});

function proposalRequest() {
  return createWorkAgentProposalRequest({
    id: 'work-proposal-test',
    title: '审阅改写',
    description: '选中文本',
    targets: [{ id: 'selection', label: '选中文本', before: '旧内容' }],
    apply: vi.fn(() => ({ appliedTargetIds: [], conflicts: [] })),
  });
}
