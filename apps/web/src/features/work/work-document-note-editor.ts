import type { Editor } from '@tiptap/core';
import { TextSelection } from '@tiptap/pm/state';
import { activeDocumentSection } from './work-document-section-editor';
import { documentNoteKey, type WorkDocumentNoteKind } from './work-document-notes';
import { createWorkId } from './work-templates';

export function insertDocumentNote(editor: Editor, kind: WorkDocumentNoteKind): boolean {
  const section = activeDocumentSection(editor);
  const referenceType = editor.schema.nodes.documentNoteReference;
  const noteType = editor.schema.nodes.documentNote;
  const paragraphType = editor.schema.nodes.paragraph;
  if (!section || !referenceType || !noteType || !paragraphType || selectionInsideNote(editor)) return false;

  const number = nextNoteNumber(editor, kind);
  const id = createWorkId(kind);
  const attributes = { id, kind, number };
  const transaction = editor.state.tr.replaceSelectionWith(referenceType.create(attributes), false);
  const updatedSection = transaction.doc.nodeAt(section.position);
  if (!updatedSection || updatedSection.type.name !== 'documentSection') return false;

  const insertPosition = section.position + updatedSection.nodeSize - 1;
  transaction.insert(insertPosition, noteType.create(attributes, paragraphType.create()));
  transaction.setSelection(TextSelection.near(transaction.doc.resolve(insertPosition + 2)));
  editor.view.dispatch(transaction.scrollIntoView());
  editor.view.focus();
  return true;
}

function nextNoteNumber(editor: Editor, kind: WorkDocumentNoteKind): number {
  const ids = new Set<string>();
  editor.state.doc.descendants((node) => {
    if (node.type.name !== 'documentNoteReference' || node.attrs.kind !== kind) return;
    const id = typeof node.attrs.id === 'string' ? node.attrs.id : '';
    if (id) ids.add(documentNoteKey(kind, id));
  });
  return ids.size + 1;
}

function selectionInsideNote(editor: Editor): boolean {
  for (let depth = editor.state.selection.$from.depth; depth > 0; depth -= 1) {
    if (editor.state.selection.$from.node(depth).type.name === 'documentNote') return true;
  }
  return false;
}
