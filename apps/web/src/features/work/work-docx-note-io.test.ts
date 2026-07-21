import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { createDocxBlob } from './work-docx-export';
import { importWorkFile } from './work-file-io';
import type { WorkDocumentContent } from './work-types';

describe('Work DOCX note interoperability', () => {
  it('round-trips editable footnotes and endnotes as native DOCX note parts', async () => {
    const content: WorkDocumentContent = {
      type: 'document',
      pageSize: 'a4',
      html: [
        '<p>Body with a footnote',
        '<sup data-document-note-reference="true" data-note-kind="footnote" data-note-id="foot-one" data-note-number="1">1</sup>',
        ' and an endnote',
        '<sup data-document-note-reference="true" data-note-kind="endnote" data-note-id="end-one" data-note-number="1">1</sup>',
        '.</p>',
        '<aside data-document-note="true" data-note-kind="footnote" data-note-id="foot-one" data-note-number="1">',
        '<p><strong>Footnote text</strong> with detail.</p>',
        '</aside>',
        '<aside data-document-note="true" data-note-kind="endnote" data-note-id="end-one" data-note-number="1">',
        '<p><em>Endnote text</em> with context.</p>',
        '</aside>',
      ].join(''),
    };

    const exported = await createDocxBlob(content);
    const archive = await JSZip.loadAsync(exported);
    const documentXml = await archive.file('word/document.xml')?.async('text');
    const footnotesXml = await archive.file('word/footnotes.xml')?.async('text');
    const endnotesXml = await archive.file('word/endnotes.xml')?.async('text');

    expect(documentXml).toMatch(/<w:footnoteReference w:id="1"/);
    expect(documentXml).toMatch(/<w:endnoteReference w:id="1"/);
    expect(footnotesXml).toContain('Footnote text');
    expect(footnotesXml).toContain('<w:b');
    expect(endnotesXml).toContain('Endnote text');
    expect(endnotesXml).toContain('<w:i');

    const reopened = await importWorkFile(
      new File([exported], 'Notes.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      })
    );
    expect(reopened.content.type).toBe('document');
    if (reopened.content.type !== 'document') return;

    const html = new DOMParser().parseFromString(reopened.content.html, 'text/html');
    const references = html.querySelectorAll('[data-document-note-reference]');
    const notes = html.querySelectorAll('[data-document-note]');
    expect(references).toHaveLength(2);
    expect(notes).toHaveLength(2);
    expect(html.querySelector('[data-note-kind="footnote"][data-document-note]')?.textContent?.trim()).toBe(
      'Footnote text with detail.'
    );
    expect(html.querySelector('[data-note-kind="endnote"][data-document-note]')?.textContent?.trim()).toBe(
      'Endnote text with context.'
    );
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'docx.notes')).toMatchObject({
      severity: 'info',
    });
  });
});
