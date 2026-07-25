import {
  formatWorkFileDate,
  formatWorkFileSize,
  localPathBasename,
  localPathParent,
  relativeLocalPath,
  workFileKindLabel,
} from '../work-local-files';
import { type WorkArtifactKind, workArtifactKindLabel } from '../work-types';
import type { WorkFileInlineOperation } from './use-work-file-inline-operation';
import { WorkFileIcon } from './work-file-icon';
import { WorkFileSelectionControl } from './work-file-selection-controls';
import { WorkInlineNameEditor } from './work-inline-name-editor';

export function WorkFileInlineEditor({
  operation,
  layout,
  index,
  workspaceSearching,
  rootPath,
  onValueChange,
  onSave,
  onCancel,
}: {
  operation: WorkFileInlineOperation;
  layout: 'grid' | 'list';
  index?: number;
  workspaceSearching: boolean;
  rootPath: string;
  onValueChange: (value: string) => void;
  onSave: () => void | Promise<void>;
  onCancel: () => void;
}) {
  const creatingFolder = operation.kind === 'create-folder';
  const creatingArtifact = operation.kind === 'create-artifact';
  const entry = creatingFolder || creatingArtifact ? null : operation.entry;
  const directory = creatingFolder || Boolean(entry?.isDirectory);
  const label =
    operation.kind === 'create-folder'
      ? '新建文件夹名称'
      : operation.kind === 'create-artifact'
        ? `${createArtifactLabel(operation.artifactKind)}名称`
        : operation.kind === 'duplicate'
          ? `副本名称，来源 ${entry?.name ?? ''}`
          : `重命名 ${entry?.name ?? ''}`;
  const saveLabel =
    operation.kind === 'create-folder'
      ? '创建文件夹'
      : operation.kind === 'create-artifact'
        ? createArtifactLabel(operation.artifactKind)
        : operation.kind === 'duplicate'
          ? '创建副本'
          : '保存名称';
  const cancelLabel =
    operation.kind === 'create-folder'
      ? '取消新建文件夹'
      : operation.kind === 'create-artifact'
        ? `取消${createArtifactLabel(operation.artifactKind)}`
        : operation.kind === 'duplicate'
          ? '取消创建副本'
          : '取消重命名';
  return (
    <div
      className={`work-file-item work-file-inline-editor selected ${operation.error ? 'has-error' : ''}`}
      data-work-file-index={index}
      role='option'
      aria-selected='true'
      aria-label={`${operation.value}，${operationLabel(operation)}`}
      aria-busy={operation.submitting}
      tabIndex={-1}
    >
      <WorkFileSelectionControl selected />
      <span className='work-file-visual'>
        <WorkFileIcon path={entry?.path ?? operation.value} directory={directory} size={layout === 'grid' ? 42 : 18} />
      </span>
      <WorkInlineNameEditor
        className='work-file-inline-name'
        value={operation.value}
        label={label}
        saveLabel={saveLabel}
        cancelLabel={cancelLabel}
        selectionEnd={operation.selectionEnd}
        busy={operation.submitting}
        error={operation.error}
        onChange={onValueChange}
        onSave={onSave}
        onCancel={onCancel}
      />
      <span className='work-file-modified'>
        {creatingFolder || creatingArtifact || operation.kind === 'duplicate'
          ? operation.kind === 'duplicate'
            ? '正在创建副本'
            : creatingArtifact
              ? '正在新建 Office 文件'
              : '正在新建'
          : workspaceSearching
            ? relativeLocalPath(localPathParent(operation.entry.path), rootPath) || localPathBasename(rootPath)
            : formatWorkFileDate(operation.entry.mtimeMs)}
      </span>
      <span className='work-file-size'>{entry ? formatWorkFileSize(entry.size, entry.isDirectory) : '—'}</span>
      <span className='work-file-kind'>
        {entry ? workFileKindLabel(entry) : creatingArtifact ? workArtifactKindLabel(operation.artifactKind) : '文件夹'}
      </span>
    </div>
  );
}

function operationLabel(operation: WorkFileInlineOperation): string {
  if (operation.kind === 'create-folder') return '正在新建文件夹';
  if (operation.kind === 'create-artifact') return `正在${createArtifactLabel(operation.artifactKind)}`;
  if (operation.kind === 'duplicate') return '正在创建副本';
  return '正在重命名';
}

function createArtifactLabel(kind: WorkArtifactKind): string {
  if (kind === 'spreadsheet') return '新建电子表格';
  if (kind === 'presentation') return '新建演示文稿';
  return '新建文字文档';
}
