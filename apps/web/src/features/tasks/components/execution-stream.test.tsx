import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../../state/app-state';
import type { TaskActions } from '../task-actions';
import { ExecutionStream } from './execution-stream';

describe('ExecutionStream permission decisions', () => {
  afterEach(() => {
    cleanup();
    appState.messagesLoading = {};
    appState.messageErrors = {};
    appState.streamingSessionId = null;
    appState.composerValue = '';
    appState.composerContextFiles = [];
    appState.composerSkills = [];
    appState.taskSubmissionState = null;
    appState.toolDecisionState = {};
    appState.toolDecisionErrors = {};
  });

  it('hides planner and synthesis messages while keeping the final response', () => {
    appState.activeSessionId = 'session-projection';
    appState.messagesBySession['session-projection'] = [
      {
        id: 'user-visible',
        sessionId: 'session-projection',
        role: 'user',
        content: '修改第2首诗歌的风格',
        createdAt: new Date().toISOString(),
      },
      {
        id: 'assistant-old',
        sessionId: 'session-projection',
        role: 'assistant',
        content: '旧的阶段性回复',
        createdAt: new Date().toISOString(),
      },
      {
        id: 'user-synthesis',
        sessionId: 'session-projection',
        role: 'user',
        content:
          '[synthesis]\nThe previous turn stopped without a final answer.\n\nOriginal user task:\n修改第2首诗歌的风格\n\nWrite the final answer now.',
        createdAt: new Date().toISOString(),
      },
      {
        id: 'assistant-final',
        sessionId: 'session-projection',
        role: 'assistant',
        content: '第2首诗歌已改为婉约风格。',
        createdAt: new Date().toISOString(),
      },
    ];

    render(<ExecutionStream actions={{} as TaskActions} />);

    expect(screen.getByText('修改第2首诗歌的风格')).toBeInTheDocument();
    expect(screen.getByText('第2首诗歌已改为婉约风格。')).toBeInTheDocument();
    expect(screen.queryByText(/Original user task/)).not.toBeInTheDocument();
    expect(screen.queryByText('旧的阶段性回复')).not.toBeInTheDocument();
  });

  it('turns an unanswered persisted instruction into an editable recovery state', () => {
    appState.activeSessionId = 'session-interrupted';
    appState.messagesBySession['session-interrupted'] = [
      {
        id: 'user-interrupted',
        sessionId: 'session-interrupted',
        role: 'user',
        content: '运行重点测试',
        createdAt: new Date().toISOString(),
      },
    ];

    render(<ExecutionStream actions={{} as TaskActions} />);

    expect(screen.getByRole('status', { name: '未完成的任务请求' })).toHaveTextContent('这次请求没有完成');
    fireEvent.click(screen.getByRole('button', { name: '继续编辑' }));
    expect(appState.composerValue).toContain('运行重点测试');
  });

  it('keeps instruction Skills and workspace files visible with local message actions', async () => {
    appState.activeSessionId = 'session-instruction-resources';
    appState.messagesBySession['session-instruction-resources'] = [
      {
        id: 'user-resources',
        sessionId: 'session-instruction-resources',
        role: 'user',
        content:
          '[Selected skills]\n- Use your `review-master` skill.\n[/Selected skills]\n\n[Workspace context files]\n- src/app.ts\n[/Workspace context files]\n\n审阅这次修改',
        createdAt: '2026-07-14T08:00:00.000Z',
      },
      {
        id: 'assistant-resources',
        sessionId: 'session-instruction-resources',
        role: 'assistant',
        content: '审阅完成。',
        createdAt: '2026-07-14T08:01:00.000Z',
      },
    ];
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });

    render(<ExecutionStream actions={{} as TaskActions} />);

    const instruction = screen.getByRole('article', { name: '你的任务指令' });
    expect(within(instruction).getByText('review-master')).toBeInTheDocument();
    expect(within(instruction).getByText('src/app.ts')).toBeInTheDocument();
    expect(instruction).not.toHaveTextContent('Selected skills');
    fireEvent.click(within(instruction).getByRole('button', { name: '复制消息' }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith('审阅这次修改'));
    fireEvent.click(within(instruction).getByRole('button', { name: '继续编辑这条指令' }));
    expect(appState.composerValue).toContain('审阅这次修改');
    expect(appState.composerContextFiles).toEqual(['src/app.ts']);
    expect(appState.composerSkills).toEqual(['review-master']);
  });

  it('anchors every assistant response with a stable Code header', () => {
    appState.activeSessionId = 'session-code-header';
    appState.messagesBySession['session-code-header'] = [
      {
        id: 'user-code-header',
        sessionId: 'session-code-header',
        role: 'user',
        content: '解释这个模块',
        createdAt: '2026-07-14T08:00:00.000Z',
      },
      {
        id: 'assistant-code-header',
        sessionId: 'session-code-header',
        role: 'assistant',
        content: '这是模块说明。',
        createdAt: '2026-07-14T08:01:00.000Z',
      },
    ];

    render(<ExecutionStream actions={{} as TaskActions} />);

    const response = screen.getByRole('article', { name: 'Code 回复' });
    expect(within(response).getByText('Code')).toBeInTheDocument();
    expect(within(response).getByRole('button', { name: '复制消息' })).toBeInTheDocument();
    expect(response.querySelector('time')).toHaveAttribute('datetime', '2026-07-14T08:01:00.000Z');
  });

  it('marks only the latest instruction as the runtime panel collision anchor', () => {
    appState.activeSessionId = 'session-runtime-anchor';
    appState.messagesBySession['session-runtime-anchor'] = [
      {
        id: 'user-runtime-anchor-1',
        sessionId: 'session-runtime-anchor',
        role: 'user',
        content: '先检查消息投影',
        createdAt: '2026-07-14T08:00:00.000Z',
      },
      {
        id: 'assistant-runtime-anchor-1',
        sessionId: 'session-runtime-anchor',
        role: 'assistant',
        content: '消息投影正常。',
        createdAt: '2026-07-14T08:01:00.000Z',
      },
      {
        id: 'user-runtime-anchor-2',
        sessionId: 'session-runtime-anchor',
        role: 'user',
        content: '继续检查运行面板',
        createdAt: '2026-07-14T08:02:00.000Z',
      },
      {
        id: 'assistant-runtime-anchor-2',
        sessionId: 'session-runtime-anchor',
        role: 'assistant',
        content: '运行面板正常。',
        createdAt: '2026-07-14T08:03:00.000Z',
      },
    ];

    render(<ExecutionStream actions={{} as TaskActions} />);

    const instructions = screen.getAllByRole('article', { name: '你的任务指令' });
    expect(instructions[0]).not.toHaveAttribute('data-task-runtime-anchor');
    expect(instructions[1]).toHaveAttribute('data-task-runtime-anchor', 'latest-instruction');
  });

  it('describes live parallel work in the assistant lifecycle instead of showing a generic pending state', () => {
    appState.activeSessionId = 'session-parallel-state';
    appState.messagesBySession['session-parallel-state'] = [
      {
        id: 'assistant-parallel-state',
        sessionId: 'session-parallel-state',
        role: 'assistant',
        content: '',
        createdAt: new Date().toISOString(),
        pending: true,
        events: [
          {
            type: 'subagent_start',
            task_id: 'review',
            session_id: 'child-review',
            agent: 'review',
            description: '检查消息视图',
          },
        ],
      },
    ];

    render(<ExecutionStream actions={{} as TaskActions} />);

    expect(screen.getByText('正在并行执行')).toBeInTheDocument();
    expect(screen.queryByText('正在准备')).not.toBeInTheDocument();
  });

  it('distinguishes a task-load failure from an empty new task and offers retry', () => {
    appState.activeSessionId = 'session-load-error';
    appState.messagesBySession = {};
    appState.messagesLoading = { 'session-load-error': false };
    appState.messageErrors = { 'session-load-error': 'Connection refused' };
    const reloadActiveTask = vi.fn(async () => undefined);

    render(<ExecutionStream actions={{ reloadActiveTask } as unknown as TaskActions} />);
    expect(screen.getByRole('alert')).toHaveTextContent('无法加载任务记录');
    expect(screen.queryByText('交给 Code 一个明确任务')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '重新加载任务' }));
    expect(reloadActiveTask).toHaveBeenCalledTimes(1);
  });

  it('shows first-turn startup feedback instead of the empty task welcome', () => {
    appState.activeSessionId = 'session-starting';
    appState.messagesBySession['session-starting'] = [];
    appState.messagesLoading['session-starting'] = false;
    delete appState.messageErrors['session-starting'];
    appState.taskSubmissionState = 'creating';

    render(<ExecutionStream actions={{} as TaskActions} />);

    expect(screen.getByRole('status')).toHaveTextContent('正在启动任务');
    expect(screen.getByRole('status')).toHaveTextContent('准备首次执行');
    expect(screen.queryByText('交给 Code 一个明确任务')).not.toBeInTheDocument();
  });

  it('renders a scoped approval card and confirms one tool call', () => {
    appState.activeSessionId = 'session-approval';
    appState.streamEvents = [];
    appState.toolDecisionState = {};
    appState.messagesBySession['session-approval'] = [
      {
        id: 'assistant-1',
        sessionId: 'session-approval',
        role: 'assistant',
        content: '',
        createdAt: new Date().toISOString(),
        pending: true,
        events: [
          {
            type: 'confirmation_required',
            tool_id: 'tool-7',
            tool_name: 'Bash',
            args: { command: 'cargo test' },
            reason: 'Run the focused validation',
            scope: 'Current workspace',
            risk: 'Executes a local process',
            timeout_ms: 30000,
          },
        ],
      },
    ];
    const confirmToolUse = vi.fn(async () => undefined);
    render(<ExecutionStream actions={{ confirmToolUse } as unknown as TaskActions} />);
    expect(screen.getByText('需要你的确认')).toBeInTheDocument();
    expect(screen.getByText('Run the focused validation')).toBeInTheDocument();
    expect(screen.getByText('Current workspace')).toBeInTheDocument();
    expect(screen.getByText('Executes a local process')).toBeInTheDocument();
    expect(screen.getByText('30 秒')).toBeInTheDocument();
    expect(document.querySelector('.tool-call-event')).toHaveTextContent('cargo test');
    expect(document.querySelector('.tool-call-summary')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '参数' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '允许一次' }));
    expect(confirmToolUse).toHaveBeenCalledWith('session-approval', 'tool-7', true);
  });

  it('prevents duplicate permission decisions while one is submitting', () => {
    appState.activeSessionId = 'session-pending-approval';
    appState.toolDecisionState = { 'session-pending-approval:tool-8': 'approving' };
    appState.messagesBySession['session-pending-approval'] = [
      {
        id: 'assistant-pending-approval',
        sessionId: 'session-pending-approval',
        role: 'assistant',
        content: '',
        createdAt: new Date().toISOString(),
        pending: true,
        events: [{ type: 'confirmation_required', tool_id: 'tool-8', tool_name: 'Bash' }],
      },
    ];
    render(<ExecutionStream actions={{} as TaskActions} />);
    expect(screen.getByRole('button', { name: '允许一次' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '拒绝' })).toBeDisabled();
  });

  it('keeps a failed HITL submission recoverable inside the owning tool card', () => {
    appState.activeSessionId = 'session-failed-approval';
    appState.toolDecisionState = {};
    appState.toolDecisionErrors = {
      'session-failed-approval:tool-9': '本地服务暂时无法提交确认',
    };
    appState.messagesBySession['session-failed-approval'] = [
      {
        id: 'assistant-failed-approval',
        sessionId: 'session-failed-approval',
        role: 'assistant',
        content: '',
        createdAt: new Date().toISOString(),
        pending: true,
        events: [{ type: 'confirmation_required', tool_id: 'tool-9', tool_name: 'Bash' }],
      },
    ];

    render(<ExecutionStream actions={{ confirmToolUse: vi.fn() } as unknown as TaskActions} />);

    expect(screen.getByRole('alert')).toHaveTextContent('本地服务暂时无法提交确认');
    expect(screen.getByRole('button', { name: '允许一次' })).toBeEnabled();
    expect(screen.getByRole('button', { name: '拒绝' })).toBeEnabled();
  });

  it('turns agent completion into a review handoff', () => {
    appState.activeSessionId = 'session-delivery';
    appState.reviewSourceTaskId = null;
    appState.messagesBySession['session-delivery'] = [
      {
        id: 'assistant-delivery',
        sessionId: 'session-delivery',
        role: 'assistant',
        content: 'Implementation complete.',
        createdAt: new Date().toISOString(),
        events: [
          {
            type: 'agent_end',
            verification_summary: {
              status: 'passed',
              report_count: 1,
              required_check_count: 2,
              pending_required_check_count: 0,
              failed_check_count: 0,
              residual_risk_count: 0,
            },
          },
        ],
      },
    ];
    render(<ExecutionStream actions={{} as TaskActions} />);
    expect(screen.getByText('任务已可审阅')).toBeInTheDocument();
    expect(screen.getByRole('progressbar', { name: '交付检查完成度' })).toHaveAttribute('aria-valuenow', '100');
    fireEvent.click(screen.getByRole('button', { name: '审阅变更' }));
    expect(appState.reviewSourceTaskId).toBe('session-delivery');
    expect(appState.reviewIntent).toBe('review');
    expect(appState.taskView).toBe('review');
  });

  it('turns failed validation into a correction in the same task', () => {
    appState.activeSessionId = 'session-needs-fix';
    appState.composerValue = 'Preserve my draft';
    appState.messagesBySession['session-needs-fix'] = [
      {
        id: 'assistant-needs-fix',
        sessionId: 'session-needs-fix',
        role: 'assistant',
        content: 'Implementation complete with a failed check.',
        createdAt: new Date().toISOString(),
        events: [
          {
            type: 'agent_end',
            verification_summary: {
              status: 'failed',
              report_count: 1,
              required_check_count: 1,
              pending_required_check_count: 0,
              failed_check_count: 1,
              residual_risk_count: 0,
              failed_subjects: ['bun run test'],
            },
          },
        ],
      },
    ];
    render(<ExecutionStream actions={{} as TaskActions} />);
    expect(screen.getByText('任务完成，仍需验证')).toBeInTheDocument();
    expect(screen.getByRole('progressbar', { name: '交付检查完成度' })).toHaveAttribute('aria-valuenow', '0');
    expect(screen.getByRole('progressbar', { name: '交付检查完成度' })).toHaveAttribute(
      'aria-valuetext',
      '0/1 项必需检查已通过'
    );
    fireEvent.click(screen.getByRole('button', { name: '继续修正' }));
    expect(appState.activeSessionId).toBe('session-needs-fix');
    expect(appState.taskView).toBe('conversation');
    expect(appState.composerValue).toContain('Preserve my draft');
    expect(appState.composerValue).toContain('bun run test');
  });

  it('does not turn a conversational reply without verification evidence into a delivery', () => {
    appState.activeSessionId = 'session-chat';
    appState.messagesBySession['session-chat'] = [
      {
        id: 'assistant-chat',
        sessionId: 'session-chat',
        role: 'assistant',
        content: 'Hi! How can I help?',
        createdAt: new Date().toISOString(),
        events: [
          {
            type: 'agent_end',
            verification_summary: {
              status: 'skipped',
              report_count: 0,
              required_check_count: 0,
              pending_required_check_count: 0,
              failed_check_count: 0,
              residual_risk_count: 0,
            },
          },
        ],
      },
    ];

    render(<ExecutionStream actions={{} as TaskActions} />);
    expect(screen.getByText('Hi! How can I help?')).toBeInTheDocument();
    expect(screen.queryByLabelText('任务交付摘要')).not.toBeInTheDocument();
  });

  it('keeps planning details in the task runtime panel instead of duplicating them inline', () => {
    appState.activeSessionId = 'session-plan';
    appState.messagesBySession['session-plan'] = [
      {
        id: 'assistant-plan',
        sessionId: 'session-plan',
        role: 'assistant',
        content: '',
        createdAt: new Date().toISOString(),
        pending: true,
        events: [
          {
            type: 'planning_end',
            plan: {
              goal: 'Rebuild Web',
              complexity: 'medium',
              required_tools: [],
              estimated_steps: 2,
              steps: [
                { id: 'one', content: 'Inspect architecture', status: 'pending' },
                { id: 'two', content: 'Implement shell', status: 'pending' },
              ],
            },
          },
          { type: 'step_start', step_id: 'two', description: 'Implement shell', step_number: 2, total_steps: 2 },
          { type: 'step_end', step_id: 'one', status: 'completed', step_number: 1, total_steps: 2 },
        ],
      },
    ];
    render(<ExecutionStream actions={{} as TaskActions} />);
    expect(screen.queryByLabelText('执行计划')).not.toBeInTheDocument();
  });

  it('does not repeat a tool result already represented by its lightweight event', () => {
    appState.activeSessionId = 'session-tool-output';
    appState.messagesBySession['session-tool-output'] = [
      {
        id: 'assistant-tool-output',
        sessionId: 'session-tool-output',
        role: 'assistant',
        content: 'Done.',
        createdAt: new Date().toISOString(),
        events: [{ type: 'tool_end', tool_id: 'tool-1', tool_name: 'Bash', output: 'focused output' }],
        contentBlocks: [{ type: 'tool_result', toolUseId: 'tool-1', name: 'Bash', content: 'focused output' }],
      },
    ];

    const { container } = render(<ExecutionStream actions={{} as TaskActions} />);
    expect(screen.getAllByText('focused output')).toHaveLength(1);
    expect(container.querySelector('.tool-call-summary')).not.toBeInTheDocument();
  });

  it('renders tool activity at its original position in the assistant message', async () => {
    appState.activeSessionId = 'session-ordered-tools';
    appState.messagesBySession['session-ordered-tools'] = [
      {
        id: 'assistant-ordered-tools',
        sessionId: 'session-ordered-tools',
        role: 'assistant',
        content: '先读取配置。读取完成，继续说明。',
        createdAt: new Date().toISOString(),
        events: [
          { type: 'text_delta', text: '先读取配置。' },
          { type: 'tool_start', tool_id: 'read-config', tool_name: 'read' },
          {
            type: 'tool_end',
            tool_id: 'read-config',
            tool_name: 'read',
            args: { path: 'config.acl' },
            output: 'model = "codex"',
            exit_code: 0,
          },
          { type: 'text_delta', text: '读取完成，继续说明。' },
        ],
      },
    ];

    const { container } = render(<ExecutionStream actions={{} as TaskActions} />);
    await waitFor(() => expect(screen.getByText('先读取配置。')).toBeInTheDocument());
    const flow = container.querySelector('.execution-response-flow');
    expect(flow?.children).toHaveLength(3);
    expect(flow?.children[0]).toHaveTextContent('先读取配置。');
    expect(flow?.children[1]).toHaveClass('tool-call-item');
    expect(flow?.children[2]).toHaveTextContent('读取完成，继续说明。');
  });

  it('opens a completed artifact directly in the right workspace preview', async () => {
    appState.activeSessionId = 'session-artifact';
    appState.workspaceRoot = '/repo';
    appState.reviewSourceTaskId = null;
    appState.messagesBySession['session-artifact'] = [
      {
        id: 'assistant-artifact',
        sessionId: 'session-artifact',
        role: 'assistant',
        content: '已完成修改。',
        createdAt: new Date().toISOString(),
        events: [
          {
            type: 'tool_end',
            id: 'edit-1',
            name: 'edit',
            args: { path: 'src/app.ts' },
            output: 'updated',
            exit_code: 0,
          },
        ],
      },
    ];

    const selectFile = vi.fn(async () => true);
    render(<ExecutionStream actions={{ selectFile } as unknown as TaskActions} />);

    expect(screen.getByLabelText('任务产物')).toHaveTextContent('app.ts');
    expect(screen.getByLabelText('任务产物')).toHaveTextContent('src');
    fireEvent.click(screen.getByRole('button', { name: '打开产物 src/app.ts' }));
    await waitFor(() => expect(selectFile).toHaveBeenCalledWith({ path: '/repo/src/app.ts', isBinary: false }));
    expect(appState.reviewSourceTaskId).toBe('session-artifact');
    expect(appState.reviewIntent).toBe('review');
  });

  it('shows the latest output inline and exposes complete evidence only when it is truncated', () => {
    const completeOutput = `${'0123456789'.repeat(50)}\nfinal line must remain visible`;
    appState.activeSessionId = 'session-full-tool-output';
    appState.messagesBySession['session-full-tool-output'] = [
      {
        id: 'assistant-full-tool-output',
        sessionId: 'session-full-tool-output',
        role: 'assistant',
        content: '',
        createdAt: new Date().toISOString(),
        events: [
          {
            type: 'tool_end',
            id: 'tool-full',
            name: 'bash',
            args: { command: 'bun test' },
            output: completeOutput,
            exit_code: 0,
          },
        ],
      },
    ];

    const { container } = render(<ExecutionStream actions={{} as TaskActions} />);
    expect(screen.getByRole('region', { name: '工具输出' })).toHaveTextContent('final line must remain visible');
    expect(container.querySelector('.tool-call-output')).not.toBeInTheDocument();
    const disclosure = screen.getByRole('button', { name: '完整输出 · 2 行' });
    expect(disclosure).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(disclosure);
    expect(disclosure).toHaveAttribute('aria-expanded', 'true');
    expect(container.querySelector('.tool-call-output')?.textContent).toBe(completeOutput);
    expect(screen.queryByRole('region', { name: '执行过程' })).not.toBeInTheDocument();
    expect(screen.queryByLabelText('执行信息')).not.toBeInTheDocument();
    expect(container.querySelector('.tool-call-summary')).not.toBeInTheDocument();
  });

  it('renders a running shell command and incremental output as a lightweight event', () => {
    appState.activeSessionId = 'session-command-preview';
    appState.messagesBySession['session-command-preview'] = [
      {
        id: 'assistant-command-preview',
        sessionId: 'session-command-preview',
        role: 'assistant',
        content: '',
        createdAt: new Date().toISOString(),
        pending: true,
        events: [
          {
            type: 'tool_execution_start',
            tool_id: 'tool-command-preview',
            tool_name: 'bash',
            args: {
              command: 'cargo test -p a3s-cli --test web_cli',
              cwd: '/repo/crates/cli',
            },
          },
          {
            type: 'tool_output_delta',
            tool_id: 'tool-command-preview',
            tool_name: 'bash',
            delta: 'running 13 tests\n',
          },
          {
            type: 'tool_output_delta',
            tool_id: 'tool-command-preview',
            tool_name: 'bash',
            delta: 'test result: ok\n',
          },
        ],
      },
    ];

    const { container } = render(<ExecutionStream actions={{} as TaskActions} />);

    const command = container.querySelector('.tool-call-event');
    expect(command).toHaveTextContent('cargo test -p a3s-cli --test web_cli');
    expect(command?.querySelector('[data-syntax-role="program"]')).toHaveTextContent('cargo');
    expect(command?.querySelectorAll('[data-syntax-role="flag"]')).toHaveLength(2);
    expect(screen.queryByRole('region', { name: '命令预览' })).not.toBeInTheDocument();
    expect(screen.getByRole('log', { name: '实时工具输出' })).toHaveTextContent('test result: ok');
    expect(container.querySelector('.tool-call-result')).toHaveTextContent('实时输出 · 2 行');
    const parameters = screen.getByRole('button', { name: '参数' });
    fireEvent.click(parameters);
    expect(document.querySelector('.tool-json-preview')).toHaveTextContent('/repo/crates/cli');
  });

  it('renders a successful call without output as an indented event note', () => {
    appState.activeSessionId = 'session-tool-no-output';
    appState.messagesBySession['session-tool-no-output'] = [
      {
        id: 'assistant-tool-no-output',
        sessionId: 'session-tool-no-output',
        role: 'assistant',
        content: '',
        createdAt: new Date().toISOString(),
        events: [
          {
            type: 'tool_end',
            tool_id: 'tool-no-output',
            tool_name: 'bash',
            args: { command: 'test -f README.md' },
            output: '',
            exit_code: 0,
          },
        ],
      },
    ];

    const { container } = render(<ExecutionStream actions={{} as TaskActions} />);

    expect(screen.getByText('(无输出)')).toBeInTheDocument();
    expect(container.querySelector('.tool-call-empty-output')).toBeInTheDocument();
    expect(container.querySelector('.tool-call-summary')).not.toBeInTheDocument();
  });

  it('keeps completed tool rows compact without wrapping them in a summary card', () => {
    appState.activeSessionId = 'session-tool-density';
    appState.messagesBySession['session-tool-density'] = [
      {
        id: 'assistant-tool-density',
        sessionId: 'session-tool-density',
        role: 'assistant',
        content: '',
        createdAt: new Date().toISOString(),
        events: Array.from({ length: 8 }, (_, index) => ({
          type: 'tool_end',
          id: `read-${index + 1}`,
          name: 'read',
          args: { path: `src/file-${index + 1}.ts` },
          output: `file ${index + 1}`,
          exit_code: 0,
        })),
      },
    ];

    const { container } = render(<ExecutionStream actions={{} as TaskActions} />);

    expect(container.querySelectorAll('.tool-call-item')).toHaveLength(8);
    expect(container.querySelectorAll('.tool-call-output-preview')).toHaveLength(8);
    expect(screen.queryByText('执行过程')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /查看之前/ })).not.toBeInTheDocument();
    expect(container.querySelector('.tool-call-summary')).not.toBeInTheDocument();
  });

  it('settles a running event in place without introducing a row disclosure', async () => {
    appState.activeSessionId = 'session-tool-transition';
    appState.messagesBySession['session-tool-transition'] = [
      {
        id: 'assistant-tool-transition',
        sessionId: 'session-tool-transition',
        role: 'assistant',
        content: '',
        createdAt: new Date().toISOString(),
        pending: true,
        events: [
          {
            type: 'tool_execution_start',
            tool_id: 'tool-transition',
            tool_name: 'bash',
            args: { command: 'bun test' },
          },
        ],
      },
    ];

    const { container } = render(<ExecutionStream actions={{} as TaskActions} />);
    const event = container.querySelector('.tool-call-item');
    expect(event).toHaveClass('running');
    expect(container.querySelector('.tool-call-summary')).not.toBeInTheDocument();

    appState.messagesBySession['session-tool-transition'][0].pending = false;
    appState.messagesBySession['session-tool-transition'][0].events = [
      {
        type: 'tool_end',
        tool_id: 'tool-transition',
        tool_name: 'bash',
        output: 'passed',
        exit_code: 0,
      },
    ];

    await waitFor(() => expect(event).toHaveClass('succeeded'));
    expect(screen.getByRole('region', { name: '工具输出' })).toHaveTextContent('passed');
    expect(container.querySelector('.tool-call-summary')).not.toBeInTheDocument();
  });
});
