import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../../state/app-state';
import type { CodeActions } from '../../code/use-code-controller';
import { WorkProduct } from './work-product';

const mocks = vi.hoisted(() => ({
  createArtifact: vi.fn(),
  pickRoot: vi.fn(async () => null as string | null),
  openCodeFile: vi.fn(async () => true),
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
  }),
}));

vi.mock('../use-work-code-controller', () => ({
  useWorkCodeController: () => ({
    tabs: [],
    activePath: null,
    openFile: mocks.openCodeFile,
  }),
}));

vi.mock('../components/work-home', () => ({
  WorkHome: ({ onTaskSubmit }: { onTaskSubmit: () => void }) => (
    <main>
      <div data-office-shortcuts='ignore'>
        <input aria-label='AI 指令' />
      </div>
      <button type='button' onClick={onTaskSubmit}>
        提交首页任务
      </button>
    </main>
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
    appState.workspaceRoot = '/docs';
    appState.health = null;
    appState.fileQuickOpenOpen = false;
    appState.newTaskConfig.workspace = '/docs';
    mocks.createArtifact.mockReset();
    mocks.pickRoot.mockReset().mockResolvedValue(null);
    mocks.openCodeFile.mockReset().mockResolvedValue(true);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('runs only plain Work commands and leaves excluded editors alone', () => {
    render(<WorkProduct actions={{} as CodeActions} />);
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

  it('lands on the AI home with the shared assistant and workspace split open', () => {
    localStorage.removeItem('a3s-work.surface');
    localStorage.removeItem('a3s-work.copilot-open');

    render(<WorkProduct actions={{} as CodeActions} />);

    expect(screen.getByRole('textbox', { name: 'AI 指令' })).toBeInTheDocument();
    expect(screen.getByRole('complementary', { name: 'AI 助手' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '提交首页任务' }));
    expect(screen.getByRole('complementary', { name: 'AI 助手' })).toBeInTheDocument();
    expect(localStorage.getItem('a3s-work.copilot-open')).toBe('true');
  });

  it('opens assistant task files in the Work code scene', () => {
    localStorage.setItem('a3s-work.copilot-open', 'true');

    render(<WorkProduct actions={{} as CodeActions} />);
    fireEvent.click(screen.getByRole('button', { name: '打开任务文件' }));

    expect(mocks.openCodeFile).toHaveBeenCalledWith({ path: '/docs/src/app.ts', isBinary: false });
  });
});
