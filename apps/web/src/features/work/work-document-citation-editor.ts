import type { Editor } from '@tiptap/core';
import { DOMParser as ProseMirrorDOMParser, type Node as ProseMirrorNode } from '@tiptap/pm/model';
import {
  documentCitationInstruction,
  renameDocumentCitationTagInInstruction,
  documentCitationTags,
  renderDocumentBibliographyHtml,
  resolveDocumentCitation,
} from './work-document-citations';
import { createWorkId } from './work-templates';
import type { WorkDocumentBibliography, WorkDocumentCitationSource, WorkDocumentContent } from './work-types';

export function insertDocumentCitation(
  editor: Editor,
  source: WorkDocumentCitationSource,
  bibliography: WorkDocumentBibliography
): boolean {
  const citationType = editor.schema.nodes.documentCitation;
  if (!citationType || selectionInsideBibliography(editor)) return false;
  const instruction = documentCitationInstruction([source.tag]);
  const resolved = resolveDocumentCitation([source.tag], bibliography, instruction);
  const transaction = editor.state.tr.replaceSelectionWith(
    citationType.create({
      id: createWorkId('citation'),
      tags: source.tag,
      instruction,
      display: resolved.text,
      orphaned: resolved.orphaned,
    }),
    false
  );
  editor.view.dispatch(transaction.scrollIntoView());
  editor.view.focus();
  return true;
}

export function insertDocumentBibliography(editor: Editor, bibliography: WorkDocumentBibliography): boolean {
  const existing = documentBibliographyNodes(editor.state.doc)[0];
  if (existing) {
    refreshDocumentCitations(editor, { type: 'document', pageSize: 'a4', html: editor.getHTML(), bibliography });
    editor.chain().focus().setNodeSelection(existing.position).run();
    return false;
  }
  const replacement = bibliographyNode(editor, bibliography, createWorkId('bibliography'));
  if (!replacement) return false;
  const transaction = editor.state.tr.replaceSelectionWith(replacement);
  const paragraphType = editor.schema.nodes.paragraph;
  if (paragraphType && transaction.selection.$to.parent.type === replacement.type) {
    transaction.insert(transaction.selection.to, paragraphType.create());
  }
  editor.view.dispatch(transaction.scrollIntoView());
  editor.view.focus();
  return true;
}

export function refreshDocumentCitations(editor: Editor, content: WorkDocumentContent): boolean {
  const bibliography = content.bibliography;
  const transaction = editor.state.tr;
  const bibliographyNodes: Array<{ node: ProseMirrorNode; position: number }> = [];
  editor.state.doc.descendants((node, position) => {
    if (node.type.name === 'documentCitation') {
      const tags = documentCitationTags(typeof node.attrs.tags === 'string' ? node.attrs.tags : '');
      const instruction = typeof node.attrs.instruction === 'string' ? node.attrs.instruction : '';
      const cached = typeof node.attrs.display === 'string' ? node.attrs.display : '';
      const resolved = resolveDocumentCitation(tags, bibliography, instruction, cached);
      if (node.attrs.display !== resolved.text || Boolean(node.attrs.orphaned) !== resolved.orphaned) {
        transaction.setNodeMarkup(position, undefined, {
          ...node.attrs,
          display: resolved.text,
          orphaned: resolved.orphaned,
        });
      }
      return;
    }
    if (node.type.name === 'documentBibliography') bibliographyNodes.push({ node, position });
  });
  if (bibliography) {
    for (const current of bibliographyNodes.reverse()) {
      const id = typeof current.node.attrs.id === 'string' ? current.node.attrs.id : 'document-bibliography-1';
      const replacement = bibliographyNode(editor, bibliography, id);
      if (replacement && !current.node.eq(replacement)) {
        transaction.replaceWith(current.position, current.position + current.node.nodeSize, replacement);
      }
    }
  }
  if (!transaction.docChanged) {
    editor.view.focus();
    return false;
  }
  editor.view.dispatch(transaction);
  editor.view.focus();
  return true;
}

export function documentCitationCount(editor: Editor): number {
  let count = 0;
  editor.state.doc.descendants((node) => {
    if (node.type.name === 'documentCitation') count += 1;
  });
  return count;
}

export function renameDocumentCitationTag(editor: Editor, previousTag: string, nextTag: string): boolean {
  if (!previousTag || previousTag === nextTag) return false;
  const transaction = editor.state.tr;
  editor.state.doc.descendants((node, position) => {
    if (node.type.name !== 'documentCitation') return;
    const tags = documentCitationTags(typeof node.attrs.tags === 'string' ? node.attrs.tags : '');
    if (!tags.includes(previousTag)) return;
    const renamed = tags.map((tag) => (tag === previousTag ? nextTag : tag));
    const instruction =
      typeof node.attrs.instruction === 'string'
        ? renameDocumentCitationTagInInstruction(node.attrs.instruction, previousTag, nextTag)
        : documentCitationInstruction(renamed);
    transaction.setNodeMarkup(position, undefined, {
      ...node.attrs,
      tags: renamed.join(' '),
      instruction,
    });
  });
  if (!transaction.docChanged) return false;
  editor.view.dispatch(transaction);
  return true;
}

function bibliographyNode(editor: Editor, bibliography: WorkDocumentBibliography, id: string): ProseMirrorNode | null {
  const document = new DOMParser().parseFromString(renderDocumentBibliographyHtml(bibliography, id), 'text/html');
  const slice = ProseMirrorDOMParser.fromSchema(editor.schema).parseSlice(document.body);
  return slice.content.firstChild?.type.name === 'documentBibliography' ? slice.content.firstChild : null;
}

function documentBibliographyNodes(document: ProseMirrorNode): Array<{ node: ProseMirrorNode; position: number }> {
  const result: Array<{ node: ProseMirrorNode; position: number }> = [];
  document.descendants((node, position) => {
    if (node.type.name === 'documentBibliography') result.push({ node, position });
  });
  return result;
}

function selectionInsideBibliography(editor: Editor): boolean {
  for (let depth = editor.state.selection.$from.depth; depth > 0; depth -= 1) {
    if (editor.state.selection.$from.node(depth).type.name === 'documentBibliography') return true;
  }
  return false;
}
