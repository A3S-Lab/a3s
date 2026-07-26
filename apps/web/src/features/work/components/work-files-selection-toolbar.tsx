import { BookPlus, CheckCircle2, Copy, Eye, ListChecks, Pencil, Scissors, Sparkles, Trash2, X } from 'lucide-react';
import { Button, IconButton } from '../../../design-system/primitives';
import type { WorkspaceEntry } from '../../../types/api';

export function WorkFilesSelectionToolbar({
  selectedEntries,
  totalCount,
  onSelectAll,
  onQuickLook,
  onRename,
  onCopy,
  onCut,
  onCreateKnowledgeBase,
  onAskAssistant,
  onDelete,
  onClear,
}: {
  selectedEntries: readonly WorkspaceEntry[];
  totalCount: number;
  onSelectAll: () => void;
  onQuickLook: (entry: WorkspaceEntry) => void;
  onRename: (entry: WorkspaceEntry) => void;
  onCopy: (entries: readonly WorkspaceEntry[]) => void;
  onCut: (entries: readonly WorkspaceEntry[]) => void;
  onCreateKnowledgeBase: (entries: readonly WorkspaceEntry[]) => void;
  onAskAssistant: (entries: readonly WorkspaceEntry[]) => void;
  onDelete: (entries: readonly WorkspaceEntry[]) => void;
  onClear: () => void;
}) {
  const selectedCount = selectedEntries.length;
  if (!selectedCount) return null;
  const onlyEntry = selectedCount === 1 ? selectedEntries[0] : null;

  return (
    <section className='work-files-selection-toolbar' role='toolbar' aria-label='已选文件操作'>
      <span className='work-files-selection-summary' aria-live='polite'>
        <CheckCircle2 size={16} aria-hidden='true' />
        已选择 <strong>{selectedCount}</strong> 项
      </span>
      <div className='work-files-selection-actions'>
        {selectedCount < totalCount && (
          <Button
            size='compact'
            tone='quiet'
            aria-label={`选择全部 ${totalCount} 项`}
            title={`选择全部 ${totalCount} 项`}
            onClick={onSelectAll}
          >
            <ListChecks size={14} aria-hidden='true' />
            <span>全选</span>
          </Button>
        )}
        {onlyEntry && (
          <>
            <Button
              size='compact'
              tone='quiet'
              aria-label='快速查看当前选择'
              title='快速查看当前选择'
              onClick={() => onQuickLook(onlyEntry)}
            >
              <Eye size={14} aria-hidden='true' />
              <span>快速查看</span>
            </Button>
            <Button
              size='compact'
              tone='quiet'
              aria-label='重命名所选项目'
              title='重命名所选项目'
              onClick={() => onRename(onlyEntry)}
            >
              <Pencil size={14} aria-hidden='true' />
              <span>重命名</span>
            </Button>
          </>
        )}
        <Button
          size='compact'
          tone='quiet'
          aria-label={`复制所选 ${selectedCount} 项`}
          title='复制'
          onClick={() => onCopy(selectedEntries)}
        >
          <Copy size={14} aria-hidden='true' />
          <span>复制</span>
        </Button>
        <Button
          size='compact'
          tone='quiet'
          aria-label={`剪切所选 ${selectedCount} 项`}
          title='剪切'
          onClick={() => onCut(selectedEntries)}
        >
          <Scissors size={14} aria-hidden='true' />
          <span>剪切</span>
        </Button>
        <Button
          size='compact'
          tone='quiet'
          aria-label='从所选项目创建知识库'
          title='创建知识库'
          onClick={() => onCreateKnowledgeBase(selectedEntries)}
        >
          <BookPlus size={14} aria-hidden='true' />
          <span>创建知识库</span>
        </Button>
        <Button
          size='compact'
          tone='quiet'
          aria-label='询问 AI 助手'
          title='询问 AI 助手'
          onClick={() => onAskAssistant(selectedEntries)}
        >
          <Sparkles size={14} aria-hidden='true' />
          <span>询问 AI</span>
        </Button>
        <Button
          size='compact'
          tone='danger'
          aria-label={`永久删除所选 ${selectedCount} 项`}
          title={`永久删除所选 ${selectedCount} 项`}
          onClick={() => onDelete(selectedEntries)}
        >
          <Trash2 size={14} aria-hidden='true' />
          <span>删除</span>
        </Button>
        <IconButton className='work-files-selection-clear' label='取消选择' onClick={onClear}>
          <X size={15} />
        </IconButton>
      </div>
    </section>
  );
}
