import { DeletedTextRun, Document, InsertedTextRun, Packer, Paragraph, TextRun } from 'docx';
import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { createWorkDocumentBlob, importWorkDocumentFile } from './work-document-file-io';

describe('Work DOCX tracked-change interoperability', () => {
  it('imports reviewable text revisions and exports them back as native Word revisions', async () => {
    const source = await Packer.toBlob(
      new Document({
        features: { trackRevisions: true },
        sections: [
          {
            children: [
              new Paragraph({
                children: [
                  new TextRun('Keep '),
                  new InsertedTextRun({
                    id: 7,
                    author: 'Alice',
                    date: '2026-07-20T01:02:03.000Z',
                    text: 'inserted',
                  }),
                  new TextRun(' and '),
                  new DeletedTextRun({
                    id: 8,
                    author: 'Bob',
                    date: '2026-07-19T01:02:03.000Z',
                    text: 'deleted',
                  }),
                ],
              }),
            ],
          },
        ],
      })
    );

    const artifact = await importWorkDocumentFile(
      new File([source], 'Reviewed.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      }),
      'docx'
    );

    expect(artifact.content.type).toBe('document');
    if (artifact.content.type !== 'document') return;
    expect(artifact.content.trackChanges).toBe(true);
    expect(artifact.content.html).toContain('data-change-kind="insertion"');
    expect(artifact.content.html).toContain('data-change-author="Alice"');
    expect(artifact.content.html).toContain('>inserted</ins>');
    expect(artifact.content.html).toContain('data-change-kind="deletion"');
    expect(artifact.content.html).toContain('data-change-author="Bob"');
    expect(artifact.content.html).toContain('>deleted</del>');
    expect(artifact.compatibility?.issues.find((issue) => issue.code === 'docx.revisions')).toMatchObject({
      severity: 'info',
      message: expect.stringContaining('remain reviewable in Work'),
    });

    const exported = await createWorkDocumentBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const documentXml = await archive.file('word/document.xml')?.async('text');
    const settingsXml = await archive.file('word/settings.xml')?.async('text');
    expect(documentXml).toContain('<w:ins ');
    expect(documentXml).toContain('w:author="Alice"');
    expect(documentXml).toContain('<w:t xml:space="preserve">inserted</w:t>');
    expect(documentXml).toContain('<w:del ');
    expect(documentXml).toContain('w:author="Bob"');
    expect(documentXml).toContain('<w:delText xml:space="preserve">deleted</w:delText>');
    expect(settingsXml).toContain('<w:trackRevisions/>');

    const reopened = await importWorkDocumentFile(
      new File([exported], 'Reviewed again.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      }),
      'docx'
    );
    expect(reopened.content.type).toBe('document');
    if (reopened.content.type !== 'document') return;
    expect(reopened.content.html).toContain('>inserted</ins>');
    expect(reopened.content.html).toContain('>deleted</del>');
  });
});
