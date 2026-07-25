import {
  Copy,
  FileText,
  FileType2,
  Folder,
  FolderPlus,
  Grid2X2,
  List,
  LoaderCircle,
  MoreHorizontal,
  Pencil,
  Presentation,
  RotateCcw,
  Sheet,
  Star,
  Trash2,
  Upload,
} from 'lucide-react';
import { type CSSProperties, useEffect, useMemo, useRef, useState } from 'react';
import { Button, IconButton, SearchField, StateView } from '../../../design-system/primitives';
import { OfficeSelect } from '../editors/office-controls';
import { WORK_TEMPLATES } from '../work-templates';
import type { WorkArtifact, WorkArtifactKind, WorkFolder, WorkLibraryView } from '../work-types';
import { workArtifactExtension, workArtifactKindLabel } from '../work-types';
import { WorkFileIcon } from './work-file-icon';
import { WorkInlineNameEditor } from './work-inline-name-editor';
import { WorkLibraryDeleteDialog, type WorkLibraryDeleteTarget } from './work-library-delete-dialog';
import { WorkSidebarOpenButton } from './work-sidebar-open-button';

interface WorkHomeProps {
  artifacts: WorkArtifact[];
  folders: WorkFolder[];
  view: WorkLibraryView;
  activeFolderId: string | null;
  loading: boolean;
  error: string | null;
  sidebarOpen: boolean;
  onOpenSidebar: () => void;
  onCreate: (templateId: string) => void;
  onOpen: (id: string) => void;
  onImport: () => void;
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

type WorkLibraryInlineOperation =
  | {
      operationId: number;
      kind: 'create-folder';
      value: string;
      initialValue: string;
      submitting: boolean;
      error: string | null;
    }
  | {
      operationId: number;
      kind: 'rename-folder';
      folder: WorkFolder;
      value: string;
      initialValue: string;
      submitting: boolean;
      error: string | null;
    }
  | {
      operationId: number;
      kind: 'rename-artifact';
      artifact: WorkArtifact;
      value: string;
      initialValue: string;
      submitting: boolean;
      error: string | null;
    };

export function WorkHome({
  artifacts,
  folders,
  view,
  activeFolderId,
  loading,
  error,
  sidebarOpen,
  onOpenSidebar,
  onCreate,
  onOpen,
  onImport,
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
      <header className='work-home-header'>
        <div className='work-home-title'>
          {!sidebarOpen && <WorkSidebarOpenButton onOpen={onOpenSidebar} />}
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
          {(view === 'home' || view === 'folder') && (
            <Button onClick={startCreateFolder}>
              <FolderPlus size={15} />
              新建文件夹
            </Button>
          )}
        </div>
      </header>

      {view === 'home' && (
        <section className='work-template-section' aria-labelledby='work-template-title'>
          <div className='work-section-heading'>
            <div>
              <h2 id='work-template-title'>新建</h2>
              <span>从空白文件或实用模板开始</span>
            </div>
          </div>
          <div className='work-template-grid'>
            {WORK_TEMPLATES.map((template) => (
              <button
                type='button'
                className={`work-template-card ${template.kind}`}
                key={template.id}
                onClick={() => onCreate(template.id)}
              >
                <span
                  className='work-template-preview'
                  style={{ '--work-template-accent': template.accent } as CSSProperties}
                >
                  <TemplatePreview kind={template.kind} detailed={!template.id.startsWith('blank-')} />
                </span>
                <span className='work-template-copy'>
                  <strong>{template.name}</strong>
                  <small>{template.description}</small>
                </span>
              </button>
            ))}
          </div>
        </section>
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

function FolderInlineEditorCard({
  operation,
  onValueChange,
  onSave,
  onCancel,
}: {
  operation: Extract<WorkLibraryInlineOperation, { kind: 'create-folder' | 'rename-folder' }>;
  onValueChange: (value: string) => void;
  onSave: () => void | Promise<void>;
  onCancel: () => void;
}) {
  const creating = operation.kind === 'create-folder';
  return (
    <article className='work-folder-card work-library-inline-card'>
      <div className='work-folder-inline-content'>
        <WorkFileIcon path={creating ? operation.value : operation.folder.name} directory size={32} />
        <WorkInlineNameEditor
          className='work-library-inline-name'
          value={operation.value}
          label={creating ? '新建文件夹名称' : `重命名文件夹 ${operation.folder.name}`}
          saveLabel={creating ? '创建文件夹' : '保存文件夹名称'}
          cancelLabel={creating ? '取消新建文件夹' : '取消重命名文件夹'}
          selectionEnd={operation.initialValue.length}
          busy={operation.submitting}
          error={operation.error}
          onChange={onValueChange}
          onSave={onSave}
          onCancel={onCancel}
        />
      </div>
    </article>
  );
}

function ArtifactInlineEditorCard({
  artifact,
  layout,
  operation,
  onValueChange,
  onSave,
  onCancel,
}: {
  artifact: WorkArtifact;
  layout: 'grid' | 'list';
  operation: Extract<WorkLibraryInlineOperation, { kind: 'rename-artifact' }>;
  onValueChange: (value: string) => void;
  onSave: () => void | Promise<void>;
  onCancel: () => void;
}) {
  return (
    <article className={`work-artifact-card work-library-inline-card ${artifact.kind} ${layout}`}>
      <div className='work-artifact-open'>
        <span className='work-artifact-thumbnail'>
          <ArtifactPreview artifact={artifact} />
        </span>
        <span className='work-artifact-details'>
          <WorkInlineNameEditor
            className='work-library-inline-name'
            value={operation.value}
            label={`重命名文件 ${artifact.title}`}
            saveLabel='保存文件名称'
            cancelLabel='取消重命名文件'
            selectionEnd={operation.initialValue.length}
            busy={operation.submitting}
            error={operation.error}
            onChange={onValueChange}
            onSave={onSave}
            onCancel={onCancel}
          />
          <small>
            {workArtifactKindLabel(artifact.kind)} · {formatRecentTime(artifact.updatedAt)}
          </small>
        </span>
      </div>
    </article>
  );
}

function ArtifactCard({
  artifact,
  layout,
  onOpen,
  onFavorite,
  onRename,
  onCopy,
  onMove,
  onRestore,
  onDelete,
  folders,
}: {
  artifact: WorkArtifact;
  layout: 'grid' | 'list';
  onOpen: () => void;
  onFavorite: () => void;
  onRename: () => void;
  onCopy: () => void;
  onMove: (folderId: string | null) => void;
  onRestore: () => void;
  onDelete: () => void;
  folders: WorkFolder[];
}) {
  const trashed = Boolean(artifact.trashedAt);
  const menuRef = useRef<HTMLDetailsElement>(null);
  const closeMenu = () => {
    if (menuRef.current) menuRef.current.open = false;
  };
  return (
    <article className={`work-artifact-card ${artifact.kind} ${layout}`}>
      <button type='button' className='work-artifact-open' onClick={onOpen} aria-label={`打开 ${artifact.title}`}>
        <span className='work-artifact-thumbnail'>
          <ArtifactPreview artifact={artifact} />
        </span>
        <span className='work-artifact-details'>
          <strong>{artifact.title}</strong>
          <small>
            {workArtifactKindLabel(artifact.kind)} · {formatRecentTime(artifact.updatedAt)}
          </small>
        </span>
      </button>
      <div className='work-artifact-actions'>
        {!trashed && (
          <IconButton
            label={artifact.favorite ? `取消收藏 ${artifact.title}` : `收藏 ${artifact.title}`}
            selected={artifact.favorite}
            className={artifact.favorite ? 'active' : ''}
            onClick={onFavorite}
          >
            <Star size={14} fill={artifact.favorite ? 'currentColor' : 'none'} />
          </IconButton>
        )}
        <details ref={menuRef}>
          <summary aria-label={`${artifact.title} 更多操作`}>
            <MoreHorizontal size={15} />
          </summary>
          <div>
            <span>
              {workArtifactExtension(artifact.kind).toUpperCase()} · 第 {artifact.revision} 版
            </span>
            {trashed ? (
              <>
                <button
                  type='button'
                  onClick={() => {
                    closeMenu();
                    onRestore();
                  }}
                >
                  <RotateCcw size={13} />
                  恢复
                </button>
                <button
                  type='button'
                  className='danger'
                  onClick={() => {
                    closeMenu();
                    onDelete();
                  }}
                >
                  <Trash2 size={13} />
                  永久删除
                </button>
              </>
            ) : (
              <>
                <button
                  type='button'
                  onClick={() => {
                    closeMenu();
                    onRename();
                  }}
                >
                  <Pencil size={13} />
                  重命名
                </button>
                <button
                  type='button'
                  onClick={() => {
                    closeMenu();
                    onCopy();
                  }}
                >
                  <Copy size={13} />
                  创建副本
                </button>
                <div className='work-move-control'>
                  <Folder size={13} />
                  <OfficeSelect
                    ariaLabel={`移动 ${artifact.title}`}
                    value={artifact.folderId ?? ''}
                    options={[
                      { value: '', label: '全部文件' },
                      ...folders.map((folder) => ({ value: folder.id, label: folder.name })),
                    ]}
                    onValueChange={(folderId) => {
                      closeMenu();
                      onMove(folderId || null);
                    }}
                  />
                </div>
                <button
                  type='button'
                  className='danger'
                  onClick={() => {
                    closeMenu();
                    onDelete();
                  }}
                >
                  <Trash2 size={13} />
                  移到回收站
                </button>
              </>
            )}
          </div>
        </details>
      </div>
    </article>
  );
}

function FolderCard({
  folder,
  onOpen,
  onRename,
  onRestore,
  onDelete,
}: {
  folder: WorkFolder;
  onOpen: () => void;
  onRename: () => void;
  onRestore: () => void;
  onDelete: () => void;
}) {
  const trashed = Boolean(folder.trashedAt);
  const menuRef = useRef<HTMLDetailsElement>(null);
  const closeMenu = () => {
    if (menuRef.current) menuRef.current.open = false;
  };
  return (
    <article className='work-folder-card'>
      <button type='button' onClick={onOpen} disabled={trashed} aria-label={`打开文件夹 ${folder.name}`}>
        <WorkFileIcon path={folder.name} directory size={32} />
        <span>
          <strong>{folder.name}</strong>
          <small>{formatRecentTime(folder.updatedAt)}</small>
        </span>
      </button>
      <details ref={menuRef}>
        <summary aria-label={`${folder.name} 更多操作`}>
          <MoreHorizontal size={15} />
        </summary>
        <div>
          {trashed ? (
            <>
              <button
                type='button'
                onClick={() => {
                  closeMenu();
                  onRestore();
                }}
              >
                <RotateCcw size={13} />
                恢复
              </button>
              <button
                type='button'
                className='danger'
                onClick={() => {
                  closeMenu();
                  onDelete();
                }}
              >
                <Trash2 size={13} />
                永久删除
              </button>
            </>
          ) : (
            <>
              <button
                type='button'
                onClick={() => {
                  closeMenu();
                  onRename();
                }}
              >
                <Pencil size={13} />
                重命名
              </button>
              <button
                type='button'
                className='danger'
                onClick={() => {
                  closeMenu();
                  onDelete();
                }}
              >
                <Trash2 size={13} />
                移到回收站
              </button>
            </>
          )}
        </div>
      </details>
    </article>
  );
}

function TemplatePreview({ kind, detailed }: { kind: WorkArtifactKind; detailed: boolean }) {
  if (kind === 'spreadsheet') {
    return (
      <>
        <Sheet size={19} />
        <i className='sheet-grid'>
          {Array.from({ length: 12 }, (_, index) => (
            <b key={index} className={detailed && [0, 4, 8].includes(index) ? 'filled' : ''} />
          ))}
        </i>
      </>
    );
  }
  if (kind === 'presentation') {
    return (
      <>
        <Presentation size={19} />
        <i className='slide-block'>
          <b />
          <b />
          {detailed && <b />}
        </i>
      </>
    );
  }
  return (
    <>
      <FileText size={19} />
      <i className='document-lines'>
        <b />
        <b />
        <b />
        {detailed && <b />}
      </i>
    </>
  );
}

function ArtifactPreview({ artifact }: { artifact: WorkArtifact }) {
  if (artifact.kind === 'pdf') {
    return (
      <span className='artifact-pdf-preview'>
        <FileType2 size={29} />
        <strong>PDF</strong>
      </span>
    );
  }
  if (artifact.kind === 'spreadsheet') {
    return <TemplatePreview kind='spreadsheet' detailed />;
  }
  if (artifact.kind === 'presentation') {
    const slide = artifact.content.type === 'presentation' ? artifact.content.slides[0] : null;
    return (
      <span className='artifact-slide-preview' style={{ background: slide?.background ?? '#ffffff' }}>
        {(slide?.elements ?? []).slice(0, 4).map((element) => (
          <i
            key={element.id}
            style={{
              left: `${element.x}%`,
              top: `${element.y}%`,
              width: `${element.width}%`,
              height: `${element.height}%`,
              background:
                element.type === 'image' && element.image
                  ? `center / cover url("${element.image.dataUrl}")`
                  : element.fill === 'transparent'
                    ? element.color
                    : element.fill,
            }}
          />
        ))}
      </span>
    );
  }
  return <TemplatePreview kind='document' detailed />;
}

function formatRecentTime(timestamp: number): string {
  const elapsed = Date.now() - timestamp;
  if (elapsed < 60_000) return '刚刚';
  if (elapsed < 3_600_000) return `${Math.max(1, Math.floor(elapsed / 60_000))} 分钟前`;
  if (elapsed < 86_400_000) return `${Math.floor(elapsed / 3_600_000)} 小时前`;
  return new Intl.DateTimeFormat('zh-CN', { month: 'short', day: 'numeric' }).format(timestamp);
}
