import { FileText, FolderPlus, Grid2X2, List, LoaderCircle, Upload } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Button, IconButton, SearchField, StateView } from '../../../design-system/primitives';
import type { TaskActions } from '../../tasks/task-actions';
import type { WorkArtifact, WorkFolder, WorkLibraryView } from '../work-types';
import { WorkHomeHero } from './work-home-hero';
import {
  ArtifactCard,
  ArtifactInlineEditorCard,
  FolderCard,
  FolderInlineEditorCard,
  type WorkLibraryInlineOperation,
} from './work-library-cards';
import { WorkLibraryDeleteDialog, type WorkLibraryDeleteTarget } from './work-library-delete-dialog';
import { ConversationSidebarOpenButton } from './conversation-sidebar-open-button';

interface WorkHomeProps {
  artifacts: WorkArtifact[];
  folders: WorkFolder[];
  view: WorkLibraryView;
  activeFolderId: string | null;
  loading: boolean;
  error: string | null;
  sidebarOpen: boolean;
  taskActions: TaskActions;
  activeSessionTitle?: string | null;
  onOpenSidebar: () => void;
  onContinueSession?: () => void;
  onNewTask?: () => void;
  onTaskSubmit: (content: string) => void;
  onOpenWorkspace: () => void;
  onCreate: (templateId: string) => void;
  onOpen: (id: string) => void;
  onImport: () => void;
  onChangeView?: (view: WorkLibraryView) => void;
  onToggleFavorite: (id: string) => void;
  onRename: (id: string, title: string) => void | Promise<void>;
  onCopy: (id: string) => void;
  onMove: (id: string, folderId: string | null) => void;
  onRestore: (id: string) => void;
  onDelete: (artifact: WorkArtifact) => void;
  onOpenFolder: (id: string) => void;
  onCreateFolder: (name: string) => void | Promise<void>;
  onRenameFolder: (id: string, name: string) => void | Promise<void>;
  onRestoreFolder: (id: string) => void;
  onDeleteFolder: (folder: WorkFolder) => void;
  onRetry: () => void;
}

