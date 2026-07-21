import type { Editor } from '@tiptap/core';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { documentPageDescriptors } from './work-document-pages';
import { syncDocumentContentFromHtml } from './work-document-section';
import {
  documentFieldDisplay,
  documentFieldInstruction,
  type WorkDocumentFieldContext,
  type WorkDocumentFieldKind,
} from './work-document-fields';
import { createWorkId } from './work-templates';
import type { WorkDocumentContent } from './work-types';

export function insertDocumentField(
  editor: Editor,
  kind: WorkDocumentFieldKind,
  content: WorkDocumentContent
): boolean {
  const fieldType = editor.schema.nodes.documentField;
  if (!fieldType) return false;
  const instruction = documentFieldInstruction(kind);
  const transaction = editor.state.tr.replaceSelectionWith(
    fieldType.create({
      id: createWorkId('field'),
      kind,
      instruction,
      display: documentFieldDisplay(kind, fallbackContext(editor), instruction),
    }),
    false
  );
  editor.view.dispatch(transaction.scrollIntoView());
  refreshDocumentFields(editor, content);
  editor.view.focus();
  return true;
}

export function refreshDocumentFields(editor: Editor, content: WorkDocumentContent): boolean {
  const canonical = syncDocumentContentFromHtml(content, editor.getHTML());
  const displays = documentPageDescriptors(canonical).flatMap((page) =>
    page.segments.flatMap((segment) => {
      const document = new DOMParser().parseFromString(segment.html, 'text/html');
      return Array.from(document.body.querySelectorAll<HTMLElement>('[data-document-field]')).map(
        (element) => element.dataset.fieldDisplay?.trim() || element.textContent?.trim() || ''
      );
    })
  );
  let fieldIndex = 0;
  const transaction = editor.state.tr;
  editor.state.doc.descendants((node, position) => {
    if (node.type.name !== 'documentField') return;
    const display = displays[fieldIndex];
    fieldIndex += 1;
    if (!display || node.attrs.display === display) return;
    transaction.setNodeMarkup(position, undefined, { ...node.attrs, display });
  });
  if (!transaction.docChanged) {
    editor.view.focus();
    return false;
  }
  editor.view.dispatch(transaction);
  editor.view.focus();
  return true;
}

function fallbackContext(editor: Editor): WorkDocumentFieldContext {
  let totalPages = 0;
  let sectionNumber = 1;
  let sectionPages = 1;
  let pageNumber = 1;
  let pagesBeforeSelection = 0;
  let foundSelection = false;
  editor.state.doc.forEach((section, offset, index) => {
    if (section.type.name !== 'documentSection') return;
    const pages = countPageBreaks(section) + 1;
    totalPages += pages;
    if (!foundSelection && editor.state.selection.from <= offset + section.nodeSize) {
      sectionNumber = index + 1;
      sectionPages = pages;
      pageNumber = pagesBeforeSelection + 1 + countPageBreaksBefore(section, editor.state.selection.from - offset - 1);
      foundSelection = true;
    }
    if (!foundSelection) pagesBeforeSelection += pages;
  });
  return { pageNumber, totalPages: Math.max(1, totalPages), sectionNumber, sectionPages };
}

function countPageBreaks(node: ProseMirrorNode): number {
  let count = 0;
  node.descendants((child) => {
    if (child.type.name === 'pageBreak') count += 1;
  });
  return count;
}

function countPageBreaksBefore(node: ProseMirrorNode, position: number): number {
  let count = 0;
  node.descendants((child, offset) => {
    if (offset >= position) return false;
    if (child.type.name === 'pageBreak') count += 1;
    return true;
  });
  return count;
}
