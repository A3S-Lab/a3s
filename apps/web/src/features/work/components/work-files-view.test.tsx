import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { WorkspaceEntry } from '../../../types/api';
import type { WorkFilesActions } from '../use-work-files-controller';
import { WorkFilesView } from './work-files-view';
import { WorkFilesWorkspace } from './work-files-workspace';

const report: WorkspaceEntry = {
  name: 'Report.docx',
  path: '/docs/Report.docx',
  isDirectory: false,
  isFile: true,
  size: 1024,
  mtimeMs: 10,
  extension: 'docx',
  isBinary: false,
};

const archive: WorkspaceEntry = {
  name: 'Archive',
  path: '/docs/Archive',
  isDirectory: true,
  isFile: false,
  isBinary: false,
  size: 0,
  mtimeMs: 10,
};

function actions(overrides: Partial<WorkFilesActions> = {}): WorkFilesActions {
  return {
    rootPath: '/docs',
    recentRootPaths: ['/docs'],
    currentPath: '/docs',
    entries: [report],
    visibleEntries: [report],
    selectedPaths: new Set<string>(),
    selectedEntries: [],
    loading: false,
    error: null,
    query: '',
    searchScope: 'folder',
    searchLoading: false,
    searchError: null,
    searchTruncated: false,
    searchUnreadableDirectories: 0,
    layout: 'grid',
    sort: { key: 'name', direction: 'ascending' },
    favoritePaths: [],
    selectionFocusPath: null,
    operationPaths: new Set<string>(),
    dropImporting: false,
    canGoBack: false,
    canGoForward: false,
    canGoUp: false,
    setQuery: vi.fn(),
    setSearchScope: vi.fn(),
    setLayout: vi.fn(),
    setSort: vi.fn(),
    selectRoot: vi.fn(async (path: string) => path),
    pickRoot: vi.fn(async () => null),
    navigateTo: vi.fn(),
    goBack: vi.fn(),
    goForward: vi.fn(),
    goUp: vi.fn(),
    refresh: vi.fn(),
    selectEntry: vi.fn(),
    selectAll: vi.fn(),
    replaceSelection: vi.fn(),
    clearSelection: vi.fn(),
    toggleFavoritePath: vi.fn(),
    createFolder: vi.fn(),
    renameEntry: vi.fn(),
    duplicateEntry: vi.fn(),
    deleteEntries: vi.fn(),
    moveEntries: vi.fn(),
    importDroppedItems: vi.fn(),
    ...overrides,
  } as WorkFilesActions;
}

