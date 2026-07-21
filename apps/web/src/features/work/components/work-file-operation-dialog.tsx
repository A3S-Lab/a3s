import { useEffect, useId, useState } from 'react';
import { Button, Dialog } from '../../../design-system/primitives';
import type { WorkspaceEntry } from '../../../types/api';
import type { WorkFilesActions } from '../use-work-files-controller';
import { workDuplicateName } from '../work-local-files';

export type WorkFileOperation =
  | { kind: 'create-folder' }
  | { kind: 'rename'; entry: WorkspaceEntry }
  | { kind: 'duplicate'; entry: WorkspaceEntry };

export function WorkFileOperationDialog({
  operation,
  actions,
  onClose,
}: {
  operation: WorkFileOperation;
  actions: WorkFilesActions;
  onClose: () => void;
}) {
  const formId = useId();
  const [name, setName] = useState(() => initialName(operation));
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setName(initialName(operation));
    setError(null);
  }, [operation]);

  const title = operation.kind === 'create-folder' ? '新建文件夹' : operation.kind === 'rename' ? '重命名' : '创建副本';
  const description =
    operation.kind === 'create-folder'
      ? '文件夹会直接创建在当前本地目录中。'
      : operation.kind === 'rename'
        ? '名称会直接更新到本地文件系统。'
        : '副本会创建在原文件或文件夹旁边。';

  return (
    <Dialog
      title={title}
      description={description}
      closeDisabled={submitting}
      onClose={onClose}
      footer={
        <>
          <Button tone='quiet' disabled={submitting} onClick={onClose}>
            取消
          </Button>
          <Button type='submit' form={formId} tone='primary' loading={submitting} disabled={!name.trim()}>
            {operation.kind === 'duplicate' ? '创建副本' : '保存'}
          </Button>
        </>
      }
    >
      <form
        id={formId}
        className='work-file-operation-form'
        onSubmit={(event) => {
          event.preventDefault();
          setSubmitting(true);
          setError(null);
          const request =
            operation.kind === 'create-folder'
              ? actions.createFolder(name)
              : operation.kind === 'rename'
                ? actions.renameEntry(operation.entry, name)
                : actions.duplicateEntry(operation.entry, name);
          void request
            .then(onClose)
            .catch((operationError) => {
              setError(operationError instanceof Error ? operationError.message : '操作失败，请重试。');
            })
            .finally(() => setSubmitting(false));
        }}
      >
        <label>
          <span>名称</span>
          <input
            data-autofocus
            aria-label='文件或文件夹名称'
            value={name}
            disabled={submitting}
            onChange={(event) => setName(event.target.value)}
            onFocus={(event) => selectBaseName(event.currentTarget, operation)}
          />
        </label>
        {error && (
          <p className='work-file-operation-error' role='alert'>
            {error}
          </p>
        )}
      </form>
    </Dialog>
  );
}

function initialName(operation: WorkFileOperation): string {
  if (operation.kind === 'create-folder') return '新建文件夹';
  if (operation.kind === 'rename') return operation.entry.name;
  return workDuplicateName(operation.entry.name, operation.entry.isDirectory);
}

function selectBaseName(input: HTMLInputElement, operation: WorkFileOperation): void {
  if (operation.kind === 'create-folder') {
    input.select();
    return;
  }
  const value = input.value;
  const extensionIndex = operation.entry.isDirectory ? -1 : value.lastIndexOf('.');
  input.setSelectionRange(0, extensionIndex > 0 ? extensionIndex : value.length);
}
