import {
  Copy,
  FileText,
  FileType2,
  Folder,
  MoreHorizontal,
  Pencil,
  Presentation,
  RotateCcw,
  Sheet,
  Star,
  Trash2,
} from 'lucide-react';
import { useRef } from 'react';
import { IconButton } from '../../../design-system/primitives';
import { OfficeSelect } from '../editors/office-controls';
import type { WorkArtifact, WorkArtifactKind, WorkFolder } from '../work-types';
import { workArtifactExtension, workArtifactKindLabel } from '../work-types';
import { WorkFileIcon } from './work-file-icon';
import { WorkInlineNameEditor } from './work-inline-name-editor';

export type WorkLibraryInlineOperation =
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

export function FolderInlineEditorCard({
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

export function ArtifactInlineEditorCard({
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

export function ArtifactCard({
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

export function FolderCard({
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

export function TemplatePreview({ kind, detailed }: { kind: WorkArtifactKind; detailed: boolean }) {
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
