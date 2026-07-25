import { Button, Dialog, InlineNotice } from '../../../design-system/primitives';
import type { WorkArtifact, WorkFolder } from '../work-types';

export type WorkLibraryDeleteTarget =
  | { kind: 'folder'; folder: WorkFolder }
  | { kind: 'artifact'; artifact: WorkArtifact };

export function WorkLibraryDeleteDialog({
  target,
  onClose,
  onConfirm,
}: {
  target: WorkLibraryDeleteTarget;
  onClose: () => void;
  onConfirm: () => void;
}) {
  return (
    <Dialog
      title={target.kind === 'folder' ? '永久删除文件夹' : '永久删除文件'}
      description='删除后无法恢复。'
      className='work-library-operation-dialog'
      onClose={onClose}
      footer={
        <>
          <Button tone='quiet' onClick={onClose}>
            取消
          </Button>
          <Button tone='danger' onClick={onConfirm}>
            确认永久删除
          </Button>
        </>
      }
    >
      <InlineNotice className='work-library-operation-warning' tone='danger' role='alert' title='永久删除确认'>
        {target.kind === 'folder'
          ? `确定永久删除文件夹“${target.folder.name}”吗？文件夹必须为空。`
          : `确定永久删除“${target.artifact.title}”吗？`}
      </InlineNotice>
    </Dialog>
  );
}
