import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { ComponentProps } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../../state/app-state';
import type { TaskActions } from '../../tasks/task-actions';
import { createWorkArtifact } from '../work-templates';
import { WorkHome } from './work-home';

function workHomeProps(overrides: Partial<ComponentProps<typeof WorkHome>> = {}): ComponentProps<typeof WorkHome> {
  return {
    artifacts: [],
    folders: [],
    view: 'home',
    activeFolderId: null,
    loading: false,
    error: null,
    sidebarOpen: true,
    taskActions: { sendMessage: vi.fn(async () => undefined) } as unknown as TaskActions,
    onOpenSidebar: vi.fn(),
    onTaskSubmit: vi.fn(),
    onOpenWorkspace: vi.fn(),
    onCreate: vi.fn(),
    onOpen: vi.fn(),
    onImport: vi.fn(),
    onToggleFavorite: vi.fn(),
    onRename: vi.fn(),
    onCopy: vi.fn(),
    onMove: vi.fn(),
    onRestore: vi.fn(),
    onDelete: vi.fn(),
    onOpenFolder: vi.fn(),
    onCreateFolder: vi.fn(),
    onRenameFolder: vi.fn(),
    onRestoreFolder: vi.fn(),
    onDeleteFolder: vi.fn(),
    onRetry: vi.fn(),
    ...overrides,
  };
}

