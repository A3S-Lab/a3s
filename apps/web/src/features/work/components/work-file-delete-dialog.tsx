import { useState } from 'react';
import { Button, Dialog, InlineNotice } from '../../../design-system/primitives';
import type { WorkspaceEntry } from '../../../types/api';
import type { WorkFilesActions } from '../use-work-files-controller';

export function WorkFileDeleteDialog({
  entries,
  actions,
  onClose,
}: {
  entries: WorkspaceEntry[];
  actions: WorkFilesActions;
  onClose: () => void;
}) {
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const runRequest = () => {
    setSubmitting(true);
    setError(null);
    void actions
      .deleteEntries(entries)
      .then(onClose)
      .catch((operationError) => {
        setError(operationError instanceof Error ? operationError.message : '操作失败，请重试。');
      })
      .finally(() => setSubmitting(false));
  };

  return (
    <Dialog
      title={entries.length === 1 ? '永久删除' : `永久删除 ${entries.length} 项`}
      description='删除后无法恢复。'
      closeDisabled={submitting}
      onClose={onClose}
      footer={
        <>
          <Button tone='quiet' disabled={submitting} onClick={onClose}>
            取消
          </Button>
          <Button tone='danger' loading={submitting} onClick={runRequest}>
            确认永久删除
          </Button>
        </>
      }
    >
      <InlineNotice className='work-library-operation-warning' tone='danger' role='alert' title='将从本机永久删除'>
        {entries.length === 1
          ? `“${entries[0].name}”将从本机永久删除。`
          : `选中的 ${entries.length} 项将从本机永久删除。`}
      </InlineNotice>
      {error && (
        <InlineNotice className='work-file-operation-error' tone='danger' role='alert' title='操作失败'>
          {error}
        </InlineNotice>
      )}
    </Dialog>
  );
}
