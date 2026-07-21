import { type Editor, Mark, mergeAttributes } from '@tiptap/core';
import { Fragment, type Mark as ProseMirrorMark, type Node as ProseMirrorNode, Slice } from '@tiptap/pm/model';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import type { WorkDocumentComment } from './work-types';

export interface WorkDocumentCommentAnchor {
  id: string;
  from: number;
  to: number;
  anchorText: string;
}

export interface WorkDocumentCommentView extends WorkDocumentComment, WorkDocumentCommentAnchor {}

const documentCommentPastePluginKey = new PluginKey('documentCommentPaste');

export const DocumentComment = Mark.create({
  name: 'documentComment',
  priority: 1050,
  inclusive: false,
  keepOnSplit: true,
  excludes: '',

  addAttributes() {
    return {
      id: {
        default: '',
        parseHTML: (element) => element.getAttribute('data-comment-id') ?? '',
        renderHTML: (attributes) => ({ 'data-comment-id': attributes.id }),
      },
    };
  },

  parseHTML() {
    return [{ tag: 'span[data-document-comment]' }];
  },

  renderHTML({ HTMLAttributes }) {
    return ['span', mergeAttributes(HTMLAttributes, { 'data-document-comment': 'true' }), 0];
  },

  addProseMirrorPlugins() {
    const type = this.type;
    return [
      new Plugin({
        key: documentCommentPastePluginKey,
        props: {
          transformPasted: (slice) => stripDocumentCommentsFromSlice(slice, type),
        },
      }),
    ];
  },
});

export function insertDocumentComment(editor: Editor, id: string): boolean {
  const type = editor.schema.marks.documentComment;
  const { from, to, empty } = editor.state.selection;
  if (!type || empty || !id.trim() || selectionContainsDocumentComment(editor.state.doc, from, to)) return false;
  const transaction = editor.state.tr.addMark(from, to, type.create({ id: id.trim() }));
  if (!transaction.docChanged) return false;
  editor.view.dispatch(transaction);
  return true;
}

export function removeDocumentComment(editor: Editor, id: string): boolean {
  const type = editor.schema.marks.documentComment;
  if (!type) return false;
  const segments = documentCommentSegments(editor.state.doc, type).filter((segment) => segment.id === id);
  if (!segments.length) return false;
  const transaction = editor.state.tr;
  for (const segment of segments) transaction.removeMark(segment.from, segment.to, type);
  if (!transaction.docChanged) return false;
  editor.view.dispatch(transaction);
  return true;
}

export function collectDocumentCommentAnchors(document: ProseMirrorNode): WorkDocumentCommentAnchor[] {
  const anchors = new Map<string, WorkDocumentCommentAnchor>();
  document.descendants((node, position) => {
    if (!node.isText || !node.text) return;
    const mark = documentCommentMark(node.marks);
    const id = mark ? stringAttribute(mark.attrs.id).trim() : '';
    if (!id) return;
    const current = anchors.get(id);
    if (current) {
      current.from = Math.min(current.from, position);
      current.to = Math.max(current.to, position + node.nodeSize);
      current.anchorText += node.text;
      return;
    }
    anchors.set(id, {
      id,
      from: position,
      to: position + node.nodeSize,
      anchorText: node.text,
    });
  });
  return Array.from(anchors.values()).sort((left, right) => left.from - right.from);
}

export function documentCommentViews(
  comments: readonly WorkDocumentComment[],
  anchors: readonly WorkDocumentCommentAnchor[]
): WorkDocumentCommentView[] {
  const byId = new Map(comments.map((comment) => [comment.id, normalizeDocumentComment(comment)] as const));
  return anchors.map((anchor) => ({
    ...(byId.get(anchor.id) ?? {
      id: anchor.id,
      author: '未知审阅者',
      date: '',
      text: '此批注的内容不可用。',
      resolved: false,
    }),
    ...anchor,
  }));
}

export function retainAnchoredDocumentComments(
  comments: readonly WorkDocumentComment[],
  anchors: readonly WorkDocumentCommentAnchor[]
): WorkDocumentComment[] {
  const ids = new Set(anchors.map((anchor) => anchor.id));
  return comments.filter((comment) => ids.has(comment.id)).map(normalizeDocumentComment);
}

export function stripDocumentCommentsFromSlice(slice: Slice, type: ProseMirrorMark['type']): Slice {
  return new Slice(stripDocumentCommentMarks(slice.content, type), slice.openStart, slice.openEnd);
}

function selectionContainsDocumentComment(document: ProseMirrorNode, from: number, to: number): boolean {
  let found = false;
  document.nodesBetween(from, to, (node) => {
    if (node.isText && documentCommentMark(node.marks)) found = true;
    return !found;
  });
  return found;
}

function documentCommentSegments(
  document: ProseMirrorNode,
  type: ProseMirrorMark['type']
): Array<{ id: string; from: number; to: number }> {
  const segments: Array<{ id: string; from: number; to: number }> = [];
  document.descendants((node, position) => {
    if (!node.isText) return;
    const mark = node.marks.find((candidate) => candidate.type === type);
    const id = mark ? stringAttribute(mark.attrs.id).trim() : '';
    if (id) segments.push({ id, from: position, to: position + node.nodeSize });
  });
  return segments;
}

function stripDocumentCommentMarks(fragment: Fragment, type: ProseMirrorMark['type']): Fragment {
  const nodes: ProseMirrorNode[] = [];
  fragment.forEach((node) => {
    const content = node.content.size ? stripDocumentCommentMarks(node.content, type) : node.content;
    const copy = node.content.size ? node.copy(content) : node;
    nodes.push(copy.mark(copy.marks.filter((mark) => mark.type !== type)));
  });
  return Fragment.fromArray(nodes);
}

function documentCommentMark(marks: readonly ProseMirrorMark[]): ProseMirrorMark | undefined {
  return marks.find((mark) => mark.type.name === 'documentComment');
}

function normalizeDocumentComment(comment: WorkDocumentComment): WorkDocumentComment {
  return {
    id: comment.id,
    author: comment.author || '未知审阅者',
    date: comment.date || '',
    text: comment.text || '（空批注）',
    resolved: Boolean(comment.resolved),
    replies: comment.replies?.map((reply) => ({
      id: reply.id,
      author: reply.author || '未知审阅者',
      date: reply.date || '',
      text: reply.text || '（空回复）',
    })),
  };
}

function stringAttribute(value: unknown): string {
  return typeof value === 'string' ? value : '';
}