describe('Work file center', () => {
  beforeEach(() => {
    appState.activeProduct = 'work';
    appState.composerValue = '';
    appState.composerContextFiles = [];
    appState.composerSkills = [];
    appState.streamingSessionId = null;
    appState.taskSubmissionState = null;
  });

  afterEach(cleanup);

  it('starts AI-native Work tasks from the home hero and keeps file actions one click away', async () => {
    const sendMessage = vi.fn(async () => undefined);
    const onTaskSubmit = vi.fn();
    const onOpenWorkspace = vi.fn();
    const onCreate = vi.fn();
    render(
      <WorkHome
        {...workHomeProps({
          taskActions: { sendMessage } as unknown as TaskActions,
          onTaskSubmit,
          onOpenWorkspace,
          onCreate,
        })}
      />
    );

    expect(screen.getByRole('heading', { name: '从一个任务开始，完成文档、数据与文件工作' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '分析数据' }));
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: '任务指令' })).toHaveTextContent('识别关键趋势、异常和相互关系')
    );
    fireEvent.click(screen.getByRole('button', { name: '发送任务' }));
    expect(onTaskSubmit).toHaveBeenCalledTimes(1);
    expect(sendMessage).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: '全部文件' }));
    expect(onOpenWorkspace).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: '文字' }));
    expect(onCreate).toHaveBeenCalledWith('blank-document');
  });

  it('restores the collapsed office sidebar from the document library', () => {
    const onOpenSidebar = vi.fn();
    render(
      <WorkHome
        artifacts={[]}
        folders={[]}
        view='home'
        activeFolderId={null}
        loading={false}
        error={null}
        sidebarOpen={false}
        taskActions={{ sendMessage: vi.fn(async () => undefined) } as unknown as TaskActions}
        onOpenSidebar={onOpenSidebar}
        onTaskSubmit={vi.fn()}
        onOpenWorkspace={vi.fn()}
        onCreate={vi.fn()}
        onOpen={vi.fn()}
        onImport={vi.fn()}
        onToggleFavorite={vi.fn()}
        onRename={vi.fn()}
        onCopy={vi.fn()}
        onMove={vi.fn()}
        onRestore={vi.fn()}
        onDelete={vi.fn()}
        onOpenFolder={vi.fn()}
        onCreateFolder={vi.fn()}
        onRenameFolder={vi.fn()}
        onRestoreFolder={vi.fn()}
        onDeleteFolder={vi.fn()}
        onRetry={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '展开会话侧边栏' }));
    expect(onOpenSidebar).toHaveBeenCalledTimes(1);
  });

  it('starts every Office artifact type from the home surface', () => {
    const create = vi.fn();
    render(
      <WorkHome
        artifacts={[]}
        folders={[]}
        view='home'
        activeFolderId={null}
        loading={false}
        error={null}
        sidebarOpen={true}
        taskActions={{ sendMessage: vi.fn(async () => undefined) } as unknown as TaskActions}
        onOpenSidebar={vi.fn()}
        onTaskSubmit={vi.fn()}
        onOpenWorkspace={vi.fn()}
        onCreate={create}
        onOpen={vi.fn()}
        onImport={vi.fn()}
        onToggleFavorite={vi.fn()}
        onRename={vi.fn()}
        onCopy={vi.fn()}
        onMove={vi.fn()}
        onRestore={vi.fn()}
        onDelete={vi.fn()}
        onOpenFolder={vi.fn()}
        onCreateFolder={vi.fn()}
        onRenameFolder={vi.fn()}
        onRestoreFolder={vi.fn()}
        onDeleteFolder={vi.fn()}
        onRetry={vi.fn()}
      />
    );

    expect(screen.getByRole('button', { name: /空白文字/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /空白表格/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /空白演示/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /季度计划/ }));
    expect(create).toHaveBeenCalledWith('quarterly-plan');
  });

  it('searches and opens persisted artifacts', () => {
    const open = vi.fn();
    const document = createWorkArtifact('project-brief');
    const spreadsheet = createWorkArtifact('quarterly-plan');
    render(
      <WorkHome
        artifacts={[document, spreadsheet]}
        folders={[]}
        view='recent'
        activeFolderId={null}
        loading={false}
        error={null}
        sidebarOpen={true}
        taskActions={{ sendMessage: vi.fn(async () => undefined) } as unknown as TaskActions}
        onOpenSidebar={vi.fn()}
        onTaskSubmit={vi.fn()}
        onOpenWorkspace={vi.fn()}
        onCreate={vi.fn()}
        onOpen={open}
        onImport={vi.fn()}
        onToggleFavorite={vi.fn()}
        onRename={vi.fn()}
        onCopy={vi.fn()}
        onMove={vi.fn()}
        onRestore={vi.fn()}
        onDelete={vi.fn()}
        onOpenFolder={vi.fn()}
        onCreateFolder={vi.fn()}
        onRenameFolder={vi.fn()}
        onRestoreFolder={vi.fn()}
        onDeleteFolder={vi.fn()}
        onRetry={vi.fn()}
      />
    );

    fireEvent.change(screen.getByRole('searchbox', { name: '搜索文件' }), {
      target: { value: '季度' },
    });
    expect(screen.queryByRole('button', { name: `打开 ${document.title}` })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: `打开 ${spreadsheet.title}` }));
    expect(open).toHaveBeenCalledWith(spreadsheet.id);
  });

  it('separates trashed files and exposes explicit recovery', () => {
    const restore = vi.fn();
    const active = createWorkArtifact('project-brief');
    const trashed = { ...createWorkArtifact('quarterly-plan'), trashedAt: Date.now() };
    render(
      <WorkHome
        artifacts={[active, trashed]}
        folders={[]}
        view='trash'
        activeFolderId={null}
        loading={false}
        error={null}
        sidebarOpen={true}
        taskActions={{ sendMessage: vi.fn(async () => undefined) } as unknown as TaskActions}
        onOpenSidebar={vi.fn()}
        onTaskSubmit={vi.fn()}
        onOpenWorkspace={vi.fn()}
        onCreate={vi.fn()}
        onOpen={vi.fn()}
        onImport={vi.fn()}
        onToggleFavorite={vi.fn()}
        onRename={vi.fn()}
        onCopy={vi.fn()}
        onMove={vi.fn()}
        onRestore={restore}
        onDelete={vi.fn()}
        onOpenFolder={vi.fn()}
        onCreateFolder={vi.fn()}
        onRenameFolder={vi.fn()}
        onRestoreFolder={vi.fn()}
        onDeleteFolder={vi.fn()}
        onRetry={vi.fn()}
      />
    );

    expect(screen.queryByRole('button', { name: `打开 ${active.title}` })).not.toBeInTheDocument();
    fireEvent.click(screen.getByLabelText(`${trashed.title} 更多操作`));
    fireEvent.click(screen.getByRole('button', { name: '恢复' }));
    expect(restore).toHaveBeenCalledWith(trashed.id);
  });

  it('completes low-risk library operations inline and trashes directly', async () => {
    const artifact = createWorkArtifact('project-brief');
    const folder = {
      id: 'folder-plans',
      name: '计划',
      parentId: null,
      createdAt: Date.now(),
      updatedAt: Date.now(),
      revision: 1,
      trashedAt: null,
    };
    const createFolder = vi.fn().mockResolvedValue(undefined);
    const rename = vi.fn().mockResolvedValue(undefined);
    const move = vi.fn();
    const remove = vi.fn();
    const { container } = render(
      <WorkHome
        artifacts={[artifact]}
        folders={[folder]}
        view='home'
        activeFolderId={null}
        loading={false}
        error={null}
        sidebarOpen={true}
        taskActions={{ sendMessage: vi.fn(async () => undefined) } as unknown as TaskActions}
        onOpenSidebar={vi.fn()}
        onTaskSubmit={vi.fn()}
        onOpenWorkspace={vi.fn()}
        onCreate={vi.fn()}
        onOpen={vi.fn()}
        onImport={vi.fn()}
        onToggleFavorite={vi.fn()}
        onRename={rename}
        onCopy={vi.fn()}
        onMove={move}
        onRestore={vi.fn()}
        onDelete={remove}
        onOpenFolder={vi.fn()}
        onCreateFolder={createFolder}
        onRenameFolder={vi.fn()}
        onRestoreFolder={vi.fn()}
        onDeleteFolder={vi.fn()}
        onRetry={vi.fn()}
      />
    );

    expect(container.querySelector('select')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: '新建文件夹' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    fireEvent.change(screen.getByRole('textbox', { name: '新建文件夹名称' }), {
      target: { value: '归档' },
    });
    fireEvent.click(screen.getByRole('button', { name: '创建文件夹' }));
    await waitFor(() => expect(createFolder).toHaveBeenCalledWith('归档'));

    const artifactMenu = screen.getByLabelText(`${artifact.title} 更多操作`).closest('details');
    if (!artifactMenu) throw new Error('Expected artifact action menu');
    fireEvent.click(screen.getByLabelText(`${artifact.title} 更多操作`));
    fireEvent.click(within(artifactMenu).getByRole('button', { name: '重命名' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    fireEvent.change(screen.getByRole('textbox', { name: `重命名文件 ${artifact.title}` }), {
      target: { value: '项目方案 2026' },
    });
    fireEvent.submit(
      screen.getByRole('textbox', { name: `重命名文件 ${artifact.title}` }).closest('form') as HTMLFormElement
    );
    await waitFor(() => expect(rename).toHaveBeenCalledWith(artifact.id, '项目方案 2026'));

    fireEvent.click(screen.getByLabelText(`${artifact.title} 更多操作`));
    fireEvent.click(screen.getByRole('combobox', { name: `移动 ${artifact.title}` }));
    fireEvent.click(screen.getByRole('option', { name: '计划' }));
    expect(move).toHaveBeenCalledWith(artifact.id, folder.id);

    const refreshedArtifactMenu = screen.getByLabelText(`${artifact.title} 更多操作`).closest('details');
    if (!refreshedArtifactMenu) throw new Error('Expected refreshed artifact action menu');
    fireEvent.click(screen.getByLabelText(`${artifact.title} 更多操作`));
    fireEvent.click(within(refreshedArtifactMenu).getByRole('button', { name: '移到回收站' }));
    expect(remove).toHaveBeenCalledWith(artifact);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('keeps an inline editor open when a library rename fails and cancels it with Escape', async () => {
    const artifact = createWorkArtifact('project-brief');
    const rename = vi.fn().mockRejectedValue(new Error('名称已存在'));
    render(<WorkHome {...workHomeProps({ artifacts: [artifact], view: 'recent', onRename: rename })} />);

    const artifactMenu = screen.getByLabelText(`${artifact.title} 更多操作`).closest('details');
    if (!artifactMenu) throw new Error('Expected artifact action menu');
    fireEvent.click(screen.getByLabelText(`${artifact.title} 更多操作`));
    fireEvent.click(within(artifactMenu).getByRole('button', { name: '重命名' }));
    const input = screen.getByRole('textbox', { name: `重命名文件 ${artifact.title}` });
    fireEvent.change(input, { target: { value: '重复名称' } });
    fireEvent.submit(input.closest('form') as HTMLFormElement);

    expect(await screen.findByRole('alert')).toHaveTextContent('名称已存在');
    expect(screen.getByRole('textbox', { name: `重命名文件 ${artifact.title}` })).toHaveValue('重复名称');
    fireEvent.keyDown(screen.getByRole('textbox', { name: `重命名文件 ${artifact.title}` }), {
      key: 'Escape',
    });
    expect(screen.queryByRole('textbox', { name: `重命名文件 ${artifact.title}` })).not.toBeInTheDocument();
  });

  it('does not let an older inline request close a newer editor', async () => {
    const artifact = createWorkArtifact('project-brief');
    let resolveCreateFolder: (() => void) | undefined;
    const createFolder = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveCreateFolder = resolve;
        })
    );
    render(<WorkHome {...workHomeProps({ artifacts: [artifact], onCreateFolder: createFolder })} />);

    fireEvent.click(screen.getByRole('button', { name: '新建文件夹' }));
    fireEvent.change(screen.getByRole('textbox', { name: '新建文件夹名称' }), {
      target: { value: '稍后完成' },
    });
    fireEvent.click(screen.getByRole('button', { name: '创建文件夹' }));
    await waitFor(() => expect(createFolder).toHaveBeenCalledWith('稍后完成'));

    const artifactMenu = screen.getByLabelText(`${artifact.title} 更多操作`).closest('details');
    if (!artifactMenu) throw new Error('Expected artifact action menu');
    fireEvent.click(screen.getByLabelText(`${artifact.title} 更多操作`));
    fireEvent.click(within(artifactMenu).getByRole('button', { name: '重命名' }));
    expect(screen.getByRole('textbox', { name: `重命名文件 ${artifact.title}` })).toBeInTheDocument();

    const finishCreateFolder = resolveCreateFolder;
    if (!finishCreateFolder) throw new Error('Expected the folder request to be pending');
    await act(async () => {
      finishCreateFolder();
    });

    expect(screen.getByRole('textbox', { name: `重命名文件 ${artifact.title}` })).toBeInTheDocument();
  });

  it('keeps confirmation only for permanent deletion from the managed-library trash', () => {
    const artifact = { ...createWorkArtifact('project-brief'), trashedAt: Date.now() };
    const remove = vi.fn();
    render(<WorkHome {...workHomeProps({ artifacts: [artifact], view: 'trash', onDelete: remove })} />);

    const artifactMenu = screen.getByLabelText(`${artifact.title} 更多操作`).closest('details');
    if (!artifactMenu) throw new Error('Expected artifact action menu');
    fireEvent.click(screen.getByLabelText(`${artifact.title} 更多操作`));
    fireEvent.click(within(artifactMenu).getByRole('button', { name: '永久删除' }));

    expect(screen.getByRole('dialog', { name: '永久删除文件' })).toBeInTheDocument();
    expect(remove).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '确认永久删除' }));
    expect(remove).toHaveBeenCalledWith(artifact);
  });
});
