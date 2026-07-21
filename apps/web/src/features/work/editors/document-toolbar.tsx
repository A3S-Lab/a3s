import type { Editor } from '@tiptap/core';
import {
  AlignCenter,
  AlignLeft,
  AlignRight,
  Bold,
  BookOpen,
  Columns3,
  FileDiff,
  FilePlus2,
  Hash,
  Highlighter,
  Image as ImageIcon,
  Italic,
  Link2,
  List,
  ListChecks,
  ListOrdered,
  MessageSquarePlus,
  MessagesSquare,
  Redo2,
  RefreshCw,
  Replace,
  Search,
  Settings2,
  Strikethrough,
  Table2,
  Underline as UnderlineIcon,
  Undo2,
} from 'lucide-react';
import type { ReactNode } from 'react';
import type { WorkDocumentCaptionKind } from '../work-document-captions';
import type { WorkDocumentFieldKind } from '../work-document-fields';
import type { WorkDocumentNoteKind } from '../work-document-notes';

export function DocumentToolbar({
  editor,
  layoutOpen,
  showPageNumbers,
  onRequestImage,
  onToggleLayout,
  onTogglePageNumbers,
  onInsertSection,
  onInsertNote,
  onInsertCaption,
  onInsertCrossReference,
  citationsOpen,
  citationSourceCount,
  onToggleCitations,
  onInsertField,
  onRefreshFields,
  onInsertComment,
  commentsOpen,
  commentCount,
  onToggleComments,
  trackChanges,
  changesOpen,
  changeCount,
  onToggleTrackChanges,
  onToggleChanges,
  onReplaceText,
}: {
  editor: Editor;
  layoutOpen: boolean;
  showPageNumbers: boolean;
  onRequestImage: () => void;
  onToggleLayout: () => void;
  onTogglePageNumbers: () => void;
  onInsertSection: () => void;
  onInsertNote: (kind: WorkDocumentNoteKind) => void;
  onInsertCaption: (kind: WorkDocumentCaptionKind) => void;
  onInsertCrossReference: () => void;
  citationsOpen: boolean;
  citationSourceCount: number;
  onToggleCitations: () => void;
  onInsertField: (kind: WorkDocumentFieldKind) => void;
  onRefreshFields: () => void;
  onInsertComment: () => void;
  commentsOpen: boolean;
  commentCount: number;
  onToggleComments: () => void;
  trackChanges: boolean;
  changesOpen: boolean;
  changeCount: number;
  onToggleTrackChanges: () => void;
  onToggleChanges: () => void;
  onReplaceText: (from: number, to: number, replacement: string) => boolean;
}) {
  return (
    <div className='work-office-toolbar document-toolbar' role='toolbar' aria-label='文字格式工具栏'>
      <select
        aria-label='段落样式'
        value={
          editor.isActive('heading', { level: 1 })
            ? 'h1'
            : editor.isActive('heading', { level: 2 })
              ? 'h2'
              : editor.isActive('heading', { level: 3 })
                ? 'h3'
                : 'paragraph'
        }
        onChange={(event) => {
          const value = event.target.value;
          if (value === 'paragraph') editor.chain().focus().setParagraph().run();
          else
            editor
              .chain()
              .focus()
              .toggleHeading({ level: Number(value.slice(1)) as 1 | 2 | 3 })
              .run();
        }}
      >
        <option value='paragraph'>正文</option>
        <option value='h1'>标题 1</option>
        <option value='h2'>标题 2</option>
        <option value='h3'>标题 3</option>
      </select>
      <span className='work-toolbar-divider' />
      <ToolbarButton
        label='加粗'
        active={editor.isActive('bold')}
        onClick={() => editor.chain().focus().toggleBold().run()}
      >
        <Bold size={15} />
      </ToolbarButton>
      <ToolbarButton
        label='斜体'
        active={editor.isActive('italic')}
        onClick={() => editor.chain().focus().toggleItalic().run()}
      >
        <Italic size={15} />
      </ToolbarButton>
      <ToolbarButton
        label='下划线'
        active={editor.isActive('underline')}
        onClick={() => editor.chain().focus().toggleUnderline().run()}
      >
        <UnderlineIcon size={15} />
      </ToolbarButton>
      <ToolbarButton
        label='删除线'
        active={editor.isActive('strike')}
        onClick={() => editor.chain().focus().toggleStrike().run()}
      >
        <Strikethrough size={15} />
      </ToolbarButton>
      <label className='work-color-tool' title='文字颜色'>
        <span style={{ background: editor.getAttributes('textStyle').color ?? '#172033' }} />
        <input
          type='color'
          value={editor.getAttributes('textStyle').color ?? '#172033'}
          aria-label='文字颜色'
          onInput={(event) => editor.chain().focus().setColor(event.currentTarget.value).run()}
        />
      </label>
      <ToolbarButton
        label='突出显示'
        active={editor.isActive('highlight')}
        onClick={() => editor.chain().focus().toggleHighlight({ color: '#fff0a6' }).run()}
      >
        <Highlighter size={15} />
      </ToolbarButton>
      <span className='work-toolbar-divider' />
      <ToolbarButton
        label='项目符号'
        active={editor.isActive('bulletList')}
        onClick={() => editor.chain().focus().toggleBulletList().run()}
      >
        <List size={15} />
      </ToolbarButton>
      <ToolbarButton
        label='编号'
        active={editor.isActive('orderedList')}
        onClick={() => editor.chain().focus().toggleOrderedList().run()}
      >
        <ListOrdered size={15} />
      </ToolbarButton>
      <ToolbarButton label='插入图片' onClick={onRequestImage}>
        <ImageIcon size={15} />
      </ToolbarButton>
      <ToolbarButton
        label='插入表格'
        onClick={() => editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run()}
      >
        <Table2 size={15} />
      </ToolbarButton>
      <ToolbarButton
        label='插入分页符'
        onClick={() => editor.chain().focus().insertContent({ type: 'pageBreak' }).run()}
      >
        <FilePlus2 size={15} />
      </ToolbarButton>
      <ToolbarButton label='插入分节符' onClick={onInsertSection}>
        <Columns3 size={15} />
      </ToolbarButton>
      <ToolbarButton label='插入脚注' onClick={() => onInsertNote('footnote')}>
        脚注
      </ToolbarButton>
      <ToolbarButton label='插入尾注' onClick={() => onInsertNote('endnote')}>
        尾注
      </ToolbarButton>
      <ToolbarButton label='插入图片题注' onClick={() => onInsertCaption('figure')}>
        图题
      </ToolbarButton>
      <ToolbarButton label='插入表格题注' onClick={() => onInsertCaption('table')}>
        表题
      </ToolbarButton>
      <ToolbarButton label='插入交叉引用' onClick={onInsertCrossReference}>
        引用
      </ToolbarButton>
      <ToolbarButton
        label={`文献库${citationSourceCount ? `（${citationSourceCount}）` : ''}`}
        active={citationsOpen}
        onClick={onToggleCitations}
      >
        <BookOpen size={15} />
      </ToolbarButton>
      <select
        aria-label='插入正文域'
        value=''
        onChange={(event) => {
          const kind = event.target.value as WorkDocumentFieldKind;
          if (kind) onInsertField(kind);
        }}
      >
        <option value=''>插入域</option>
        <option value='page'>当前页码</option>
        <option value='numPages'>总页数</option>
        <option value='section'>当前节号</option>
        <option value='sectionPages'>本节页数</option>
        <option value='date'>当前日期</option>
        <option value='time'>当前时间</option>
      </select>
      <ToolbarButton label='更新所有正文域' onClick={onRefreshFields}>
        <RefreshCw size={15} />
      </ToolbarButton>
      <ToolbarButton label='添加批注' onClick={onInsertComment}>
        <MessageSquarePlus size={15} />
      </ToolbarButton>
      <ToolbarButton
        label={editor.isActive('link') ? '取消链接' : '添加链接'}
        active={editor.isActive('link')}
        onClick={() => {
          if (editor.isActive('link')) {
            editor.chain().focus().unsetLink().run();
            return;
          }
          const href = window.prompt('链接地址', editor.getAttributes('link').href ?? 'https://');
          if (href?.trim()) editor.chain().focus().setLink({ href: href.trim() }).run();
        }}
      >
        <Link2 size={15} />
      </ToolbarButton>
      {editor.isActive('table') && (
        <>
          <ToolbarButton label='添加行' onClick={() => editor.chain().focus().addRowAfter().run()}>
            + 行
          </ToolbarButton>
          <ToolbarButton label='添加列' onClick={() => editor.chain().focus().addColumnAfter().run()}>
            + 列
          </ToolbarButton>
          <ToolbarButton label='删除表格' onClick={() => editor.chain().focus().deleteTable().run()}>
            × 表
          </ToolbarButton>
        </>
      )}
      <span className='work-toolbar-divider' />
      <ToolbarButton label='页面设置' active={layoutOpen} onClick={onToggleLayout}>
        <Settings2 size={15} />
      </ToolbarButton>
      <ToolbarButton label='显示页码' active={showPageNumbers} onClick={onTogglePageNumbers}>
        <Hash size={15} />
      </ToolbarButton>
      <ToolbarButton
        label={`审阅批注${commentCount ? `（${commentCount}）` : ''}`}
        active={commentsOpen}
        onClick={onToggleComments}
      >
        <MessagesSquare size={15} />
      </ToolbarButton>
      <ToolbarButton label='修订模式' active={trackChanges} onClick={onToggleTrackChanges}>
        <FileDiff size={15} />
      </ToolbarButton>
      <ToolbarButton
        label={`审阅修订${changeCount ? `（${changeCount}）` : ''}`}
        active={changesOpen}
        onClick={onToggleChanges}
      >
        <ListChecks size={15} />
      </ToolbarButton>
      <span className='work-toolbar-divider' />
      <ToolbarButton
        label='左对齐'
        active={editor.isActive({ textAlign: 'left' })}
        onClick={() => editor.chain().focus().setTextAlign('left').run()}
      >
        <AlignLeft size={15} />
      </ToolbarButton>
      <ToolbarButton
        label='居中'
        active={editor.isActive({ textAlign: 'center' })}
        onClick={() => editor.chain().focus().setTextAlign('center').run()}
      >
        <AlignCenter size={15} />
      </ToolbarButton>
      <ToolbarButton
        label='右对齐'
        active={editor.isActive({ textAlign: 'right' })}
        onClick={() => editor.chain().focus().setTextAlign('right').run()}
      >
        <AlignRight size={15} />
      </ToolbarButton>
      <span className='work-toolbar-spacer' />
      <ToolbarButton label='查找' onClick={() => findDocumentText(editor, false)}>
        <Search size={15} />
      </ToolbarButton>
      <ToolbarButton label='替换' onClick={() => findDocumentText(editor, true, onReplaceText)}>
        <Replace size={15} />
      </ToolbarButton>
      <ToolbarButton
        label='撤销'
        disabled={!editor.can().chain().focus().undo().run()}
        onClick={() => editor.chain().focus().undo().run()}
      >
        <Undo2 size={15} />
      </ToolbarButton>
      <ToolbarButton
        label='重做'
        disabled={!editor.can().chain().focus().redo().run()}
        onClick={() => editor.chain().focus().redo().run()}
      >
        <Redo2 size={15} />
      </ToolbarButton>
    </div>
  );
}