describe('Work Finder file view', () => {
  afterEach(cleanup);

  it('restores the collapsed office sidebar from the file toolbar', () => {
    const onOpenSidebar = vi.fn();
    render(
      <WorkFilesWorkspace
        actions={actions()}
        openingPath={null}
        copilotOpen={false}
        sidebarOpen={false}
        onOpenFile={vi.fn()}
        onAgentRequest={vi.fn()}
        onOpenSidebar={onOpenSidebar}
        onToggleCopilot={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '展开办公侧边栏' }));
    expect(onOpenSidebar).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('button', { name: '切换工作区，当前 docs' })).toBeInTheDocument();
  });

  it('opens supported Office files explicitly as Work copies', () => {
    const openFile = vi.fn();
    render(
      <WorkFilesView
        actions={actions()}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={openFile}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );

    const item = screen.getByRole('option', { name: /Report.docx/ });
    fireEvent.contextMenu(item, { clientX: 30, clientY: 40 });
    fireEvent.click(screen.getByRole('menuitem', { name: '打开' }));
    expect(openFile).toHaveBeenCalledWith(report);
  });

  it('requires an in-product confirmation before permanently deleting local entries', async () => {
    const deleteEntries = vi.fn().mockResolvedValue(undefined);
    render(
      <WorkFilesView
        actions={actions({
          entries: [report, archive],
          visibleEntries: [report, archive],
          selectedPaths: new Set([report.path, archive.path]),
          selectedEntries: [report, archive],
          deleteEntries,
        })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );

    fireEvent.contextMenu(screen.getByRole('option', { name: /Report.docx/ }), { clientX: 30, clientY: 40 });
    fireEvent.click(screen.getByRole('menuitem', { name: '永久删除 2 项' }));
    expect(screen.getByRole('dialog', { name: '永久删除 2 项' })).toBeInTheDocument();
    expect(deleteEntries).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: '确认永久删除' }));
    await waitFor(() => expect(deleteEntries).toHaveBeenCalledWith([report, archive]));
  });

  it.each(['grid', 'list'] as const)('creates a folder inline in %s view', async (layout) => {
    const createFolder = vi.fn().mockResolvedValue(undefined);
    render(
      <WorkFilesView
        actions={actions({ layout, createFolder })}
        openingPath={null}
        createFolderRequest={1}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );

    const input = await screen.findByRole('textbox', { name: '新建文件夹名称' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    fireEvent.change(input, { target: { value: '项目归档' } });
    fireEvent.submit(input.closest('form') as HTMLFormElement);

    await waitFor(() => expect(createFolder).toHaveBeenCalledWith('项目归档'));
    expect(screen.queryByRole('textbox', { name: '新建文件夹名称' })).not.toBeInTheDocument();
  });

  it('renames the selected local file inline from F2 and supports Escape cancellation', async () => {
    const renameEntry = vi.fn().mockResolvedValue(undefined);
    render(
      <WorkFilesView
        actions={actions({
          selectedPaths: new Set([report.path]),
          selectedEntries: [report],
          selectionFocusPath: report.path,
          renameEntry,
        })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );
    const listbox = screen.getByRole('listbox', { name: '本地文件' });

    fireEvent.keyDown(listbox, { key: 'F2' });
    let input = screen.getByRole('textbox', { name: `重命名 ${report.name}` });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(screen.queryByRole('textbox', { name: `重命名 ${report.name}` })).not.toBeInTheDocument();
    expect(renameEntry).not.toHaveBeenCalled();

    fireEvent.keyDown(listbox, { key: 'F2' });
    input = screen.getByRole('textbox', { name: `重命名 ${report.name}` });
    fireEvent.change(input, { target: { value: 'Plan.docx' } });
    fireEvent.submit(input.closest('form') as HTMLFormElement);
    await waitFor(() => expect(renameEntry).toHaveBeenCalledWith(report, 'Plan.docx'));
  });

  it('names a local duplicate inline and keeps the editor open after an error', async () => {
    const duplicateEntry = vi.fn().mockRejectedValueOnce(new Error('目标名称已存在')).mockResolvedValueOnce(undefined);
    render(
      <WorkFilesView
        actions={actions({ duplicateEntry })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );

    fireEvent.contextMenu(screen.getByRole('option', { name: /Report.docx/ }), { clientX: 30, clientY: 40 });
    fireEvent.click(screen.getByRole('menuitem', { name: '创建副本' }));
    const input = screen.getByRole('textbox', { name: `副本名称，来源 ${report.name}` });
    expect(input).toHaveValue('Report 副本.docx');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    fireEvent.change(input, { target: { value: 'Report 备份.docx' } });
    fireEvent.submit(input.closest('form') as HTMLFormElement);
    expect(await screen.findByRole('alert')).toHaveTextContent('目标名称已存在');
    expect(screen.getByRole('textbox', { name: `副本名称，来源 ${report.name}` })).toHaveValue('Report 备份.docx');

    fireEvent.submit(input.closest('form') as HTMLFormElement);
    await waitFor(() => expect(duplicateEntry).toHaveBeenLastCalledWith(report, 'Report 备份.docx'));
  });

  it('rejects a duplicate name that is identical to its source before calling the filesystem', () => {
    const duplicateEntry = vi.fn();
    render(
      <WorkFilesView
        actions={actions({ duplicateEntry })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );

    fireEvent.contextMenu(screen.getByRole('option', { name: /Report.docx/ }), { clientX: 30, clientY: 40 });
    fireEvent.click(screen.getByRole('menuitem', { name: '创建副本' }));
    const input = screen.getByRole('textbox', { name: `副本名称，来源 ${report.name}` });
    fireEvent.change(input, { target: { value: report.name } });
    fireEvent.submit(input.closest('form') as HTMLFormElement);

    expect(screen.getByRole('alert')).toHaveTextContent('副本名称不能与原项目相同');
    expect(input).toHaveValue(report.name);
    expect(duplicateEntry).not.toHaveBeenCalled();
  });

  it('exposes prominent actions for the current multi-selection', () => {
    const notes = {
      ...report,
      name: 'Notes.txt',
      path: '/docs/Notes.txt',
      extension: 'txt',
    };
    const selectAll = vi.fn();
    const clearSelection = vi.fn();
    const onAgentRequest = vi.fn();
    render(
      <WorkFilesView
        actions={actions({
          entries: [report, archive, notes],
          visibleEntries: [report, archive, notes],
          selectedPaths: new Set([report.path, archive.path]),
          selectedEntries: [report, archive],
          selectAll,
          clearSelection,
        })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={onAgentRequest}
      />
    );

    expect(screen.getByRole('toolbar', { name: '已选文件操作' })).toHaveTextContent('已选择 2 项');
    fireEvent.click(screen.getByRole('button', { name: '选择全部 3 项' }));
    expect(selectAll).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: '询问 AI 助手' }));
    expect(onAgentRequest).toHaveBeenCalledWith(
      expect.objectContaining({
        paths: [report.path, archive.path],
      })
    );

    fireEvent.click(screen.getByRole('button', { name: '取消选择' }));
    expect(clearSelection).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: '永久删除所选 2 项' }));
    expect(screen.getByRole('dialog', { name: '永久删除 2 项' })).toBeInTheDocument();
  });

  it('creates native Office files from the current-folder context menu', () => {
    const onCreateArtifact = vi.fn();
    render(
      <WorkFilesView
        actions={actions()}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
        onCreateArtifact={onCreateArtifact}
      />
    );

    fireEvent.contextMenu(screen.getByRole('listbox', { name: '本地文件' }), { clientX: 40, clientY: 60 });
    fireEvent.click(screen.getByRole('menuitem', { name: '新建文字文档' }));
    expect(onCreateArtifact).toHaveBeenCalledWith('blank-document');
  });

  it.each([
    ['grid', 'blank-document', '新建文字文档名称', '新建文字文档.docx', '项目计划.docx'],
    ['list', 'blank-spreadsheet', '新建电子表格名称', '新建电子表格.xlsx', '项目预算.xlsx'],
    ['grid', 'blank-presentation', '新建演示文稿名称', '新建演示文稿.pptx', '项目汇报.pptx'],
  ] as const)('creates %s-view Office files inline for %s', async (layout, templateId, inputLabel, defaultName, requestedName) => {
    const request = { requestId: 7, templateId, directory: '/docs' };
    const onCreateArtifactFile = vi.fn().mockResolvedValue('created');
    const onConsumeCreateArtifactRequest = vi.fn();
    render(
      <WorkFilesView
        actions={actions({ layout })}
        openingPath={null}
        createFolderRequest={0}
        createArtifactRequest={request}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
        onCreateArtifactFile={onCreateArtifactFile}
        onConsumeCreateArtifactRequest={onConsumeCreateArtifactRequest}
      />
    );

    const input = await screen.findByRole('textbox', { name: inputLabel });
    expect(input).toHaveValue(defaultName);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(onConsumeCreateArtifactRequest).toHaveBeenCalledTimes(1);

    fireEvent.change(input, { target: { value: requestedName } });
    fireEvent.submit(input.closest('form') as HTMLFormElement);
    await waitFor(() => expect(onCreateArtifactFile).toHaveBeenCalledWith(request, requestedName));
    expect(screen.queryByRole('textbox', { name: inputLabel })).not.toBeInTheDocument();
  });

  it('keeps inline Office creation open when the destination name exists', async () => {
    const request = { requestId: 8, templateId: 'blank-document', directory: '/docs' };
    const onCreateArtifactFile = vi.fn().mockResolvedValue('exists');
    render(
      <WorkFilesView
        actions={actions()}
        openingPath={null}
        createFolderRequest={0}
        createArtifactRequest={request}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
        onCreateArtifactFile={onCreateArtifactFile}
      />
    );

    const input = await screen.findByRole('textbox', { name: '新建文字文档名称' });
    fireEvent.submit(input.closest('form') as HTMLFormElement);

    expect(await screen.findByRole('alert')).toHaveTextContent('当前文件夹中已有同名文件');
    expect(input).toHaveValue('新建文字文档.docx');
  });

  it('prefills a selection-aware Copilot request without sending it', () => {
    const onAgentRequest = vi.fn();
    render(
      <WorkFilesView
        actions={actions({
          selectedPaths: new Set([report.path]),
          selectedEntries: [report],
        })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={onAgentRequest}
      />
    );

    fireEvent.contextMenu(screen.getByRole('option', { name: /Report.docx/ }), { clientX: 30, clientY: 40 });
    fireEvent.click(screen.getByRole('menuitem', { name: '总结文件' }));
    expect(onAgentRequest).toHaveBeenCalledWith(
      expect.objectContaining({
        workspaceRoot: '',
        paths: ['/docs/Report.docx'],
        instruction: expect.stringContaining('总结'),
      })
    );
  });

  it('switches between current-folder and whole-workspace search and shows result locations', () => {
    const setSearchScope = vi.fn();
    const nestedReport = { ...report, path: '/docs/Reports/Report.docx' };
    const { rerender } = render(
      <WorkFilesWorkspace
        actions={actions({
          currentPath: '/docs/Reports',
          entries: [],
          visibleEntries: [nestedReport],
          query: 'report',
          searchScope: 'workspace',
          setSearchScope,
        })}
        openingPath={null}
        copilotOpen={false}
        sidebarOpen={true}
        onOpenFile={vi.fn()}
        onAgentRequest={vi.fn()}
        onOpenSidebar={vi.fn()}
        onToggleCopilot={vi.fn()}
      />
    );

    expect(screen.getByRole('option', { name: /Report.docx/ })).toHaveTextContent('Reports');
    fireEvent.click(screen.getByRole('radio', { name: '仅搜索当前文件夹 Reports' }));
    expect(setSearchScope).toHaveBeenCalledWith('folder');
    rerender(
      <WorkFilesWorkspace
        actions={actions({
          currentPath: '/docs/Reports',
          entries: [],
          visibleEntries: [nestedReport],
          query: 'report',
          searchScope: 'folder',
          setSearchScope,
        })}
        openingPath={null}
        copilotOpen={false}
        sidebarOpen={true}
        onOpenFile={vi.fn()}
        onAgentRequest={vi.fn()}
        onOpenSidebar={vi.fn()}
        onToggleCopilot={vi.fn()}
      />
    );
    fireEvent.click(screen.getByRole('radio', { name: '搜索全部文件 docs' }));
    expect(setSearchScope).toHaveBeenCalledWith('workspace');
  });

  it('sorts with an in-product menu instead of a system select', () => {
    const setSort = vi.fn();
    const { container } = render(
      <WorkFilesWorkspace
        actions={actions({ setSort })}
        openingPath={null}
        copilotOpen={false}
        sidebarOpen={true}
        onOpenFile={vi.fn()}
        onAgentRequest={vi.fn()}
        onOpenSidebar={vi.fn()}
        onToggleCopilot={vi.fn()}
      />
    );

    expect(container.querySelector('select')).toBeNull();
    fireEvent.click(screen.getByRole('combobox', { name: '排序方式' }));
    fireEvent.click(screen.getByRole('option', { name: '修改日期' }));
    expect(setSort).toHaveBeenCalledWith({ key: 'modified', direction: 'ascending' });
  });

  it('sorts directly from list headers and exposes the active direction', () => {
    const setSort = vi.fn();
    const { rerender } = render(
      <WorkFilesView
        actions={actions({
          entries: [archive, report],
          visibleEntries: [archive, report],
          layout: 'list',
          sort: { key: 'name', direction: 'ascending' },
          setSort,
        })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );

    const nameHeader = screen.getByRole('columnheader', { name: /名称/ });
    expect(nameHeader).toHaveAttribute('aria-sort', 'ascending');
    fireEvent.click(screen.getByRole('button', { name: '名称，当前升序，切换为降序' }));
    expect(setSort).toHaveBeenCalledWith({ key: 'name', direction: 'descending' });

    fireEvent.click(screen.getByRole('button', { name: '修改日期，点击排序' }));
    expect(setSort).toHaveBeenLastCalledWith({ key: 'modified', direction: 'ascending' });

    rerender(
      <WorkFilesView
        actions={actions({
          entries: [archive, report],
          visibleEntries: [archive, report],
          layout: 'list',
          sort: { key: 'size', direction: 'descending' },
          setSort,
        })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );
    expect(screen.getByRole('columnheader', { name: /大小/ })).toHaveAttribute('aria-sort', 'descending');
  });

  it('opens Quick Look from the Space key and the contextual action', () => {
    const onQuickLook = vi.fn();
    const selectedActions = actions({
      selectedPaths: new Set([report.path]),
      selectedEntries: [report],
    });
    render(
      <WorkFilesView
        actions={selectedActions}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={onQuickLook}
        onAgentRequest={vi.fn()}
      />
    );

    fireEvent.keyDown(screen.getByRole('listbox', { name: '本地文件' }), { key: ' ' });
    expect(onQuickLook).toHaveBeenCalledWith(report);

    onQuickLook.mockClear();
    fireEvent.contextMenu(screen.getByRole('option', { name: /Report.docx/ }), { clientX: 30, clientY: 40 });
    fireEvent.click(screen.getByRole('menuitem', { name: '快速查看' }));
    expect(onQuickLook).toHaveBeenCalledWith(report);
  });

  it('opens Quick Look for the selected item from the Finder toolbar', async () => {
    render(
      <WorkFilesWorkspace
        actions={actions({
          entries: [archive],
          visibleEntries: [archive],
          selectedPaths: new Set([archive.path]),
          selectedEntries: [archive],
        })}
        openingPath={null}
        copilotOpen={false}
        sidebarOpen={true}
        onOpenFile={vi.fn()}
        onAgentRequest={vi.fn()}
        onOpenSidebar={vi.fn()}
        onToggleCopilot={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '快速查看所选项目' }));

    expect(screen.getByRole('dialog', { name: 'Archive' })).toBeInTheDocument();
    expect(await screen.findByText('快速查看不会读取文件夹内的内容；打开文件夹后可以继续浏览。')).toBeInTheDocument();
  });

  it('moves selected files to an ancestor breadcrumb', () => {
    const nestedReport = { ...report, path: '/docs/Reports/Report.docx' };
    const moveEntries = vi.fn().mockResolvedValue(undefined);
    render(
      <WorkFilesWorkspace
        actions={actions({
          currentPath: '/docs/Reports',
          entries: [nestedReport],
          visibleEntries: [nestedReport],
          selectedPaths: new Set([nestedReport.path]),
          selectedEntries: [nestedReport],
          moveEntries,
          canGoUp: true,
        })}
        openingPath={null}
        copilotOpen={false}
        sidebarOpen={true}
        onOpenFile={vi.fn()}
        onAgentRequest={vi.fn()}
        onOpenSidebar={vi.fn()}
        onToggleCopilot={vi.fn()}
      />
    );
    const dataTransfer = {
      dropEffect: 'none',
      types: ['application/x-a3s-work-local-paths'],
      getData: vi.fn(() => JSON.stringify([nestedReport.path])),
    };
    const rootBreadcrumb = screen.getByRole('button', { name: 'docs' });

    fireEvent.dragOver(rootBreadcrumb, { dataTransfer });
    expect(rootBreadcrumb).toHaveClass('drop-target');
    fireEvent.drop(rootBreadcrumb, { dataTransfer });
    expect(moveEntries).toHaveBeenCalledWith([nestedReport.path], '/docs');
  });

  it('imports operating-system files into ancestor breadcrumbs', () => {
    const importDroppedItems = vi.fn().mockResolvedValue(undefined);
    render(
      <WorkFilesWorkspace
        actions={actions({
          currentPath: '/docs/Reports',
          entries: [report],
          visibleEntries: [report],
          importDroppedItems,
          canGoUp: true,
        })}
        openingPath={null}
        copilotOpen={false}
        sidebarOpen={true}
        onOpenFile={vi.fn()}
        onAgentRequest={vi.fn()}
        onOpenSidebar={vi.fn()}
        onToggleCopilot={vi.fn()}
      />
    );
    const dataTransfer = operatingSystemDrop();
    const rootBreadcrumb = screen.getByRole('button', { name: 'docs' });

    fireEvent.dragOver(rootBreadcrumb, { dataTransfer });
    expect(rootBreadcrumb).toHaveClass('drop-target');
    fireEvent.drop(rootBreadcrumb, { dataTransfer });
    expect(importDroppedItems).toHaveBeenCalledWith(dataTransfer, '/docs');
  });

  it('moves dragged selections into folders and exposes sidebar favorites in the context menu', () => {
    const moveEntries = vi.fn();
    const toggleFavoritePath = vi.fn();
    const finderActions = actions({
      entries: [archive, report],
      visibleEntries: [archive, report],
      moveEntries,
      toggleFavoritePath,
    });
    render(
      <WorkFilesView
        actions={finderActions}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );
    const data = new Map<string, string>();
    const dataTransfer = {
      effectAllowed: 'none',
      dropEffect: 'none',
      get types() {
        return [...data.keys()];
      },
      setData: vi.fn((type: string, value: string) => data.set(type, value)),
      getData: vi.fn((type: string) => data.get(type) ?? ''),
    };
    const reportItem = screen.getByRole('option', { name: /Report.docx/ });
    const archiveItem = screen.getByRole('option', { name: /Archive/ });

    fireEvent.dragStart(reportItem, { dataTransfer });
    fireEvent.dragOver(archiveItem, { dataTransfer });
    expect(archiveItem).toHaveClass('drop-target');
    fireEvent.drop(archiveItem, { dataTransfer });
    expect(moveEntries).toHaveBeenCalledWith(['/docs/Report.docx'], '/docs/Archive');

    fireEvent.contextMenu(archiveItem, { clientX: 30, clientY: 40 });
    fireEvent.click(screen.getByRole('menuitem', { name: '添加到侧边栏' }));
    expect(toggleFavoritePath).toHaveBeenCalledWith('/docs/Archive');
  });

  it('supports Finder arrow selection and command-up navigation', () => {
    const selectEntry = vi.fn();
    const goUp = vi.fn();
    render(
      <WorkFilesView
        actions={actions({
          entries: [archive, report],
          visibleEntries: [archive, report],
          selectedPaths: new Set([archive.path]),
          selectedEntries: [archive],
          selectionFocusPath: archive.path,
          layout: 'list',
          selectEntry,
          goUp,
        })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );
    const listbox = screen.getByRole('listbox', { name: '本地文件' });

    fireEvent.keyDown(listbox, { key: 'ArrowDown', shiftKey: true });
    expect(selectEntry).toHaveBeenCalledWith(report, { range: true });
    expect(screen.getByRole('option', { name: /Report.docx/ })).toHaveFocus();

    fireEvent.keyDown(listbox, { key: 'ArrowUp', metaKey: true });
    expect(goUp).toHaveBeenCalledTimes(1);
  });

  it('maps command-shift click to an additive range selection', () => {
    const selectEntry = vi.fn();
    render(
      <WorkFilesView
        actions={actions({
          entries: [archive, report],
          visibleEntries: [archive, report],
          selectedPaths: new Set([archive.path]),
          selectedEntries: [archive],
          selectionFocusPath: archive.path,
          selectEntry,
        })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('option', { name: /Report.docx/ }), {
      metaKey: true,
      shiftKey: true,
    });
    expect(selectEntry).toHaveBeenCalledWith(report, {
      toggle: true,
      range: true,
      additive: true,
    });
  });

  it.each(['grid', 'list'] as const)('selects intersecting files with a mouse marquee in %s view', (layout) => {
    const replaceSelection = vi.fn();
    render(
      <WorkFilesView
        actions={actions({
          entries: [archive, report],
          visibleEntries: [archive, report],
          layout,
          replaceSelection,
        })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );
    const listbox = screen.getByRole('listbox', { name: '本地文件' });
    const [archiveItem, reportItem] = screen.getAllByRole('option');
    vi.spyOn(listbox, 'getBoundingClientRect').mockReturnValue(new DOMRect(0, 0, 500, 300));
    vi.spyOn(archiveItem, 'getBoundingClientRect').mockReturnValue(new DOMRect(20, 20, 90, 70));
    vi.spyOn(reportItem, 'getBoundingClientRect').mockReturnValue(new DOMRect(150, 20, 90, 70));
    Object.assign(listbox, {
      setPointerCapture: vi.fn(),
      hasPointerCapture: vi.fn(() => true),
      releasePointerCapture: vi.fn(),
    });

    fireEvent.pointerDown(listbox, {
      pointerId: 4,
      button: 0,
      isPrimary: true,
      clientX: 5,
      clientY: 5,
    });
    fireEvent.pointerMove(listbox, {
      pointerId: 4,
      clientX: 130,
      clientY: 100,
    });

    expect(replaceSelection).toHaveBeenLastCalledWith([archive.path]);
    expect(listbox.querySelector('.work-files-marquee')).toBeInTheDocument();

    fireEvent.pointerUp(listbox, {
      pointerId: 4,
      clientX: 130,
      clientY: 100,
    });
    expect(listbox.querySelector('.work-files-marquee')).not.toBeInTheDocument();
  });

  it('keeps selection actions below the file surface so item coordinates stay stable', () => {
    render(
      <WorkFilesView
        actions={actions({
          entries: [archive, report],
          visibleEntries: [archive, report],
          selectedPaths: new Set([archive.path]),
          selectedEntries: [archive],
        })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );
    const listbox = screen.getByRole('listbox', { name: '本地文件' });
    const toolbar = screen.getByRole('toolbar', { name: '已选文件操作' });

    expect(listbox.nextElementSibling).toBe(toolbar);
  });

  it.each(['grid', 'list'] as const)('toggles an item from its visible selection control in %s view', (layout) => {
    const selectEntry = vi.fn();
    render(
      <WorkFilesView
        actions={actions({
          entries: [archive, report],
          visibleEntries: [archive, report],
          layout,
          selectEntry,
        })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );

    const reportItem = screen.getByRole('option', { name: /Report.docx/ });
    const selectionControl = reportItem.querySelector('[data-work-file-selection-control]');
    expect(selectionControl).toBeInTheDocument();
    fireEvent.click(selectionControl as Element);
    expect(selectEntry).toHaveBeenCalledWith(
      report,
      expect.objectContaining({
        toggle: true,
      })
    );
  });

  it('selects and clears every visible row from the list header checkbox', () => {
    const selectAll = vi.fn();
    const clearSelection = vi.fn();
    const { rerender } = render(
      <WorkFilesView
        actions={actions({
          entries: [archive, report],
          visibleEntries: [archive, report],
          selectedPaths: new Set([archive.path]),
          selectedEntries: [archive],
          layout: 'list',
          selectAll,
          clearSelection,
        })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );

    const partialCheckbox = screen.getByRole('checkbox', { name: '选择全部 2 项' });
    expect(partialCheckbox).toHaveAttribute('aria-checked', 'mixed');
    fireEvent.click(partialCheckbox);
    expect(selectAll).toHaveBeenCalledTimes(1);

    rerender(
      <WorkFilesView
        actions={actions({
          entries: [archive, report],
          visibleEntries: [archive, report],
          selectedPaths: new Set([archive.path, report.path]),
          selectedEntries: [archive, report],
          layout: 'list',
          selectAll,
          clearSelection,
        })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );
    const checkedCheckbox = screen.getByRole('checkbox', { name: '取消选择全部 2 项' });
    expect(checkedCheckbox).toHaveAttribute('aria-checked', 'true');
    fireEvent.click(checkedCheckbox);
    expect(clearSelection).toHaveBeenCalledTimes(1);
  });

  it('imports operating-system drops into visible folders and the current folder background', () => {
    const importDroppedItems = vi.fn().mockResolvedValue(undefined);
    render(
      <WorkFilesView
        actions={actions({
          entries: [archive, report],
          visibleEntries: [archive, report],
          importDroppedItems,
        })}
        openingPath={null}
        createFolderRequest={0}
        onOpenFile={vi.fn()}
        onQuickLook={vi.fn()}
        onAgentRequest={vi.fn()}
      />
    );
    const archiveItem = screen.getByRole('option', { name: /Archive/ });
    const listbox = screen.getByRole('listbox', { name: '本地文件' });
    const folderDrop = operatingSystemDrop();

    fireEvent.dragOver(archiveItem, { dataTransfer: folderDrop });
    expect(archiveItem).toHaveClass('external-drop-target');
    fireEvent.drop(archiveItem, { dataTransfer: folderDrop });
    expect(importDroppedItems).toHaveBeenCalledWith(folderDrop, '/docs/Archive');

    const backgroundDrop = operatingSystemDrop();
    fireEvent.dragOver(listbox, { dataTransfer: backgroundDrop });
    expect(screen.getByRole('status')).toHaveTextContent('松开放入当前文件夹');
    fireEvent.drop(listbox, { dataTransfer: backgroundDrop });
    expect(importDroppedItems).toHaveBeenCalledWith(backgroundDrop, '/docs');
  });
});

function operatingSystemDrop(): DataTransfer {
  const file = {
    name: 'notes.txt',
    size: 3,
    webkitRelativePath: '',
    arrayBuffer: async () => Uint8Array.from([1, 2, 3]).buffer,
  } as File;
  return {
    types: ['Files'],
    items: [],
    files: [file],
    dropEffect: 'none',
  } as unknown as DataTransfer;
}
