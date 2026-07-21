import type { Editor } from '@tiptap/core';
import { TextSelection } from '@tiptap/pm/state';
import { activeDocumentSection } from './work-document-section-editor';
import {
  documentCaptionDisplay,
  documentCaptionKind,
  type WorkDocumentCaptionKind,
  type WorkDocumentCaptionTarget,
} from './work-document-captions';
import { createWorkId } from './work-templates';

export function insertDocumentCaption(editor: Editor, kind: WorkDocumentCaptionKind, title: string): boolean {
  const section = activeDocumentSection(editor);
  const captionType = editor.schema.nodes.documentCaption;
  const paragraphType = editor.schema.nodes.paragraph;
  if (!section || !captionType || !paragraphType || selectionInsideCaption(editor)) return false;

  const id = createWorkId(`caption-${kind}`);
  const number = nextCaptionNumber(editor, kind);
  const caption = captionType.create({ id, kind, number }, title.trim() ? editor.schema.text(title.trim()) : undefined);
  const child = activeSectionChild(section, editor.state.selection.from);
  if (!child) return false;
  const insertPosition = section.position + 1 + child.offset + child.nodeSize;
  const transaction = editor.state.tr.insert(insertPosition, caption);
  let selectionPosition = insertPosition + caption.nodeSize;
  if (child.index === section.node.childCount - 1) {
    transaction.insert(selectionPosition, paragraphType.create());
    selectionPosition += 1;
  }
  transaction.setSelection(TextSelection.near(transaction.doc.resolve(selectionPosition)));
  editor.view.dispatch(transaction.scrollIntoView());
  editor.view.focus();
  return true;
}

export function insertDocumentCrossReference(editor: Editor, target: WorkDocumentCaptionTarget): boolean {
  const referenceType = editor.schema.nodes.documentCrossReference;
  if (!referenceType) return false;
  const transaction = editor.state.tr.replaceSelectionWith(
    referenceType.create({
      targetId: target.id,
      kind: target.kind,
      number: target.number,
      orphaned: false,
    }),
    false
  );
  editor.view.dispatch(transaction.scrollIntoView());
  editor.view.focus();
  return true;
}

export function editorDocumentCaptionTargets(editor: Editor): WorkDocumentCaptionTarget[] {
  const targets: WorkDocumentCaptionTarget[] = [];
  editor.state.doc.descendants((node) => {
    if (node.type.name !== 'documentCaption') return;
    const id = typeof node.attrs.id === 'string' ? node.attrs.id.trim() : '';
    const kind = documentCaptionKind(node.attrs.kind);
    if (!id || !kind) return;
    const number = positiveInteger(node.attrs.number);
    targets.push({
      id,
      kind,
      number,
      label: kind === 'figure' ? '图' : '表',
      title: node.textContent.trim(),
      display: documentCaptionDisplay(kind, number),
    });
  });
  return targets;
}

function nextCaptionNumber(editor: Editor, kind: WorkDocumentCaptionKind): number {
  return editorDocumentCaptionTargets(editor).filter((caption) => caption.kind === kind).length + 1;
}

function activeSectionChild(
  section: NonNullable<ReturnType<typeof activeDocumentSection>>,
  selectionPosition: number
): { index: number; offset: number; nodeSize: number } | null {
  const relativePosition = Math.max(0, selectionPosition - section.position - 1);
  let active: { index: number; offset: number; nodeSize: number } | null = null;
  section.node.forEach((node, offset, index) => {
    if (relativePosition >= offset) active = { index, offset, nodeSize: node.nodeSize };
  });
  return active;
}

function selectionInsideCaption(editor: Editor): boolean {
  for (let depth = editor.state.selection.$from.depth; depth > 0; depth -= 1) {
    if (editor.state.selection.$from.node(depth).type.name === 'documentCaption') return true;
  }
  return false;
}

function positiveInteger(value: unknown): number {
  const number = Number(value);
  return Number.isSafeInteger(number) && number > 0 ? number : 1;
}
