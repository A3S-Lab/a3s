import {
  Copy,
  FileText,
  FileType2,
  Folder,
  FolderPlus,
  Grid2X2,
  List,
  MoreHorizontal,
  Pencil,
  Presentation,
  RotateCcw,
  Search,
  Sheet,
  Star,
  Trash2,
  Upload,
} from 'lucide-react';
import { type CSSProperties, useMemo, useState } from 'react';
import { WORK_TEMPLATES } from '../work-templates';
import type { WorkArtifact, WorkArtifactKind, WorkFolder, WorkLibraryView } from '../work-types';
import { workArtifactExtension, workArtifactKindLabel } from '../work-types';

interface WorkHomeProps {
  artifacts: WorkArtifact[];
  folders: WorkFolder[];
  view: WorkLibraryView;
  activeFolderId: string | null;
  loading: boolean;
  error: string | null;
  onCreate: (templateId: string) => void;
  onOpen: (id: string) => void;
  onImport: () => void;
  onToggleFavorite: (id: string) => void;
  onRename: (id: string, title: string) => void;
  onCopy: (id: string) => void;
  onMove: (id: string, folderId: string | null) => void;
  onRestore: (id: string) => void;
  onDelete: (artifact: WorkArtifact) => void;
  onOpenFolder: (id: string) => void;
  onCreateFolder: (name: string) => void;
  onRenameFolder: (id: string, name: string) => void;
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
            : '工作副本';

