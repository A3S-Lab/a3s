import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import { afterEach, describe, expect, it } from 'vitest';
import type { WorkDocumentComment } from './work-types';
import {
  collectDocumentCommentAnchors,
  DocumentComment,
  documentCommentViews,
  insertDocumentComment,
  removeDocumentComment,
  retainAnchoredDocumentComments,
} from './work-document-comments';

const editors: Editor[] = [];

describe('Work document comments', () => {
  afterEach(() => {
    for (const editor of editors.splice(0)) editor.destroy();
  });

  it('anchors a comment to selected text and prevents overlapping comment ranges', () => {
    const editor = createEditor('<p>Alpha beta gamma</p>');
    editor.commands.setTextSelection({ from: 1, to: 6 });

    expect(insertDocumentComment(editor, 'comment-1')).toBe(true);
    expect(editor.getHTML()).toContain('data-document-comment="true"');
    expect(editor.getHTML()).toContain('data-comment-id="comment-1"');
    expect(collectDocumentCommentAnchors(editor.state.doc)).toEqual([
      { id: 'comment-1', from: 1, to: 6, anchorText: 'Alpha' },
    ]);

    editor.commands.setTextSelection({ from: 2, to: 4 });
    expect(insertDocumentComment(editor, 'comment-2')).toBe(false);
    expect(collectDocumentCommentAnchors(editor.state.doc)).toHaveLength(1);
  });

  it('joins stored thread metadata to anchors and removes orphaned comments', () => {
    const editor = createEditor(
      '<p><span data-document-comment="true" data-comment-id="comment-1">Review this</span> later</p>'
    );
    const comments: WorkDocumentComment[] = [
      {
        id: 'comment-1',
        author: 'Alice',
        date: '2026-07-20T00:00:00.000Z',
        text: 'Please verify this claim.',
        resolved: false,
        replies: [
          {
            id: 'reply-1',
            author: 'Bob',
            date: '2026-07-20T01:00:00.000Z',
            text: 'Verified against the source.',
          },
        ],
      },
      {
        id: 'orphan',
        author: 'Alice',
        date: '',
        text: 'No longer anchored',
        resolved: false,
      },
    ];
    const anchors = collectDocumentCommentAnchors(editor.state.doc);

    expect(documentCommentViews(comments, anchors)).toMatchObject([
      {
        id: 'comment-1',
        anchorText: 'Review this',
        text: 'Please verify this claim.',
        replies: [{ id: 'reply-1', text: 'Verified against the source.' }],
      },
    ]);
    expect(retainAnchoredDocumentComments(comments, anchors).map((comment) => comment.id)).toEqual(['comment-1']);
  });

  it('removes every marked segment for a comment without deleting its text', () => {
    const editor = createEditor(
      '<p><span data-document-comment="true" data-comment-id="comment-1">First</span> and ' +
        '<span data-document-comment="true" data-comment-id="comment-1">second</span></p>'
    );

    expect(removeDocumentComment(editor, 'comment-1')).toBe(true);
    expect(editor.getText()).toBe('First and second');
    expect(editor.getHTML()).not.toContain('data-document-comment');
  });

  it('strips copied comment anchors before paste without removing other formatting', () => {
    const editor = createEditor(
      '<p><strong><span data-document-comment="true" data-comment-id="comment-1">Reviewed</span></strong></p>'
    );
    const transformPasted = editor.view.someProp('transformPasted');
    const pasted = transformPasted?.(editor.state.doc.slice(1, 9), editor.view, false);
    let commentMarks = 0;
    let strongMarks = 0;
    pasted?.content.descendants((node) => {
      commentMarks += node.marks.filter((mark) => mark.type.name === 'documentComment').length;
      strongMarks += node.marks.filter((mark) => mark.type.name === 'bold').length;
    });

    expect(pasted?.content.textBetween(0, pasted.content.size)).toBe('Reviewed');
    expect(commentMarks).toBe(0);
    expect(strongMarks).toBeGreaterThan(0);
  });
});

function createEditor(content: string): Editor {
  const editor = new Editor({
    extensions: [StarterKit, DocumentComment],
    content,
  });
  editors.push(editor);
  return editor;
}
