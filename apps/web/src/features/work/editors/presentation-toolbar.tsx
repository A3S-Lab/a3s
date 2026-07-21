import {
  AlignCenter,
  AlignLeft,
  AlignRight,
  ArrowDownToLine,
  ArrowUpToLine,
  BarChart3,
  Bold,
  ClipboardPaste,
  Copy,
  Image,
  LayoutTemplate,
  Link2,
  MessageSquarePlus,
  MessagesSquare,
  Plus,
  Scissors,
  Square,
  Table2,
  Trash2,
  Type,
} from 'lucide-react';
import type { WorkSlide, WorkSlideElement, WorkSlideTextAlign } from '../work-types';

export function PresentationToolbar({
  selectedSlide,
  selectedElement,
  slideCount,
  onAddSlide,
  onDuplicateSlide,
  onDeleteSlide,
  onCopySelection,
  onCutSelection,
  onPasteSelection,
  onAddElement,
  onRequestImage,
  onAddTable,
  onAddChart,
  onAddComment,
  commentsOpen,
  commentCount,
  onToggleComments,
  onUpdateElement,
  onReorderElement,
  onSetBackground,
  designOpen,
  editingDesign,
  onToggleDesign,
  background,
}: {
  selectedSlide: WorkSlide;
  selectedElement: WorkSlideElement | null;
  slideCount: number;
  onAddSlide: () => void;
  onDuplicateSlide: () => void;
  onDeleteSlide: () => void;
  onCopySelection: () => void;
  onCutSelection: () => void;
  onPasteSelection: () => void;
  onAddElement: (type: 'text' | 'shape') => void;
  onRequestImage: () => void;
  onAddTable: () => void;
  onAddChart: () => void;
  onAddComment: () => void;
  commentsOpen: boolean;
  commentCount: number;
  onToggleComments: () => void;
  onUpdateElement: (patch: Partial<WorkSlideElement>) => void;
  onReorderElement: (direction: -1 | 1) => void;
  onSetBackground: (color: string) => void;
  designOpen: boolean;
  editingDesign: boolean;
  onToggleDesign: () => void;
  background?: string;
}) {
  return (
    <div className='work-office-toolbar presentation-toolbar' role='toolbar' aria-label='演示编辑工具栏'>
      <button type='button' onClick={onAddSlide}>
        <Plus size={15} />
        新建幻灯片
      </button>
      <button type='button' onClick={onDuplicateSlide}>
        <Copy size={14} />
        复制幻灯片
      </button>
      <button type='button' onClick={onDeleteSlide} disabled={slideCount === 1}>
        <Trash2 size={14} />
      </button>
      <button type='button' aria-label='复制所选元素或幻灯片' title='复制（⌘/Ctrl+C）' onClick={onCopySelection}>
        <Copy size={14} />
      </button>
      <button type='button' aria-label='剪切所选元素或幻灯片' title='剪切（⌘/Ctrl+X）' onClick={onCutSelection}>
        <Scissors size={14} />
      </button>
      <button type='button' aria-label='粘贴演示内容' title='粘贴（⌘/Ctrl+V）' onClick={onPasteSelection}>
        <ClipboardPaste size={14} />
      </button>
      <button
        type='button'
        className={designOpen ? 'active' : ''}
        aria-label='母版与布局'
        aria-pressed={designOpen}
        onClick={onToggleDesign}
      >
        <LayoutTemplate size={14} />
        母版
      </button>
      <span className='work-toolbar-divider' />
      <button type='button' onClick={() => onAddElement('text')}>
        <Type size={15} />
        文本框
      </button>
      <button type='button' onClick={() => onAddElement('shape')}>
        <Square size={15} />
        形状
      </button>
      <button type='button' onClick={onRequestImage}>
        <Image size={15} />
        图片
      </button>
      {!editingDesign && (
        <>
          <button type='button' onClick={onAddTable}>
            <Table2 size={15} />
            表格
          </button>
          <button type='button' onClick={onAddChart}>
            <BarChart3 size={15} />
            图表
          </button>
          <button type='button' aria-label='添加演示批注' onClick={onAddComment}>
            <MessageSquarePlus size={15} />
            批注
          </button>
          <button
            type='button'
            className={commentsOpen ? 'active' : ''}
            aria-label={`审阅演示批注${commentCount ? `（${commentCount}）` : ''}`}
            aria-pressed={commentsOpen}
            onClick={onToggleComments}
          >
            <MessagesSquare size={15} />
            审阅
          </button>
        </>
      )}
      <span className='work-toolbar-divider' />
      {selectedElement ? (
        <>
          <label className='presentation-number-field'>
            <span>字号</span>
            <input
              type='number'
              min={8}
              max={96}
              value={selectedElement.fontSize}
              onChange={(event) => onUpdateElement({ fontSize: Number(event.target.value) || 8 })}
            />
          </label>
          <button
            type='button'
            className={selectedElement.bold ? 'active' : ''}
            aria-label='加粗'
            onClick={() => onUpdateElement({ bold: !selectedElement.bold })}
          >
            <Bold size={15} />
          </button>
          {(['left', 'center', 'right'] as WorkSlideTextAlign[]).map((align) => (
            <button
              type='button'
              className={selectedElement.align === align ? 'active' : ''}
              aria-label={`${align} 对齐`}
              key={align}
              onClick={() => onUpdateElement({ align })}
            >
              {align === 'left' ? (
                <AlignLeft size={15} />
              ) : align === 'center' ? (
                <AlignCenter size={15} />
              ) : (
                <AlignRight size={15} />
              )}
            </button>
          ))}
          <label className='work-color-tool' title='文字颜色'>
            <span style={{ background: selectedElement.color }} />
            <input
              type='color'
              value={selectedElement.color}
              aria-label='演示文字颜色'
              onInput={(event) => onUpdateElement({ color: event.currentTarget.value })}
            />
          </label>
          <button type='button' aria-label='下移一层' title='下移一层' onClick={() => onReorderElement(-1)}>
            <ArrowDownToLine size={15} />
          </button>
          <button type='button' aria-label='上移一层' title='上移一层' onClick={() => onReorderElement(1)}>
            <ArrowUpToLine size={15} />
          </button>
          {(selectedElement.type === 'text' || selectedElement.type === 'shape') && (
            <button
              type='button'
              className={selectedElement.href ? 'active' : ''}
              aria-label='设置链接'
              onClick={() => {
                const href = window.prompt('链接地址', selectedElement.href ?? 'https://');
                if (href !== null) onUpdateElement({ href: href.trim() || undefined });
              }}
            >
              <Link2 size={15} />
            </button>
          )}
        </>
      ) : (
        <label className='work-color-tool slide-background-tool' title='幻灯片背景'>
          <span style={{ background: background ?? selectedSlide.background }} />
          <input
            type='color'
            value={background ?? selectedSlide.background}
            aria-label={editingDesign ? '设计背景颜色' : '幻灯片背景颜色'}
            onInput={(event) => onSetBackground(event.currentTarget.value)}
          />
        </label>
      )}
    </div>
  );
}