function ToolbarButton({
  label,
  active = false,
  disabled = false,
  onClick,
  children,
}: {
  label: string;
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type='button'
      className={active ? 'active' : ''}
      aria-label={label}
      title={label}
      aria-pressed={active}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function findDocumentText(
  editor: Editor,
  replace: boolean,
  onReplaceText?: (from: number, to: number, replacement: string) => boolean
) {
  const query = window.prompt(replace ? '查找要替换的文字' : '查找文字');
  if (!query) return;
  const range = textRange(editor, query, editor.state.selection.to) ?? textRange(editor, query, 0);
  if (!range) {
    window.alert(`没有找到“${query}”。`);
    return;
  }
  const chain = editor.chain().focus().setTextSelection(range);
  if (!replace) {
    chain.run();
    return;
  }
  const replacement = window.prompt('替换为', query);
  if (replacement !== null) {
    if (onReplaceText) onReplaceText(range.from, range.to, replacement);
    else chain.insertContent(replacement).run();
  }
}

function textRange(editor: Editor, query: string, from: number): { from: number; to: number } | null {
  let match: { from: number; to: number } | null = null;
  editor.state.doc.descendants((node, position) => {
    if (match || !node.isText || !node.text) return;
    const start = Math.max(0, from - position);
    const index = node.text.indexOf(query, start);
    if (index >= 0) match = { from: position + index, to: position + index + query.length };
  });
  return match;
}
