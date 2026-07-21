import Color from '@tiptap/extension-color';
import Highlight from '@tiptap/extension-highlight';
import Image from '@tiptap/extension-image';
import Placeholder from '@tiptap/extension-placeholder';
import { TableKit } from '@tiptap/extension-table';
import TextAlign from '@tiptap/extension-text-align';
import { TextStyle } from '@tiptap/extension-text-style';
import Underline from '@tiptap/extension-underline';
import { EditorContent, useEditor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import { Copy, Languages, MessageSquareText, Sparkles, TextQuote } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { WorkspaceContextMenu, type WorkspaceContextMenuItem } from '../../workspace/components/workspace-context-menu';
import { showToast } from '../../../state/app-state';
import {
  editorDocumentCaptionTargets,
  insertDocumentCaption,
  insertDocumentCrossReference,
} from '../work-document-caption-editor';
import { DocumentCaption, DocumentCrossReference } from '../work-document-caption-nodes';
import { documentCitationCount } from '../work-document-citation-editor';
import { DocumentBibliography, DocumentCitation } from '../work-document-citation-nodes';
import { insertDocumentField, refreshDocumentFields } from '../work-document-field-editor';
import { DocumentField } from '../work-document-field-node';
import {
  collectDocumentCommentAnchors,
  DocumentComment,
  documentCommentViews,
  insertDocumentComment,
  removeDocumentComment,
  retainAnchoredDocumentComments,
} from '../work-document-comments';
import {
  documentInitialSectionLayout,
  normalizeDocumentHtml,
  syncDocumentContentFromHtml,
} from '../work-document-section';
import { insertDocumentNote } from '../work-document-note-editor';
import { DocumentNote, DocumentNoteReference } from '../work-document-note-nodes';
import {
  documentPageChromeLegacyFields,
  normalizeDocumentPageChrome,
  updateDocumentPageChromeVariant,
} from '../work-document-page-chrome';
import {
  activeDocumentSection,
  insertDocumentSection,
  mergeDocumentSectionWithPrevious,
  updateActiveDocumentSection,
} from '../work-document-section-editor';
import { DocumentSection } from '../work-document-section-node';
import { documentMargins, millimetersToPixels } from '../work-document-layout';
import { DocumentPageBreak } from '../work-document-page-break';
import type { WorkDocumentContent } from '../work-types';
import type { WorkEditorAgentRequest } from '../work-agent-request';
import {
  createWorkAgentProposalRequest,
  type WorkAgentProposalRequest,
  type WorkAgentProposalTarget,
} from '../work-agent-proposal';
import {
  collectDocumentChanges,
  DocumentChange,
  replaceDocumentTextWithTrackedChange,
  type WorkDocumentChangeKind,
} from '../work-document-changes';
import { createWorkId } from '../work-templates';
import { DocumentChangesPanel } from './document-changes-panel';
import { DocumentCitationsPanel } from './document-citations-panel';
import { DocumentCommentsPanel } from './document-comments-panel';
import { DocumentLayoutPanel } from './document-layout-panel';
import { DocumentToolbar } from './document-toolbar';
import { WorkDocumentPreview } from '../components/work-document-pages';

interface DocumentEditorProps {
  content: WorkDocumentContent;
  preview: boolean;
  onChange: (content: WorkDocumentContent) => void;
  onAgentRequest?: (request: WorkEditorAgentRequest) => void | Promise<void>;
}

export function DocumentEditor({ content, preview, onChange, onAgentRequest }: DocumentEditorProps) {
  const imageInputRef = useRef<HTMLInputElement>(null);
  const contentRef = useRef(content);
  const trackChangesRef = useRef(Boolean(content.trackChanges));
  const [layoutOpen, setLayoutOpen] = useState(false);
  const [changesOpen, setChangesOpen] = useState(false);
  const [commentsOpen, setCommentsOpen] = useState(false);
  const [citationsOpen, setCitationsOpen] = useState(false);
  const [agentMenu, setAgentMenu] = useState<{
    x: number;
    y: number;
    selection: string;
    rawSelection: string;
    from: number;
    to: number;
  } | null>(null);
  const [, setSelectionVersion] = useState(0);
  contentRef.current = content;
  trackChangesRef.current = Boolean(content.trackChanges);
  const createTrackedChange = (_kind: WorkDocumentChangeKind) => ({
    id: createWorkId('change'),
    author: 'A3S Work 用户',
    date: new Date().toISOString(),
  });
  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        link: {
          autolink: true,
          defaultProtocol: 'https',
          openOnClick: false,
        },
        underline: false,
      }),
      DocumentSection,
      DocumentCaption,
      DocumentCrossReference,
      DocumentCitation,
      DocumentBibliography,
      DocumentField,
      DocumentComment,
      DocumentNoteReference,
      DocumentNote,
      Underline,
      Image.configure({
        allowBase64: true,
        resize: { enabled: true, alwaysPreserveAspectRatio: true, minWidth: 60, minHeight: 40 },
      }),
      TableKit.configure({
        table: { resizable: true, allowTableNodeSelection: true },
      }),
      TextStyle,
      Color,
      Highlight.configure({ multicolor: true }),
      TextAlign.configure({ types: ['heading', 'paragraph'] }),
      Placeholder.configure({ placeholder: '在这里开始输入…' }),
      DocumentPageBreak,
      DocumentChange.configure({
        isTracking: () => trackChangesRef.current,
        createChange: createTrackedChange,
      }),
    ],
    content: normalizeDocumentHtml(content),
    editable: !preview,
    editorProps: {
      attributes: {
        'aria-label': '文档正文',
        'aria-multiline': 'true',
        role: 'textbox',
        spellcheck: 'true',
      },
    },
    onUpdate: ({ editor: current }) => {
      const anchors = collectDocumentCommentAnchors(current.state.doc);
      const next = {
        ...syncDocumentContentFromHtml(contentRef.current, current.getHTML()),
        comments: retainAnchoredDocumentComments(contentRef.current.comments ?? [], anchors),
      };
      contentRef.current = next;
      onChange(next);
    },
    onSelectionUpdate: () => setSelectionVersion((value) => value + 1),
  });

  useEffect(() => {
    editor?.setEditable(!preview);
  }, [editor, preview]);

  if (!editor) {
    return <output className='work-editor-loading'>正在准备文字编辑器…</output>;
  }

  const section = activeDocumentSection(editor);
  const layout = section?.layout ?? documentInitialSectionLayout(content);
  const margins = documentMargins({
    ...content,
    pageSize: layout.pageSize,
    margins: layout.margins,
  });
  const pageCount = documentPageCount(editor);
  const pageStart = Math.max(1, layout.pageNumberStart ?? 1);
  const pageChrome = normalizeDocumentPageChrome(layout.pageChrome, layout);
  const defaultChrome = pageChrome.default;
  const changes = collectDocumentChanges(editor.state.doc);
  const commentAnchors = collectDocumentCommentAnchors(editor.state.doc);
  const comments = documentCommentViews(content.comments ?? [], commentAnchors);
  const citationCount = documentCitationCount(editor);
  const updateLayout = (next: typeof layout) => {
    updateActiveDocumentSection(editor, next);
  };
  const addSection = () => {
    insertDocumentSection(editor, layout.breakAfter);
  };

  if (preview) {
    return (
      <section className='work-document-editor preview'>
        <WorkDocumentPreview content={content} />
        <footer className='work-document-status'>
          <span>{pageCount} 页</span>
          <span>{section?.count ?? 1} 节</span>
        </footer>
      </section>
    );
  }

  return (
    <section className='work-document-editor'>
      <input
        ref={imageInputRef}
        className='work-file-input'
        type='file'
        accept='image/*'
        aria-label='插入文档图片'
        onChange={(event) => {
          const file = event.target.files?.[0];
          event.target.value = '';
          if (!file) return;
          if (file.size > 8 * 1024 * 1024) {
            window.alert('单张图片不能超过 8 MiB。');
            return;
          }
          void fileToDataUrl(file).then((src) =>
            editor.chain().focus().setImage({ src, alt: file.name, title: file.name }).run()
          );
        }}
      />
      <DocumentToolbar
        editor={editor}
        layoutOpen={layoutOpen}
        showPageNumbers={defaultChrome.showPageNumber}
        onRequestImage={() => imageInputRef.current?.click()}
        onToggleLayout={() => setLayoutOpen((value) => !value)}
        onTogglePageNumbers={() => {
          const nextPageChrome = updateDocumentPageChromeVariant(pageChrome, 'default', {
            showPageNumber: !defaultChrome.showPageNumber,
          });
          updateLayout({ ...layout, pageChrome: nextPageChrome, ...documentPageChromeLegacyFields(nextPageChrome) });
        }}
        onInsertSection={addSection}
        onInsertNote={(kind) => insertDocumentNote(editor, kind)}
        onInsertCaption={(kind) => {
          const title = window.prompt(kind === 'figure' ? '图片题注文字' : '表格题注文字', '');
          if (title !== null) insertDocumentCaption(editor, kind, title);
        }}
        onInsertCrossReference={() => {
          const targets = editorDocumentCaptionTargets(editor);
          if (!targets.length) {
            window.alert('请先插入图片或表格题注。');
            return;
          }
          const choice = window.prompt(
            `引用题注（${targets.map((target) => target.display).join('、')}）`,
            targets[0].display
          );
          if (choice === null) return;
          const target = targets.find(
            (item) => item.display === choice.trim() || `${item.display} ${item.title}`.trim() === choice.trim()
          );
          if (!target) {
            window.alert('没有找到该题注。');
            return;
          }
          insertDocumentCrossReference(editor, target);
        }}
        citationsOpen={citationsOpen}
        citationSourceCount={content.bibliography?.sources.length ?? 0}
        onToggleCitations={() => setCitationsOpen((value) => !value)}
        onInsertField={(kind) => insertDocumentField(editor, kind, contentRef.current)}
        onRefreshFields={() => {
          refreshDocumentFields(editor, contentRef.current);
        }}
        onInsertComment={() => {
          if (editor.state.selection.empty) {
            window.alert('请先选择要批注的文字。');
            return;
          }
          const text = window.prompt('批注内容', '');
          if (!text?.trim()) return;
          const comment = {
            id: createWorkId('comment'),
            author: 'A3S Work 用户',
            date: new Date().toISOString(),
            text: text.trim(),
            resolved: false,
          };
          const previous = contentRef.current;
          contentRef.current = {
            ...previous,
            comments: [...(previous.comments ?? []), comment],
          };
          if (!insertDocumentComment(editor, comment.id)) {
            contentRef.current = previous;
            window.alert('所选文字已经包含批注，请选择其他文字。');
            return;
          }
          setCommentsOpen(true);
        }}
        commentsOpen={commentsOpen}
        commentCount={comments.length}
        onToggleComments={() => setCommentsOpen((value) => !value)}
        trackChanges={Boolean(content.trackChanges)}
        changesOpen={changesOpen}
        changeCount={changes.length}
        onToggleTrackChanges={() => {
          const trackChanges = !trackChangesRef.current;
          trackChangesRef.current = trackChanges;
          onChange({ ...contentRef.current, trackChanges });
        }}
        onToggleChanges={() => setChangesOpen((value) => !value)}
        onReplaceText={(from, to, replacement) => {
          editor.commands.focus();
          if (trackChangesRef.current) {
            return replaceDocumentTextWithTrackedChange(editor, from, to, replacement, createTrackedChange);
          }
          return editor
            .chain()
            .focus()
            .setTextSelection({ from, to })
            .insertContent(plainTextAsHtml(replacement))
            .run();
        }}
      />
      {citationsOpen && (
        <DocumentCitationsPanel
          editor={editor}
          content={content}
          onChange={(next) => {
            contentRef.current = next;
            onChange(next);
          }}
          onClose={() => setCitationsOpen(false)}
        />
      )}
      {commentsOpen && (
        <DocumentCommentsPanel
          editor={editor}
          comments={comments}
          onReply={(id, text) => {
            const next = {
              ...contentRef.current,
              comments: (contentRef.current.comments ?? []).map((comment) =>
                comment.id === id
                  ? {
                      ...comment,
                      replies: [
                        ...(comment.replies ?? []),
                        {
                          id: createWorkId('comment-reply'),
                          author: 'A3S Work 用户',
                          date: new Date().toISOString(),
                          text,
                        },
                      ],
                    }
                  : comment
              ),
            };
            contentRef.current = next;
            onChange(next);
          }}
          onToggleResolved={(id) => {
            const next = {
              ...contentRef.current,
              comments: (contentRef.current.comments ?? []).map((comment) =>
                comment.id === id ? { ...comment, resolved: !comment.resolved } : comment
              ),
            };
            contentRef.current = next;
            onChange(next);
          }}
          onDelete={(id) => {
            const next = {
              ...contentRef.current,
              comments: (contentRef.current.comments ?? []).filter((comment) => comment.id !== id),
            };
            contentRef.current = next;
            if (!removeDocumentComment(editor, id)) onChange(next);
          }}
          onClose={() => setCommentsOpen(false)}
        />
      )}
      {changesOpen && <DocumentChangesPanel editor={editor} changes={changes} onClose={() => setChangesOpen(false)} />}
      {layoutOpen && section && (
        <DocumentLayoutPanel
          layout={layout}
          sectionIndex={section.index}
          sectionCount={section.count}
          onChange={updateLayout}
          onInsertSection={addSection}
          onMergeSection={() => mergeDocumentSectionWithPrevious(editor)}
        />
      )}
      <div className='work-document-scroll'>
        <article
          className={`work-document-page ${layout.pageSize} ${layout.orientation}`}
          aria-label={preview ? '文字预览' : '文字页面'}
          style={{
            padding: `${millimetersToPixels(margins.top)}px ${millimetersToPixels(
              margins.right
            )}px ${millimetersToPixels(margins.bottom)}px ${millimetersToPixels(margins.left)}px`,
          }}
        >
          {(defaultChrome.headerHtml || layoutOpen) && (
            <header className='work-document-page-header'>
              {defaultChrome.headerHtml ? (
                <div
                  className='work-document-page-chrome-html'
                  dangerouslySetInnerHTML={{ __html: defaultChrome.headerHtml }}
                />
              ) : (
                '页眉'
              )}
            </header>
          )}
          <section
            className='work-document-editable'
            aria-label='文档内容编辑区域'
            onContextMenu={(event) => {
              if (!onAgentRequest) return;
              const { from, to, empty } = editor.state.selection;
              if (empty) return;
              const rawSelection = editor.state.doc.textBetween(from, to, '\n');
              const selection = rawSelection.trim();
              if (!selection) return;
              event.preventDefault();
              setAgentMenu({ x: event.clientX, y: event.clientY, selection, rawSelection, from, to });
            }}
          >
            <EditorContent editor={editor} />
          </section>
          {(defaultChrome.footerHtml || defaultChrome.showPageNumber || layoutOpen) && (
            <footer className='work-document-page-footer'>
              {defaultChrome.footerHtml ? (
                <div
                  className='work-document-page-chrome-html'
                  dangerouslySetInnerHTML={{ __html: defaultChrome.footerHtml }}
                />
              ) : (
                <span>{layoutOpen ? '页脚' : ''}</span>
              )}
              {defaultChrome.showPageNumber && (
                <span>
                  {pageStart} / {pageStart + pageCount - 1}
                </span>
              )}
            </footer>
          )}
        </article>
      </div>
      <footer className='work-document-status'>
        <span>
          {layout.pageSize === 'a4' ? 'A4' : 'Letter'} · {layout.orientation === 'portrait' ? '纵向' : '横向'} ·{' '}
          {layout.columns.count} 栏{layout.columns.custom ? ' · 自定义栏宽' : ''}
        </span>
        <span>{editor.getText().trim().length} 字符</span>
        <span>
          {content.bibliography?.sources.length ?? 0} 条文献 · {citationCount} 处引文
        </span>
        <span>{section?.count ?? 1} 节</span>
        <span>{pageCount} 页</span>
      </footer>
      {agentMenu && onAgentRequest && (
        <WorkspaceContextMenu
          label='选中文本 AI 操作'
          x={agentMenu.x}
          y={agentMenu.y}
          items={documentAgentMenuItems(agentMenu.selection, onAgentRequest, {
            target: {
              id: 'document-selection',
              label: '选中文本',
              before: agentMenu.rawSelection,
            },
            apply: (changes) => {
              const change = changes.find((candidate) => candidate.id === 'document-selection');
              if (!change) return { appliedTargetIds: [], conflicts: [] };
              const current = editor.state.doc.textBetween(agentMenu.from, agentMenu.to, '\n');
              if (current !== agentMenu.rawSelection) {
                return {
                  appliedTargetIds: [],
                  conflicts: [
                    {
                      targetId: change.id,
                      label: change.label,
                      message: '选中文本在建议生成后已发生变化。',
                    },
                  ],
                };
              }
              const applied = trackChangesRef.current
                ? replaceDocumentTextWithTrackedChange(
                    editor,
                    agentMenu.from,
                    agentMenu.to,
                    change.after,
                    createTrackedChange
                  )
                : editor
                    .chain()
                    .focus()
                    .setTextSelection({ from: agentMenu.from, to: agentMenu.to })
                    .insertContent(plainTextAsHtml(change.after))
                    .run();
              return applied
                ? { appliedTargetIds: [change.id], conflicts: [] }
                : {
                    appliedTargetIds: [],
                    conflicts: [
                      {
                        targetId: change.id,
                        label: change.label,
                        message: '编辑器无法替换当前选区。',
                      },
                    ],
                  };
            },
          })}
          onClose={() => setAgentMenu(null)}
        />
      )}
    </section>
  );
}

