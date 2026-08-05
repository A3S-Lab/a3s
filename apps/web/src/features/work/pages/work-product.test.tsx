import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../../state/app-state';
import type { CodeActions } from '../../code/use-code-controller';
import { WorkProduct } from './work-product';

const mocks = vi.hoisted(() => ({
  createArtifact: vi.fn(),
  pickRoot: vi.fn(async () => null as string | null),
  selectRoot: vi.fn(async () => undefined),
  openCodeFile: vi.fn(async () => true),
  codeTabs: [] as Array<{ path: string }>,
}));

vi.mock('../use-work-controller', () => ({
  useWorkController: () => ({
    activeArtifact: null,
    pendingImport: null,
    artifacts: [],
    folders: [],
    libraryView: 'all',
    activeFolderId: null,
    loading: false,
    loadError: null,
    createArtifact: mocks.createArtifact,
  }),
}));

vi.mock('../use-work-files-controller', () => ({
  useWorkFilesController: () => ({
    rootPath: '/docs',
    currentPath: '/docs',
    pickRoot: mocks.pickRoot,
    selectRoot: mocks.selectRoot,
  }),
}));

vi.mock('../use-work-code-controller', () => ({
  useWorkCodeController: () => ({
    tabs: mocks.codeTabs,
    activePath: mocks.codeTabs[0]?.path ?? null,
    openFile: mocks.openCodeFile,
  }),
}));

vi.mock('../components/work-home', () => ({
  WorkHome: ({ onTaskSubmit }: { onTaskSubmit: (content: string) => void }) => (
    <main>
      <div data-office-shortcuts='ignore'>
        <input aria-label='AI 指令' />
      </div>
      <button type='button' onClick={() => onTaskSubmit('整理本周工作并给出下一步')}>
        提交首页任务
      </button>
    </main>
  ),
}));

vi.mock('../components/work-conversation', () => ({
  WorkConversation: ({ sessionId }: { sessionId: string | null }) => (
    <main aria-label='独立任务对话'>独立对话页 {sessionId}</main>
  ),
}));

vi.mock('../components/work-code-workspace', () => ({
  WorkCodeWorkspace: ({ onToggleAssistant }: { onToggleAssistant: () => void }) => (
    <main>
      <button type='button' onClick={onToggleAssistant}>
        切换上下文助手
      </button>
    </main>
  ),
}));

vi.mock('../../tasks/components/task-library', () => ({
  TaskLibrary: ({ onSelectSession }: { onSelectSession: (session: Record<string, unknown>) => void }) => (
    <aside aria-label='任务列表'>
      <button
        type='button'
        onClick={() =>
          onSelectSession({
            sessionId: 'task-from-library',
            workspace: '/library',
            cwd: '/library',
            followDefaultModel: true,
            permissionMode: 'default',
            state: 'idle',
            createdAt: 1,
          })
        }
      >
        打开历史任务
      </button>
    </aside>
  ),
}));

vi.mock('../components/work-copilot', () => ({
  readWorkCopilotWidth: () => 420,
  WorkCopilot: ({ actions }: { actions: CodeActions }) => (
    <aside aria-label='AI 助手'>
      <button type='button' onClick={() => void actions.selectFile({ path: '/docs/src/app.ts', isBinary: false })}>
        打开任务文件
      </button>
    </aside>
  ),
}));