export function WorkHome({
  artifacts,
  folders,
  view,
  activeFolderId,
  loading,
  error,
  sidebarOpen,
  taskActions,
  activeSessionTitle = null,
  onOpenSidebar,
  onContinueSession,
  onNewTask,
  onTaskSubmit,
  onOpenWorkspace,
  onCreate,
  onOpen,
  onImport,
  onChangeView,
  onToggleFavorite,
  onRename,
  onCopy,
  onMove,
  onRestore,
  onDelete,
  onOpenFolder,
  onCreateFolder,
  onRenameFolder,
  onRestoreFolder,
  onDeleteFolder,
  onRetry,
}: WorkHomeProps) {
  const [query, setQuery] = useState('');
  const [layout, setLayout] = useState<'grid' | 'list'>('grid');
  const [inlineOperation, setInlineOperation] = useState<WorkLibraryInlineOperation | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<WorkLibraryDeleteTarget | null>(null);
  const nextInlineOperationIdRef = useRef(0);
  const visibleArtifacts = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    return artifacts.filter((artifact) => {
      if (view === 'trash') {
        return Boolean(artifact.trashedAt) && (!normalized || artifact.title.toLowerCase().includes(normalized));
      }
      if (artifact.trashedAt) return false;
      if (view === 'favorites' && !artifact.favorite) return false;
      if (view === 'folder' && (artifact.folderId ?? null) !== activeFolderId) return false;
      return !normalized || artifact.title.toLowerCase().includes(normalized);
    });
  }, [activeFolderId, artifacts, query, view]);
  const visibleFolders = useMemo(() => {
    if (view === 'trash') return folders.filter((folder) => folder.trashedAt);
    if (view !== 'home' && view !== 'folder') return [];
    const parentId = view === 'folder' ? activeFolderId : null;
    return folders.filter((folder) => !folder.trashedAt && (folder.parentId ?? null) === parentId);
  }, [activeFolderId, folders, view]);
  const activeFolder = folders.find((folder) => folder.id === activeFolderId);
  const heading =
    view === 'favorites'
      ? '收藏文件'
      : view === 'recent'
        ? '最近打开'
        : view === 'trash'
          ? '回收站'
          : view === 'folder'
            ? (activeFolder?.name ?? '文件夹')
            : '我的文档';
  useEffect(() => {
    setInlineOperation(null);
  }, [activeFolderId, view]);
  const startCreateFolder = () => {
    const value = '新建文件夹';
    nextInlineOperationIdRef.current += 1;
    setInlineOperation({
      operationId: nextInlineOperationIdRef.current,
      kind: 'create-folder',
      value,
      initialValue: value,
      submitting: false,
      error: null,
    });
  };
  const startRenameFolder = (folder: WorkFolder) => {
    nextInlineOperationIdRef.current += 1;
    setInlineOperation({
      operationId: nextInlineOperationIdRef.current,
      kind: 'rename-folder',
      folder,
      value: folder.name,
      initialValue: folder.name,
      submitting: false,
      error: null,
    });
  };
  const startRenameArtifact = (artifact: WorkArtifact) => {
    nextInlineOperationIdRef.current += 1;
    setInlineOperation({
      operationId: nextInlineOperationIdRef.current,
      kind: 'rename-artifact',
      artifact,
      value: artifact.title,
      initialValue: artifact.title,
      submitting: false,
      error: null,
    });
  };
  const saveInlineOperation = async () => {
    if (!inlineOperation || inlineOperation.submitting) return;
    const value = inlineOperation.value.trim();
    if (!value) {
      setInlineOperation((current) => (current ? { ...current, error: '请输入名称。' } : current));
      return;
    }
    if (
      (inlineOperation.kind === 'rename-folder' && value === inlineOperation.folder.name) ||
      (inlineOperation.kind === 'rename-artifact' && value === inlineOperation.artifact.title)
    ) {
      setInlineOperation(null);
      return;
    }
    const activeOperation = inlineOperation;
    setInlineOperation({ ...activeOperation, submitting: true, error: null });
    try {
      if (activeOperation.kind === 'create-folder') await onCreateFolder(value);
      else if (activeOperation.kind === 'rename-folder') await onRenameFolder(activeOperation.folder.id, value);
      else await onRename(activeOperation.artifact.id, value);
      setInlineOperation((current) => (current?.operationId === activeOperation.operationId ? null : current));
    } catch (operationError) {
      setInlineOperation((current) =>
        current?.operationId === activeOperation.operationId
          ? {
              ...current,
              submitting: false,
              error: operationError instanceof Error ? operationError.message : '操作失败，请重试。',
            }
          : current
      );
    }
  };
  const confirmPermanentDelete = () => {
    if (!deleteTarget) return;
    if (deleteTarget.kind === 'folder') onDeleteFolder(deleteTarget.folder);
    else onDelete(deleteTarget.artifact);
    setDeleteTarget(null);
  };
  const updateInlineValue = (value: string) => {
    setInlineOperation((current) => (current ? { ...current, value, error: null } : current));
  };
  const cancelInlineOperation = () => {
    setInlineOperation((current) => (current?.submitting ? current : null));
  };

  return (
    <section className='work-home'>
      {view === 'home' ? (
        <>
          {!sidebarOpen && (
            <div className='work-home-sidebar-trigger'>
              <ConversationSidebarOpenButton onOpen={onOpenSidebar} />
            </div>
          )}
          <WorkHomeHero
            taskActions={taskActions}
            activeSessionTitle={activeSessionTitle}
            onContinueSession={onContinueSession}
            onNewTask={onNewTask}
            onTaskSubmit={onTaskSubmit}
            onCreate={onCreate}
            onImport={onImport}
            onOpenWorkspace={onOpenWorkspace}
          />
          <header className='work-home-library-header'>
            <div>
              <h2>继续工作</h2>
              <p>从最近任务和本地文件接着处理。</p>
            </div>
            <div className='work-home-header-actions'>
              <SearchField
                className='work-search'
                label='搜索文件'
                clearLabel='清除文件搜索'
                value={query}
                placeholder='搜索文件'
                onValueChange={setQuery}
              />
              <Button onClick={startCreateFolder}>
                <FolderPlus size={15} />
                新建文件夹
              </Button>
            </div>
          </header>
        </>
      ) : (
        <header className='work-home-header'>
          <div className='work-home-title'>
            {!sidebarOpen && <ConversationSidebarOpenButton onOpen={onOpenSidebar} />}
            <h1>{heading}</h1>
          </div>
          <div className='work-home-header-actions'>
            <SearchField
              className='work-search'
              label='搜索文件'
              clearLabel='清除文件搜索'
              value={query}
              placeholder='搜索文件'
              onValueChange={setQuery}
            />
            <Button onClick={onImport}>
              <Upload size={15} />
              打开文件
            </Button>
            {view === 'folder' && (
              <Button onClick={startCreateFolder}>
                <FolderPlus size={15} />
                新建文件夹
              </Button>
            )}
          </div>
        </header>
      )}

      {onChangeView && (
        <nav className='work-home-view-nav' aria-label='工作资料视图'>
          {[
            ['home', '我的文件'],
            ['recent', '最近'],
            ['favorites', '收藏'],
            ['trash', '回收站'],
          ].map(([id, label]) => (
            <button
              type='button'
              key={id}
              className={view === id || (view === 'folder' && id === 'home') ? 'active' : ''}
              aria-current={view === id || (view === 'folder' && id === 'home') ? 'page' : undefined}
              onClick={() => onChangeView(id as WorkLibraryView)}
            >
              {label}
            </button>
          ))}
        </nav>
      )}

      {(visibleFolders.length > 0 || inlineOperation?.kind === 'create-folder') && (
        <section className='work-folder-section' aria-labelledby='work-folders-title'>
          <div className='work-section-heading'>
            <div>
              <h2 id='work-folders-title'>{view === 'trash' ? '已删除文件夹' : '文件夹'}</h2>
              <span>{visibleFolders.length} 个文件夹</span>
            </div>
          </div>
          <div className='work-folder-grid'>
            {inlineOperation?.kind === 'create-folder' && (
              <FolderInlineEditorCard
                key={`create-folder:${inlineOperation.operationId}`}
                operation={inlineOperation}
                onValueChange={updateInlineValue}
                onSave={saveInlineOperation}
                onCancel={cancelInlineOperation}
              />
            )}
            {visibleFolders.map((folder) =>
              inlineOperation?.kind === 'rename-folder' && inlineOperation.folder.id === folder.id ? (
                <FolderInlineEditorCard
                  key={folder.id}
                  operation={inlineOperation}
                  onValueChange={updateInlineValue}
                  onSave={saveInlineOperation}
                  onCancel={cancelInlineOperation}
                />
              ) : (
                <FolderCard
                  key={folder.id}
                  folder={folder}
                  onOpen={() => onOpenFolder(folder.id)}
                  onRename={() => startRenameFolder(folder)}
                  onRestore={() => onRestoreFolder(folder.id)}
                  onDelete={() => {
                    if (folder.trashedAt) setDeleteTarget({ kind: 'folder', folder });
                    else onDeleteFolder(folder);
                  }}
                />
              )
            )}
          </div>
        </section>
      )}

      <section className='work-files-section' aria-labelledby='work-files-title'>
        <div className='work-section-heading'>
          <div>
            <h2 id='work-files-title'>{view === 'home' ? '最近文件' : heading}</h2>
            <span>{visibleArtifacts.length} 个文件</span>
          </div>
          <fieldset className='work-layout-toggle'>
            <legend className='sr-only'>文件布局</legend>
            <IconButton
              label='网格视图'
              selected={layout === 'grid'}
              className={layout === 'grid' ? 'active' : ''}
              onClick={() => setLayout('grid')}
            >
              <Grid2X2 size={14} />
            </IconButton>
            <IconButton
              label='列表视图'
              selected={layout === 'list'}
              className={layout === 'list' ? 'active' : ''}
              onClick={() => setLayout('list')}
            >
              <List size={15} />
            </IconButton>
          </fieldset>
        </div>

        {loading ? (
          <StateView
            className='work-home-state'
            size='compact'
            role='status'
            icon={<LoaderCircle className='spin' size={18} />}
            title='正在读取文件…'
          />
        ) : error ? (
          <StateView
            className='work-home-state'
            size='compact'
            tone='danger'
            role='alert'
            icon={<FileText size={22} />}
            title='无法读取 Work 文件'
            description={error}
            actions={<Button onClick={onRetry}>重试</Button>}
          />
        ) : visibleArtifacts.length ? (
          <div className={`work-artifact-${layout}`}>
            {visibleArtifacts.map((artifact) =>
              inlineOperation?.kind === 'rename-artifact' && inlineOperation.artifact.id === artifact.id ? (
                <ArtifactInlineEditorCard
                  key={artifact.id}
                  artifact={artifact}
                  layout={layout}
                  operation={inlineOperation}
                  onValueChange={updateInlineValue}
                  onSave={saveInlineOperation}
                  onCancel={cancelInlineOperation}
                />
              ) : (
                <ArtifactCard
                  key={artifact.id}
                  artifact={artifact}
                  layout={layout}
                  onOpen={() => onOpen(artifact.id)}
                  onFavorite={() => onToggleFavorite(artifact.id)}
                  onRename={() => startRenameArtifact(artifact)}
                  onCopy={() => onCopy(artifact.id)}
                  onMove={(folderId) => onMove(artifact.id, folderId)}
                  onRestore={() => onRestore(artifact.id)}
                  onDelete={() => {
                    if (artifact.trashedAt) setDeleteTarget({ kind: 'artifact', artifact });
                    else onDelete(artifact);
                  }}
                  folders={folders.filter((folder) => !folder.trashedAt)}
                />
              )
            )}
          </div>
        ) : (
          <StateView
            className='work-home-state'
            size='compact'
            icon={<FileText size={24} />}
            title={query ? '没有匹配的文件' : view === 'favorites' ? '还没有收藏文件' : '还没有文件'}
            description={query ? '换一个关键词试试' : '创建或打开一个 Office 文件开始工作'}
            actions={!query && <Button onClick={() => onCreate('blank-document')}>新建文字</Button>}
          />
        )}
      </section>
      {deleteTarget && (
        <WorkLibraryDeleteDialog
          target={deleteTarget}
          onClose={() => setDeleteTarget(null)}
          onConfirm={confirmPermanentDelete}
        />
      )}
    </section>
  );
}
