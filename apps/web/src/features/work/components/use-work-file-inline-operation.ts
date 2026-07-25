import { useCallback, useEffect, useRef, useState } from 'react';
import type { WorkspaceEntry } from '../../../types/api';
import type { WorkFilesActions } from '../use-work-files-controller';
import { workLocalArtifactDefaultFileName, workLocalArtifactFileName } from '../work-local-artifact-create';
import { workDuplicateName } from '../work-local-files';
import { WORK_TEMPLATES } from '../work-templates';
import type { WorkArtifactKind } from '../work-types';

export interface WorkFileCreateArtifactRequest {
  requestId: number;
  templateId: string;
  directory: string;
}

export type WorkFileCreateArtifactResult = 'created' | 'exists' | 'error';

export type WorkFileInlineOperation =
  | {
      operationId: number;
      kind: 'create-folder';
      value: string;
      initialValue: string;
      selectionEnd: number;
      submitting: boolean;
      error: string | null;
    }
  | {
      operationId: number;
      kind: 'create-artifact';
      request: WorkFileCreateArtifactRequest;
      artifactKind: WorkArtifactKind;
      value: string;
      initialValue: string;
      selectionEnd: number;
      submitting: boolean;
      error: string | null;
    }
  | {
      operationId: number;
      kind: 'rename' | 'duplicate';
      entry: WorkspaceEntry;
      value: string;
      initialValue: string;
      selectionEnd: number;
      submitting: boolean;
      error: string | null;
    };

export function useWorkFileInlineOperation(
  actions: WorkFilesActions,
  {
    createArtifactRequest,
    onCreateArtifact,
    onConsumeCreateArtifactRequest,
  }: {
    createArtifactRequest?: WorkFileCreateArtifactRequest | null;
    onCreateArtifact?: (
      request: WorkFileCreateArtifactRequest,
      fileName: string
    ) => Promise<WorkFileCreateArtifactResult>;
    onConsumeCreateArtifactRequest?: () => void;
  }
) {
  const [operation, setOperation] = useState<WorkFileInlineOperation | null>(null);
  const nextOperationIdRef = useRef(0);
  const consumedCreateArtifactRequestRef = useRef<number | null>(null);

  useEffect(() => {
    setOperation(null);
  }, [actions.currentPath]);

  useEffect(() => {
    if (!createArtifactRequest || consumedCreateArtifactRequestRef.current === createArtifactRequest.requestId) {
      return;
    }
    consumedCreateArtifactRequestRef.current = createArtifactRequest.requestId;
    const artifactKind =
      WORK_TEMPLATES.find((template) => template.id === createArtifactRequest.templateId)?.kind ?? 'document';
    const value = workLocalArtifactDefaultFileName(artifactKind);
    actions.clearSelection();
    nextOperationIdRef.current += 1;
    setOperation({
      operationId: nextOperationIdRef.current,
      kind: 'create-artifact',
      request: createArtifactRequest,
      artifactKind,
      value,
      initialValue: value,
      selectionEnd: baseNameSelectionEnd(value, false),
      submitting: false,
      error: null,
    });
    onConsumeCreateArtifactRequest?.();
  }, [actions.clearSelection, createArtifactRequest, onConsumeCreateArtifactRequest]);

  const startCreateFolder = useCallback(() => {
    const value = '新建文件夹';
    actions.clearSelection();
    nextOperationIdRef.current += 1;
    setOperation({
      operationId: nextOperationIdRef.current,
      kind: 'create-folder',
      value,
      initialValue: value,
      selectionEnd: value.length,
      submitting: false,
      error: null,
    });
  }, [actions.clearSelection]);

  const startRename = useCallback((entry: WorkspaceEntry) => {
    nextOperationIdRef.current += 1;
    setOperation({
      operationId: nextOperationIdRef.current,
      kind: 'rename',
      entry,
      value: entry.name,
      initialValue: entry.name,
      selectionEnd: baseNameSelectionEnd(entry.name, entry.isDirectory),
      submitting: false,
      error: null,
    });
  }, []);

  const startDuplicate = useCallback(
    (entry: WorkspaceEntry) => {
      const value = workDuplicateName(entry.name, entry.isDirectory);
      actions.clearSelection();
      nextOperationIdRef.current += 1;
      setOperation({
        operationId: nextOperationIdRef.current,
        kind: 'duplicate',
        entry,
        value,
        initialValue: value,
        selectionEnd: baseNameSelectionEnd(value, entry.isDirectory),
        submitting: false,
        error: null,
      });
    },
    [actions.clearSelection]
  );

  const setValue = useCallback((value: string) => {
    setOperation((current) => (current ? { ...current, value, error: null } : current));
  }, []);

  const cancel = useCallback(() => {
    setOperation((current) => (current?.submitting ? current : null));
  }, []);

  const save = useCallback(async () => {
    if (!operation || operation.submitting) return;
    const value = operation.value.trim();
    if (!value) {
      setOperation((current) => (current ? { ...current, error: '请输入名称。' } : current));
      return;
    }
    if (operation.kind === 'rename' && value === operation.entry.name) {
      setOperation(null);
      return;
    }
    if (operation.kind === 'duplicate' && value === operation.entry.name) {
      setOperation((current) =>
        current?.kind === 'duplicate' ? { ...current, error: '副本名称不能与原项目相同。' } : current
      );
      return;
    }
    let artifactFileName: string | null = null;
    if (operation.kind === 'create-artifact') {
      try {
        artifactFileName = workLocalArtifactFileName(value, operation.artifactKind);
      } catch (validationError) {
        setOperation((current) =>
          current?.kind === 'create-artifact'
            ? {
                ...current,
                error: validationError instanceof Error ? validationError.message : '请输入有效的文件名。',
              }
            : current
        );
        return;
      }
    }
    const activeOperation = operation;
    setOperation({ ...activeOperation, submitting: true, error: null });
    try {
      if (activeOperation.kind === 'create-folder') await actions.createFolder(value);
      else if (activeOperation.kind === 'create-artifact') {
        if (!onCreateArtifact || !artifactFileName) throw new Error('当前无法创建 Office 文件，请重试。');
        const result = await onCreateArtifact(activeOperation.request, artifactFileName);
        if (result === 'exists') throw new Error('当前文件夹中已有同名文件，请使用其他名称。');
        if (result === 'error') throw new Error('文件未创建，请检查当前文件夹后重试。');
      } else if (activeOperation.kind === 'rename') await actions.renameEntry(activeOperation.entry, value);
      else await actions.duplicateEntry(activeOperation.entry, value);
      setOperation((current) => (sameInlineOperation(current, activeOperation) ? null : current));
    } catch (operationError) {
      setOperation((current) =>
        sameInlineOperation(current, activeOperation)
          ? {
              ...current,
              submitting: false,
              error: operationError instanceof Error ? operationError.message : '操作失败，请重试。',
            }
          : current
      );
    }
  }, [actions, onCreateArtifact, operation]);

  return {
    operation,
    startCreateFolder,
    startRename,
    startDuplicate,
    setValue,
    save,
    cancel,
  };
}

function baseNameSelectionEnd(name: string, directory: boolean): number {
  if (directory) return name.length;
  const extensionIndex = name.lastIndexOf('.');
  return extensionIndex > 0 ? extensionIndex : name.length;
}

function sameInlineOperation(
  current: WorkFileInlineOperation | null,
  active: WorkFileInlineOperation
): current is WorkFileInlineOperation {
  return current?.operationId === active.operationId;
}