describe('Work product shortcuts', () => {
  beforeEach(() => {
    localStorage.setItem('a3s-work.surface', 'library');
    localStorage.setItem('a3s-work.copilot-open', 'false');
    appState.sidebarOpen = false;
    appState.activeProduct = 'work';
    appState.workRoute = 'home';
    appState.conversationSessionId = null;
    appState.activeSessionId = null;
    appState.sessions = [];
    appState.taskSubmissionState = null;
    appState.workspaceRoot = '/docs';
    appState.health = null;
    appState.fileQuickOpenOpen = false;
    appState.newTaskConfig.workspace = '/docs';
    window.history.replaceState(null, '', '#home');
    mocks.createArtifact.mockReset();
    mocks.pickRoot.mockReset().mockResolvedValue(null);
    mocks.selectRoot.mockReset().mockResolvedValue(undefined);
    mocks.openCodeFile.mockReset().mockResolvedValue(true);
    mocks.codeTabs = [];
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('runs only plain Work commands and leaves excluded editors alone', () => {
    render(<WorkProduct actions={codeActions()} />);
    const prompt = screen.getByRole('textbox', { name: 'AI 指令' });

    expect(fireEvent.keyDown(prompt, { key: 'n', metaKey: true })).toBe(true);
    expect(fireEvent.keyDown(prompt, { key: 'o', metaKey: true })).toBe(true);
    expect(fireEvent.keyDown(window, { key: 'n', metaKey: true, shiftKey: true })).toBe(true);
    expect(mocks.createArtifact).not.toHaveBeenCalled();
    expect(mocks.pickRoot).not.toHaveBeenCalled();

    expect(fireEvent.keyDown(window, { key: 'n', metaKey: true })).toBe(false);
    expect(fireEvent.keyDown(window, { key: 'o', metaKey: true })).toBe(false);
    expect(mocks.createArtifact).toHaveBeenCalledWith('blank-document');
    expect(mocks.pickRoot).toHaveBeenCalledOnce();
  });

  it('keeps Home focused and opens an existing task as an independent conversation', () => {
    localStorage.removeItem('a3s-work.surface');
    localStorage.removeItem('a3s-work.copilot-open');
    appState.activeSessionId = 'task-active';
    appState.sessions = [session('task-active', '/docs')];

    render(<WorkProduct actions={codeActions()} />);

    expect(screen.getByRole('textbox', { name: 'AI 指令' })).toBeInTheDocument();
    expect(screen.queryByRole('complementary', { name: 'AI 助手' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '提交首页任务' }));
    expect(screen.getByRole('main', { name: '独立任务对话' })).toHaveTextContent('task-active');
    expect(window.location.hash).toBe('#conversation/task-active');
    expect(screen.queryByRole('complementary', { name: 'AI 助手' })).not.toBeInTheDocument();
    expect(localStorage.getItem('a3s-work.copilot-open')).toBe('false');
  });

  it('opens a newly created task as soon as it receives a durable session id', async () => {
    render(<WorkProduct actions={codeActions()} />);

    fireEvent.click(screen.getByRole('button', { name: '提交首页任务' }));
    act(() => {
      appState.taskSubmissionState = 'creating';
    });
    expect(screen.getByRole('textbox', { name: 'AI 指令' })).toBeInTheDocument();

    act(() => {
      appState.sessions = [session('task-new', '/docs')];
      appState.activeSessionId = 'task-new';
      appState.taskSubmissionState = 'queueing';
    });

    await waitFor(() => expect(screen.getByRole('main', { name: '独立任务对话' })).toHaveTextContent('task-new'));
    expect(window.location.hash).toBe('#conversation/task-new');
  });

  it('opens a task-library selection as the canonical conversation page', async () => {
    appState.sidebarOpen = true;
    appState.sessions = [session('task-from-library', '/library')];
    const actions = codeActions();

    render(<WorkProduct actions={actions} />);
    fireEvent.click(screen.getByRole('button', { name: '打开历史任务' }));

    await waitFor(() => expect(actions.selectSession).toHaveBeenCalledWith('task-from-library'));
    expect(screen.getByRole('main', { name: '独立任务对话' })).toHaveTextContent('task-from-library');
    expect(window.location.hash).toBe('#conversation/task-from-library');
    expect(screen.queryByRole('complementary', { name: 'AI 助手' })).not.toBeInTheDocument();
  });

  it('keeps the assistant contextual to the Work code scene', () => {
    localStorage.setItem('a3s-work.copilot-open', 'true');
    mocks.codeTabs = [{ path: '/docs/src/app.ts' }];

    render(<WorkProduct actions={codeActions()} />);
    expect(screen.getByRole('complementary', { name: 'AI 助手' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '打开任务文件' }));

    expect(mocks.openCodeFile).toHaveBeenCalledWith({ path: '/docs/src/app.ts', isBinary: false });
  });
});

function codeActions(): CodeActions {
  return {
    newConversation: vi.fn(() => {
      appState.activeSessionId = null;
    }),
    selectSession: vi.fn(async (sessionId: string) => {
      appState.activeSessionId = sessionId;
    }),
  } as unknown as CodeActions;
}

function session(sessionId: string, workspace: string) {
  return {
    sessionId,
    workspace,
    cwd: workspace,
    followDefaultModel: true,
    permissionMode: 'default',
    state: 'idle',
    title: sessionId,
    createdAt: 1,
  };
}
