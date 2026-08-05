import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../../state/app-state';
import type { CodeActions } from '../../code/use-code-controller';
import { WorkConversation } from './work-conversation';

vi.mock('../../tasks/components/execution-stream', () => ({
  ExecutionStream: () => <div>执行记录</div>,
}));

vi.mock('../../tasks/components/task-composer', () => ({
  TaskComposer: () => <div>后续指令输入框</div>,
}));

vi.mock('../../tasks/components/task-runtime-floating-panel', () => ({
  TaskRuntimeFloatingPanel: () => <aside aria-label='任务运行状态'>运行状态</aside>,
}));

describe('Work conversation', () => {
  beforeEach(() => {
    appState.bootPhase = 'ready';
    appState.serviceStatus = 'connected';
    appState.sidebarOpen = false;
    appState.taskSubmissionState = null;
    appState.streamingSessionId = null;
    appState.activeSessionId = null;
    appState.sessions = [];
    appState.messagesBySession = {};
    appState.messageErrors = {};
    appState.executionTimings = {};
    appState.turnQueues = {};
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('keeps task identity, execution, runtime, and follow-up controls in one canonical page', () => {
    appState.activeSessionId = 'task-7';
    appState.streamingSessionId = 'task-7';
    appState.sessions = [session('task-7', '/repo/a3s', '修复首页导航')];
    const onHome = vi.fn();
    const onNewTask = vi.fn();
    const onOpenWorkspace = vi.fn();

    render(
      <WorkConversation
        actions={{} as CodeActions}
        sessionId='task-7'
        sidebarOpen={false}
        onOpenSidebar={vi.fn()}
        onHome={onHome}
        onNewTask={onNewTask}
        onOpenWorkspace={onOpenWorkspace}
      />
    );

    expect(screen.getByRole('region', { name: '任务对话：修复首页导航' })).toBeInTheDocument();
    expect(screen.getByText('/repo/a3s')).toBeInTheDocument();
    expect(screen.getByText('执行中')).toBeInTheDocument();
    expect(screen.getByText('执行记录')).toBeInTheDocument();
    expect(screen.getByText('后续指令输入框')).toBeInTheDocument();
    expect(screen.getByRole('complementary', { name: '任务运行状态' })).toBeInTheDocument();
    expect(screen.queryByRole('complementary', { name: 'AI 助手' })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '返回首页' }));
    fireEvent.click(screen.getByRole('button', { name: '打开工作区' }));
    fireEvent.click(screen.getByRole('button', { name: '新建任务' }));
    expect(onHome).toHaveBeenCalledOnce();
    expect(onOpenWorkspace).toHaveBeenCalledOnce();
    expect(onNewTask).toHaveBeenCalledOnce();
  });

  it('announces session-scoped execution changes through the status ribbon', () => {
    appState.activeSessionId = 'task-7';
    appState.sessions = [session('task-7', '/repo/a3s', '检查任务状态')];
    appState.executionTimings['task-7'] = { startedAt: 1, status: 'running' };
    const props = {
      actions: {} as CodeActions,
      sessionId: 'task-7',
      sidebarOpen: true,
      onOpenSidebar: vi.fn(),
      onHome: vi.fn(),
      onNewTask: vi.fn(),
      onOpenWorkspace: vi.fn(),
    };
    const view = render(<WorkConversation {...props} />);

    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-live', 'polite');
    expect(status).toHaveAttribute('aria-atomic', 'true');
    expect(status).toHaveTextContent('执行中');

    act(() => {
      appState.executionTimings['task-7'] = { startedAt: 1, completedAt: 2, status: 'completed' };
    });
    view.rerender(<WorkConversation {...props} />);

    expect(screen.getByRole('status')).toHaveTextContent('已完成');
  });

  it('restores failed terminal state from the latest assistant events', () => {
    appState.activeSessionId = 'task-7';
    appState.sessions = [session('task-7', '/repo/a3s', '恢复失败任务')];
    appState.messagesBySession['task-7'] = [
      {
        id: 'assistant-1',
        sessionId: 'task-7',
        role: 'assistant',
        content: '执行未完成',
        createdAt: '2026-08-05T08:00:00.000Z',
        events: [{ type: 'error', message: 'Command failed' }],
      },
    ];

    render(
      <WorkConversation
        actions={{} as CodeActions}
        sessionId='task-7'
        sidebarOpen
        onOpenSidebar={vi.fn()}
        onHome={vi.fn()}
        onNewTask={vi.fn()}
        onOpenWorkspace={vi.fn()}
      />
    );

    expect(screen.getByRole('status')).toHaveTextContent('执行失败');
  });

  it('does not apply a global submission label to another selected task', () => {
    appState.taskSubmissionState = 'queueing';
    appState.activeSessionId = 'task-other';
    appState.sessions = [session('task-other', '/repo/other', '其他任务')];

    render(
      <WorkConversation
        actions={{} as CodeActions}
        sessionId='task-other'
        sidebarOpen
        onOpenSidebar={vi.fn()}
        onHome={vi.fn()}
        onNewTask={vi.fn()}
        onOpenWorkspace={vi.fn()}
      />
    );

    expect(screen.getByRole('status')).toHaveTextContent('可继续');
  });

  it('shows a truthful recovery state when a copied route points to a missing task', () => {
    render(
      <WorkConversation
        actions={{} as CodeActions}
        sessionId='deleted-task'
        sidebarOpen
        onOpenSidebar={vi.fn()}
        onHome={vi.fn()}
        onNewTask={vi.fn()}
        onOpenWorkspace={vi.fn()}
      />
    );

    expect(screen.getByRole('alert')).toHaveTextContent('找不到这个任务');
    expect(screen.getByRole('alert')).toHaveTextContent('可能已被删除，或当前服务没有返回这条任务记录');
    expect(screen.queryByText('执行记录')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '返回首页' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '新建任务' })).toBeInTheDocument();
  });

  it('does not render another active task while the requested route is still opening', () => {
    appState.activeSessionId = 'task-old';
    appState.sessions = [session('task-old', '/repo/old', '旧任务'), session('task-new', '/repo/new', '新任务')];

    render(
      <WorkConversation
        actions={{} as CodeActions}
        sessionId='task-new'
        sidebarOpen
        onOpenSidebar={vi.fn()}
        onHome={vi.fn()}
        onNewTask={vi.fn()}
        onOpenWorkspace={vi.fn()}
      />
    );

    expect(screen.getByText('正在打开任务').closest('[role="status"]')).toHaveTextContent('正在打开任务');
    expect(screen.queryByText('执行记录')).not.toBeInTheDocument();
    expect(screen.queryByText('旧任务')).not.toBeInTheDocument();
  });
});

function session(sessionId: string, workspace: string, title: string) {
  return {
    sessionId,
    workspace,
    cwd: workspace,
    followDefaultModel: true,
    permissionMode: 'default',
    state: 'idle',
    title,
    createdAt: 1,
  };
}
