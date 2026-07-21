import {
  ArrowDownAZ,
  ArrowDownZA,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  Eye,
  FolderOpen,
  FolderPlus,
  Grid2X2,
  List,
  RefreshCw,
  Search,
  Sparkles,
} from 'lucide-react';
import { useEffect, useState } from 'react';
import type { WorkspaceEntry } from '../../../types/api';
import { hasDraggedWorkspaceFiles } from '../../workspace/workspace-drop-import';
import type { WorkAgentRequest } from '../work-agent-request';
import type { WorkFilesActions } from '../use-work-files-controller';
import {
  canMoveLocalPaths,
  hasWorkLocalFileDragData,
  localPathBasename,
  readWorkLocalFileDragData,
  workBreadcrumbs,
  type WorkFilesSortKey,
} from '../work-local-files';
import { WorkFilesView } from './work-files-view';
import { WorkQuickLook } from './work-quick-look';

export function WorkFilesWorkspace({
  actions,
  openingPath,
  copilotOpen,
  onOpenFile,
  onAgentRequest,
  onCreateArtifact,
  onToggleCopilot,
}: {
  actions: WorkFilesActions;
  openingPath: string | null;
  copilotOpen: boolean;
  onOpenFile: (entry: WorkspaceEntry) => void | Promise<void>;
  onAgentRequest: (request: WorkAgentRequest) => void | Promise<void>;
  onCreateArtifact?: (templateId: string) => void;
  onToggleCopilot: () => void;
}) {
  const [createFolderRequest, setCreateFolderRequest] = useState(0);
  const [quickLookPath, setQuickLookPath] = useState<string | null>(null);
  const [breadcrumbDropPath, setBreadcrumbDropPath] = useState<string | null>(null);
  const [breadcrumbDropCopies, setBreadcrumbDropCopies] = useState(false);
  const workspaceSearching = actions.searchScope === 'workspace' && Boolean(actions.query.trim());
  const quickLookEntry = quickLookPath
    ? (actions.visibleEntries.find((entry) => entry.path === quickLookPath) ?? null)
    : null;
  const quickLookIndex = quickLookEntry
    ? actions.visibleEntries.findIndex((entry) => entry.path === quickLookEntry.path)
    : -1;
  const previousQuickLookEntry = quickLookIndex > 0 ? actions.visibleEntries[quickLookIndex - 1] : null;
  const nextQuickLookEntry =
    quickLookIndex >= 0 && quickLookIndex < actions.visibleEntries.length - 1
      ? actions.visibleEntries[quickLookIndex + 1]
      : null;
  useEffect(() => {
    setQuickLookPath(null);
  }, [actions.currentPath]);
  useEffect(() => {
    if (quickLookPath && !actions.visibleEntries.some((entry) => entry.path === quickLookPath)) setQuickLookPath(null);
  }, [actions.visibleEntries, quickLookPath]);
  if (!actions.rootPath) {
    return (
      <main className='work-files-onboarding'>
        <span className='work-files-onboarding-icon'>
          <FolderOpen size={30} />
        </span>
        <span className='eyebrow'>A3S WORK · LOCAL FILES</span>
        <h1>把本地文件夹变成智能工作台</h1>
        <p>像在访达中一样浏览真实文件，并在右侧随时调用 AI 助手。A3S 只会在你明确操作时读写本地内容。</p>
        <button type='button' onClick={() => void actions.pickRoot()}>
          <FolderOpen size={16} />
          选择本地文件夹
        </button>
        <small>支持目录浏览、搜索、排序、新建文件夹、重命名和创建副本。</small>
      </main>
    );
  }

  const breadcrumbs = workBreadcrumbs(actions.rootPath, actions.currentPath);
  return (
    <main className='work-files-workspace'>
      <header className='work-files-toolbar'>
        <div className='work-files-history-actions'>
          <button type='button' aria-label='后退' disabled={!actions.canGoBack} onClick={actions.goBack}>
            <ChevronLeft size={17} />
          </button>
          <button type='button' aria-label='前进' disabled={!actions.canGoForward} onClick={actions.goForward}>
            <ChevronRight size={17} />
          </button>
          <button type='button' aria-label='上一级文件夹' disabled={!actions.canGoUp} onClick={actions.goUp}>
            <ChevronUp size={17} />
          </button>
        </div>
        <nav className='work-files-breadcrumbs' aria-label='当前文件夹路径' title={actions.currentPath}>
          {breadcrumbs.map((breadcrumb, index) => (
            <span key={breadcrumb.path}>
              {index > 0 && <ChevronRight size={12} aria-hidden='true' />}
              <button
                type='button'
                className={
                  breadcrumbDropPath === breadcrumb.path
                    ? `drop-target ${breadcrumbDropCopies ? 'external-drop-target' : ''}`
                    : ''
                }
                aria-current={index === breadcrumbs.length - 1 ? 'location' : undefined}
                onClick={() => actions.navigateTo(breadcrumb.path)}
                onDragOver={(event) => {
                  if (hasDraggedWorkspaceFiles(event.dataTransfer)) {
                    event.preventDefault();
                    event.stopPropagation();
                    event.dataTransfer.dropEffect = actions.dropImporting ? 'none' : 'copy';
                    setBreadcrumbDropPath(breadcrumb.path);
                    setBreadcrumbDropCopies(true);
                    return;
                  }
                  if (!hasWorkLocalFileDragData(event.dataTransfer)) return;
                  const paths = readWorkLocalFileDragData(event.dataTransfer);
                  if (paths.length && !canMoveLocalPaths(paths, breadcrumb.path)) return;
                  event.preventDefault();
                  event.dataTransfer.dropEffect = 'move';
                  setBreadcrumbDropPath(breadcrumb.path);
                  setBreadcrumbDropCopies(false);
                }}
                onDragLeave={() => {
                  setBreadcrumbDropPath(null);
                  setBreadcrumbDropCopies(false);
                }}
                onDrop={(event) => {
                  if (hasDraggedWorkspaceFiles(event.dataTransfer)) {
                    event.preventDefault();
                    event.stopPropagation();
                    setBreadcrumbDropPath(null);
                    setBreadcrumbDropCopies(false);
                    if (actions.dropImporting) return;
                    void Promise.resolve(actions.importDroppedItems(event.dataTransfer, breadcrumb.path)).catch(
                      () => undefined
                    );
                    return;
                  }
                  const paths = readWorkLocalFileDragData(event.dataTransfer);
                  setBreadcrumbDropPath(null);
                  setBreadcrumbDropCopies(false);
                  if (!canMoveLocalPaths(paths, breadcrumb.path)) return;
                  event.preventDefault();
                  void Promise.resolve(actions.moveEntries(paths, breadcrumb.path)).catch(() => undefined);
                }}
              >
                {breadcrumb.label}
              </button>
            </span>
          ))}
        </nav>
        <label className='work-files-search'>
          <Search size={14} />
          <input
            type='search'
            aria-label={workspaceSearching ? '搜索整个工作区' : '搜索当前文件夹'}
            placeholder='搜索'
            value={actions.query}
            onChange={(event) => actions.setQuery(event.target.value)}
          />
        </label>
        <div className='work-files-toolbar-actions'>
          <button
            type='button'
            aria-label='新建文件夹'
            disabled={actions.loading || actions.dropImporting || Boolean(actions.error)}
            onClick={() => setCreateFolderRequest((value) => value + 1)}
          >
            <FolderPlus size={16} />
          </button>
          <button
            type='button'
            aria-label='刷新当前文件夹'
            disabled={actions.loading || actions.dropImporting}
            onClick={() => void actions.refresh()}
          >
            <RefreshCw className={actions.loading || actions.dropImporting ? 'spin' : ''} size={15} />
          </button>
          <button
            type='button'
            aria-label='快速查看所选项目'
            disabled={actions.selectedEntries.length !== 1}
            onClick={() => setQuickLookPath(actions.selectedEntries[0]?.path ?? null)}
          >
            <Eye size={15} />
          </button>
          <label className='work-files-sort-select'>
            <span className='sr-only'>排序方式</span>
            <select
              aria-label='排序方式'
              value={actions.sort.key}
              onChange={(event) =>
                actions.setSort({
                  ...actions.sort,
                  key: event.target.value as WorkFilesSortKey,
                })
              }
            >
              <option value='name'>名称</option>
              <option value='modified'>修改日期</option>
              <option value='size'>大小</option>
              <option value='kind'>种类</option>
            </select>
          </label>
          <button
            type='button'
            aria-label={actions.sort.direction === 'ascending' ? '切换为降序' : '切换为升序'}
            onClick={() =>
              actions.setSort({
                ...actions.sort,
                direction: actions.sort.direction === 'ascending' ? 'descending' : 'ascending',
              })
            }
          >
            {actions.sort.direction === 'ascending' ? <ArrowDownAZ size={15} /> : <ArrowDownZA size={15} />}
          </button>
          <fieldset className='work-files-layout-toggle'>
            <legend className='sr-only'>文件布局</legend>
            <button
              type='button'
              className={actions.layout === 'grid' ? 'active' : ''}
              aria-label='图标视图'
              onClick={() => actions.setLayout('grid')}
            >
              <Grid2X2 size={15} />
            </button>
            <button
              type='button'
              className={actions.layout === 'list' ? 'active' : ''}
              aria-label='列表视图'
              onClick={() => actions.setLayout('list')}
            >
              <List size={16} />
            </button>
          </fieldset>
          <button
            type='button'
            className={`work-copilot-toggle ${copilotOpen ? 'active' : ''}`}
            aria-label={copilotOpen ? '关闭 Work AI 助手' : '打开 Work AI 助手'}
            aria-pressed={copilotOpen}
            onClick={onToggleCopilot}
          >
            <Sparkles size={15} />
            <span>AI 助手</span>
          </button>
        </div>
      </header>
      {actions.query.trim() && (
        <section className='work-files-search-scope' aria-label='搜索范围'>
          <span>搜索范围</span>
          <button
            type='button'
            className={actions.searchScope === 'folder' ? 'active' : ''}
            aria-label={`仅搜索当前文件夹 ${localPathBasename(actions.currentPath)}`}
            aria-pressed={actions.searchScope === 'folder'}
            onClick={() => actions.setSearchScope('folder')}
          >
            {localPathBasename(actions.currentPath)}
          </button>
          <button
            type='button'
            className={actions.searchScope === 'workspace' ? 'active' : ''}
            aria-label={`搜索整个工作区 ${localPathBasename(actions.rootPath)}`}
            aria-pressed={actions.searchScope === 'workspace'}
            onClick={() => actions.setSearchScope('workspace')}
          >
            整个“{localPathBasename(actions.rootPath)}”
          </button>
          {workspaceSearching && actions.searchError && (
            <span className='error' role='alert'>
              搜索失败：{actions.searchError}
            </span>
          )}
          {workspaceSearching && !actions.searchError && actions.searchTruncated && (
            <span className='notice'>结果已达到安全上限，仅显示已找到的部分项目。</span>
          )}
          {workspaceSearching && actions.searchUnreadableDirectories > 0 && (
            <span className='notice'>已略过 {actions.searchUnreadableDirectories} 个无法读取的文件夹。</span>
          )}
        </section>
      )}
      {actions.error && !actions.loading ? (
        <section className='work-files-load-error' role='alert'>
          <span>
            <FolderOpen size={24} />
          </span>
          <strong>无法读取这个文件夹</strong>
          <p>{actions.error}</p>
          <div>
            <button type='button' onClick={() => void actions.refresh()}>
              重试
            </button>
            <button type='button' onClick={() => void actions.pickRoot()}>
              选择其他文件夹
            </button>
          </div>
        </section>
      ) : (
        <WorkFilesView
          actions={actions}
          openingPath={openingPath}
          createFolderRequest={createFolderRequest}
          onOpenFile={onOpenFile}
          onQuickLook={(entry) => setQuickLookPath(entry.path)}
          onAgentRequest={(request) =>
            onAgentRequest({
              ...request,
              workspaceRoot: actions.rootPath,
            })
          }
          onCreateArtifact={onCreateArtifact}
        />
      )}
      {(actions.loading || actions.dropImporting || actions.searchLoading) && (
        <output className='work-files-loading' aria-live='polite'>
          <span />
          {actions.dropImporting ? '正在复制拖入项目…' : actions.searchLoading ? '正在搜索工作区…' : '正在读取文件夹…'}
        </output>
      )}
      {quickLookEntry && (
        <WorkQuickLook
          entry={quickLookEntry}
          previousEntry={previousQuickLookEntry}
          nextEntry={nextQuickLookEntry}
          onNavigate={(entry) => {
            actions.selectEntry(entry);
            setQuickLookPath(entry.path);
          }}
          onOpen={(entry) => {
            if (entry.isDirectory) actions.navigateTo(entry.path);
            else void onOpenFile(entry);
          }}
          onClose={() => setQuickLookPath(null)}
        />
      )}
    </main>
  );
}