export function documentEditorSelectionText(editor: Pick<NonNullable<ReturnType<typeof useEditor>>, 'state'>): string {
  const { from, to, empty } = editor.state.selection;
  if (empty) return '';
  return editor.state.doc.textBetween(from, to, '\n').trim();
}

export function documentAgentMenuItems(
  selection: string,
  onAgentRequest: (request: WorkEditorAgentRequest) => void | Promise<void>,
  proposalOptions?: {
    target: WorkAgentProposalTarget;
    apply: WorkAgentProposalRequest['apply'];
  }
): WorkspaceContextMenuItem[] {
  return [
    {
      id: 'copy',
      label: '复制',
      icon: <Copy size={14} />,
      onSelect: () => {
        void copyDocumentSelection(selection);
      },
    },
    {
      id: 'ask',
      label: '询问 AI 助手',
      icon: <MessageSquareText size={14} />,
      separatorBefore: true,
      onSelect: () =>
        void onAgentRequest({
          instruction: '请围绕这段选中文本回答我的问题：\n\n问题：',
          selection,
        }),
    },
    {
      id: 'summarize',
      label: '总结选中内容',
      icon: <TextQuote size={14} />,
      onSelect: () =>
        void onAgentRequest({
          instruction: '请用简洁、准确的语言总结这段选中文本，保留关键事实和结论。',
          selection,
        }),
    },
    {
      id: 'rewrite',
      label: '改写得更清晰',
      icon: <Sparkles size={14} />,
      onSelect: () =>
        void onAgentRequest({
          instruction: '请改写这段选中文本，使表达更清晰、自然、专业，并说明主要改动。先提供建议稿，不要直接修改文档。',
          selection,
          proposal: proposalOptions
            ? createWorkAgentProposalRequest({
                title: '审阅文字改写',
                description: `选中文本 · ${selection.length} 个字符`,
                targets: [proposalOptions.target],
                apply: proposalOptions.apply,
              })
            : undefined,
        }),
    },
    {
      id: 'translate',
      label: '翻译选中内容',
      icon: <Languages size={14} />,
      onSelect: () =>
        void onAgentRequest({
          instruction:
            '请翻译这段选中文本。请先判断原语言，并询问或根据上下文确定目标语言；先提供译文，不要直接修改文档。',
          selection,
          proposal: proposalOptions
            ? createWorkAgentProposalRequest({
                title: '审阅翻译建议',
                description: `选中文本 · ${selection.length} 个字符`,
                targets: [proposalOptions.target],
                apply: proposalOptions.apply,
              })
            : undefined,
        }),
    },
  ];
}