  return (
    <section className='work-home'>
      <header className='work-home-header'>
        <div>
          <p>WORKSPACE</p>
          <h1>{heading}</h1>
          <span>{view === 'home' ? '用于兼容编辑、自动保存与版本恢复' : '继续处理上次的内容'}</span>
        </div>
        <div className='work-home-header-actions'>
          <label className='work-search'>
            <Search size={15} />
            <input
              type='search'
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder='搜索文件'
              aria-label='搜索 Work 文件'
            />
          </label>
          <button type='button' className='work-secondary-button' onClick={onImport}>
            <Upload size={15} />
            打开文件
          </button>
          {(view === 'home' || view === 'folder') && (
            <button
              type='button'
              className='work-secondary-button'
              onClick={() => {
                const name = window.prompt('文件夹名称');
                if (name?.trim()) onCreateFolder(name);
              }}
            >
              <FolderPlus size={15} />
              新建文件夹
            </button>
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

      {visibleFolders.length > 0 && (
        <section className='work-folder-section' aria-labelledby='work-folders-title'>
          <div className='work-section-heading'>
            <div>
              <h2 id='work-folders-title'>{view === 'trash' ? '已删除文件夹' : '文件夹'}</h2>
              <span>{visibleFolders.length} 个文件夹</span>
            </div>
          </div>
          <div className='work-folder-grid'>
            {visibleFolders.map((folder) => (
              <FolderCard
                key={folder.id}
                folder={folder}
                onOpen={() => onOpenFolder(folder.id)}
                onRename={() => {
                  const name = window.prompt('重命名文件夹', folder.name);
                  if (name?.trim() && name.trim() !== folder.name) onRenameFolder(folder.id, name);
                }}
                onRestore={() => onRestoreFolder(folder.id)}
                onDelete={() => onDeleteFolder(folder)}
              />
            ))}
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
            <button
              type='button'
              className={layout === 'grid' ? 'active' : ''}
              aria-label='网格视图'
              onClick={() => setLayout('grid')}
            >
              <Grid2X2 size={14} />
            </button>
            <button
              type='button'
              className={layout === 'list' ? 'active' : ''}
              aria-label='列表视图'
              onClick={() => setLayout('list')}
            >
              <List size={15} />
            </button>
          </fieldset>
        </div>

        {loading ? (
          <output className='work-files-state'>
            <span className='work-state-spinner' />
            正在读取文件…
          </output>
        ) : error ? (
          <div className='work-files-state error'>
            <strong>无法读取 Work 文件</strong>
            <span>{error}</span>
            <button type='button' onClick={onRetry}>
              重试
            </button>
          </div>
        ) : visibleArtifacts.length ? (
          <div className={`work-artifact-${layout}`}>
            {visibleArtifacts.map((artifact) => (
              <ArtifactCard
                key={artifact.id}
                artifact={artifact}
                layout={layout}
                onOpen={() => onOpen(artifact.id)}
                onFavorite={() => onToggleFavorite(artifact.id)}
                onRename={() => {
                  const title = window.prompt('重命名文件', artifact.title);
                  if (title?.trim() && title.trim() !== artifact.title) onRename(artifact.id, title);
                }}
                onCopy={() => onCopy(artifact.id)}
                onMove={(folderId) => onMove(artifact.id, folderId)}
                onRestore={() => onRestore(artifact.id)}
                onDelete={() => onDelete(artifact)}
                folders={folders.filter((folder) => !folder.trashedAt)}
              />
            ))}
          </div>
        ) : (
          <div className='work-files-state empty'>
            <span className='work-empty-icon'>
              <FileText size={24} />
            </span>
            <strong>{query ? '没有匹配的文件' : view === 'favorites' ? '还没有收藏文件' : '还没有文件'}</strong>
            <span>{query ? '换一个关键词试试' : '创建或打开一个 Office 文件开始工作'}</span>
            {!query && (
              <button type='button' onClick={() => onCreate('blank-document')}>
                新建文字
              </button>
            )}
          </div>
        )}
      </section>
    </section>
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
          <button
            type='button'
            className={artifact.favorite ? 'active' : ''}
            onClick={onFavorite}
            aria-label={artifact.favorite ? `取消收藏 ${artifact.title}` : `收藏 ${artifact.title}`}
          >
            <Star size={14} fill={artifact.favorite ? 'currentColor' : 'none'} />
          </button>
        )}
        <details>
          <summary aria-label={`${artifact.title} 更多操作`}>
            <MoreHorizontal size={15} />
          </summary>
          <div>
            <span>
              {workArtifactExtension(artifact.kind).toUpperCase()} · 第 {artifact.revision} 版
            </span>
            {trashed ? (
              <>
                <button type='button' onClick={onRestore}>
                  <RotateCcw size={13} />
                  恢复
                </button>
                <button type='button' className='danger' onClick={onDelete}>
                  <Trash2 size={13} />
                  永久删除
                </button>
              </>
            ) : (
              <>
                <button type='button' onClick={onRename}>
                  <Pencil size={13} />
                  重命名
                </button>
                <button type='button' onClick={onCopy}>
                  <Copy size={13} />
                  创建副本
                </button>
                <label className='work-move-control'>
                  <Folder size={13} />
                  <select
                    aria-label={`移动 ${artifact.title}`}
                    value={artifact.folderId ?? ''}
                    onChange={(event) => onMove(event.target.value || null)}
                  >
                    <option value=''>全部文件</option>
                    {folders.map((folder) => (
                      <option key={folder.id} value={folder.id}>
                        {folder.name}
                      </option>
                    ))}
                  </select>
                </label>
                <button type='button' className='danger' onClick={onDelete}>
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
  return (
    <article className='work-folder-card'>
      <button type='button' onClick={onOpen} disabled={trashed} aria-label={`打开文件夹 ${folder.name}`}>
        <Folder size={24} fill='currentColor' />
        <span>
          <strong>{folder.name}</strong>
          <small>{formatRecentTime(folder.updatedAt)}</small>
        </span>
      </button>
      <details>
        <summary aria-label={`${folder.name} 更多操作`}>
          <MoreHorizontal size={15} />
        </summary>
        <div>
          {trashed ? (
            <>
              <button type='button' onClick={onRestore}>
                <RotateCcw size={13} />
                恢复
              </button>
              <button type='button' className='danger' onClick={onDelete}>
                <Trash2 size={13} />
                永久删除
              </button>
            </>
          ) : (
            <>
              <button type='button' onClick={onRename}>
                <Pencil size={13} />
                重命名
              </button>
              <button type='button' className='danger' onClick={onDelete}>
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
