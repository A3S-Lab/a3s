import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { importWorkDocumentFile, createWorkDocumentBlob } from './work-document-file-io';
import { attribute, descendants, OoxmlPackage } from './work-ooxml-package';
import { analyzeDocxCompatibility } from './work-office-diagnostics';
import { createWorkArtifact } from './work-templates';

describe('Work DOCX comment interoperability', () => {
  it('round-trips anchored comments, replies, authors, dates, and resolved state', async () => {
    const artifact = createWorkArtifact('blank-document');
    artifact.title = 'Comment review';
    artifact.content = {
      type: 'document',
      pageSize: 'a4',
      html: [
        '<p>Review ',
        '<span data-document-comment="true" data-comment-id="comment-one">this statement</span>',
        ' before publishing.</p>',
      ].join(''),
      comments: [
        {
          id: 'comment-one',
          author: 'Alice',
          date: '2026-07-20T00:00:00.000Z',
          text: 'Please verify the source.',
          resolved: true,
          replies: [
            {
              id: 'reply-one',
              author: 'Bob',
              date: '2026-07-20T01:00:00.000Z',
              text: 'Verified against the original report.',
            },
          ],
        },
      ],
    };

    const exported = await createWorkDocumentBlob(artifact);
    const archive = await OoxmlPackage.load(await exported.arrayBuffer());
    const document = await archive.xml('word/document.xml');
    const comments = await archive.xml('word/comments.xml');

    expect(descendants(document, 'commentRangeStart')).toHaveLength(1);
    expect(descendants(document, 'commentRangeEnd')).toHaveLength(1);
    expect(descendants(document, 'commentReference')).toHaveLength(1);
    expect(descendants(comments, 'comment').map((comment) => comment.textContent)).toEqual([
      'Please verify the source.',
      'Verified against the original report.',
    ]);
    expect(descendants(comments, 'comment').map((comment) => attribute(comment, 'author'))).toEqual(['Alice', 'Bob']);
    expect(archive.has('word/commentsExtended.xml')).toBe(true);

    const reopened = await importWorkDocumentFile(
      new File([exported], 'Comment review.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      }),
      'docx'
    );
    expect(reopened.content.type).toBe('document');
    if (reopened.content.type !== 'document') throw new Error('Expected document content');
    expect(reopened.content.html).toContain('data-document-comment="true"');
    expect(reopened.content.html).toContain('data-comment-id="docx-comment-0"');
    expect(reopened.content.comments).toMatchObject([
      {
        id: 'docx-comment-0',
        author: 'Alice',
        text: 'Please verify the source.',
        resolved: true,
        replies: [
          {
            author: 'Bob',
            text: 'Verified against the original report.',
          },
        ],
      },
    ]);
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'docx.comments')).toMatchObject({
      severity: 'info',
    });
  });

  it('preserves one comment range that spans multiple paragraphs', async () => {
    const artifact = createWorkArtifact('blank-document');
    artifact.content = {
      type: 'document',
      pageSize: 'a4',
      html: [
        '<p><span data-document-comment="true" data-comment-id="cross-paragraph">First paragraph</span></p>',
        '<p><span data-document-comment="true" data-comment-id="cross-paragraph">Second paragraph</span></p>',
      ].join(''),
      comments: [
        {
          id: 'cross-paragraph',
          author: 'Alice',
          date: '2026-07-20T00:00:00.000Z',
          text: 'Review both paragraphs together.',
          resolved: false,
        },
      ],
    };

    const exported = await createWorkDocumentBlob(artifact);
    const archive = await OoxmlPackage.load(await exported.arrayBuffer());
    const document = await archive.xml('word/document.xml');
    expect(descendants(document, 'commentRangeStart')).toHaveLength(1);
    expect(descendants(document, 'commentRangeEnd')).toHaveLength(1);

    const reopened = await importWorkDocumentFile(
      new File([exported], 'Cross paragraph comment.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      }),
      'docx'
    );
    if (reopened.content.type !== 'document') throw new Error('Expected document content');
    expect(reopened.content.html.match(/data-comment-id="docx-comment-0"/g)).toHaveLength(2);
    expect(reopened.content.html).toContain('First paragraph');
    expect(reopened.content.html).toContain('Second paragraph');
    expect(reopened.content.comments?.[0].text).toBe('Review both paragraphs together.');
  });

  it('diagnoses rich comment bodies without warning about an empty comments part', async () => {
    const artifact = createWorkArtifact('blank-document');
    artifact.content = {
      type: 'document',
      pageSize: 'a4',
      html: '<p><span data-document-comment="true" data-comment-id="diagnostic-comment">Review</span></p>',
      comments: [
        {
          id: 'diagnostic-comment',
          author: 'Alice',
          date: '2026-07-20T00:00:00.000Z',
          text: 'Keep this bold.',
          resolved: false,
        },
      ],
    };
    const archive = await JSZip.loadAsync(await createWorkDocumentBlob(artifact));
    const commentsPath = 'word/comments.xml';
    const commentsXml = await archive.file(commentsPath)?.async('text');
    if (!commentsXml) throw new Error('Expected DOCX comments part');
    archive.file(commentsPath, commentsXml.replace('<w:r>', '<w:r><w:rPr><w:b/></w:rPr>'));
    const richSource = await archive.generateAsync({ type: 'blob' });
    const richReport = await analyzeDocxCompatibility(
      new File([richSource], 'Rich comment.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      }),
      []
    );
    expect(richReport.issues.find((issue) => issue.code === 'docx.comments')).toMatchObject({ severity: 'info' });
    expect(richReport.issues.find((issue) => issue.code === 'docx.comments.formatting')).toMatchObject({
      severity: 'warning',
    });

    archive.file(
      commentsPath,
      [
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
        '<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>',
      ].join('')
    );
    const emptySource = await archive.generateAsync({ type: 'blob' });
    const emptyReport = await analyzeDocxCompatibility(
      new File([emptySource], 'Empty comments.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      }),
      []
    );
    expect(emptyReport.issues.some((issue) => issue.code.startsWith('docx.comments'))).toBe(false);
  });
});