function plainTextAsHtml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')
    .replace(/\r?\n/g, '<br>');
}

async function copyDocumentSelection(selection: string): Promise<void> {
  try {
    if (!navigator.clipboard?.writeText) throw new Error('Clipboard API is unavailable');
    await navigator.clipboard.writeText(selection);
    showToast('选中文本已复制', 'success');
  } catch {
    showToast('无法访问剪贴板，请使用系统复制快捷键。', 'error');
  }
}

function documentPageCount(editor: NonNullable<ReturnType<typeof useEditor>>): number {
  let pages = 1;
  const sections: Array<{ breakAfter?: string }> = [];
  editor.state.doc.forEach((node) => {
    if (node.type.name !== 'documentSection') return;
    sections.push(node.attrs);
    node.descendants((child) => {
      if (child.type.name === 'documentNote') return false;
      if (child.type.name === 'pageBreak') pages += 1;
    });
  });
  for (let index = 0; index < sections.length - 1; index += 1) {
    if (sections[index].breakAfter !== 'continuous' && sections[index].breakAfter !== 'nextColumn') pages += 1;
  }
  return pages;
}

function fileToDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.addEventListener('load', () => resolve(String(reader.result)));
    reader.addEventListener('error', () => reject(reader.error ?? new Error('Image could not be read')));
    reader.readAsDataURL(file);
  });
}
