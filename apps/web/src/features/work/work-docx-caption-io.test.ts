import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { createDocxBlob } from './work-docx-export';
import { docxFieldOccurrences } from './work-docx-field-instructions';
import { importWorkFile } from './work-file-io';
import { attribute, descendants, parseXml } from './work-ooxml-package';
import { analyzeDocxCompatibility } from './work-office-diagnostics';
import type { WorkDocumentContent } from './work-types';

const WORD_NAMESPACE = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main';

describe('Work DOCX caption interoperability', () => {
  it('round-trips figure/table captions, bookmarks, and REF fields', async () => {
    const content: WorkDocumentContent = {
      type: 'document',
      pageSize: 'a4',
      html: [
        '<p>Caption interoperability</p>',
        '<figcaption data-document-caption="true" data-caption-kind="figure" data-caption-id="architecture">',
        '<strong>System architecture</strong>',
        '</figcaption>',
        '<p>See <span data-document-cross-reference="true" data-reference-target-id="architecture">图 1</span>.</p>',
        '<figcaption data-document-caption="true" data-caption-kind="table" data-caption-id="metrics">',
        'Runtime metrics',
        '</figcaption>',
      ].join(''),
    };

    const exported = await createDocxBlob(content);
    const archive = await JSZip.loadAsync(exported);
    const documentXml = await archive.file('word/document.xml')?.async('text');
    const settingsXml = await archive.file('word/settings.xml')?.async('text');

    expect(documentXml).toContain('SEQ Figure');
    expect(documentXml).toContain('SEQ Table');
    expect(documentXml).toContain('REF A3SCaption_');
    expect(documentXml).toContain('<w:bookmarkStart');
    expect(settingsXml).toContain('<w:updateFields');

    const reopened = await importWorkFile(
      new File([exported], 'Captions.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      })
    );
    expect(reopened.content.type).toBe('document');
    if (reopened.content.type !== 'document') return;
    expect(reopened.content.html).toContain('data-document-caption');
    expect(reopened.content.html).toContain('data-caption-kind="figure"');
    expect(reopened.content.html).toContain('<strong>System architecture</strong>');
    expect(reopened.content.html).toContain('data-document-cross-reference');
    expect(reopened.content.html).toContain('图 1');
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'docx.captions')).toMatchObject({
      severity: 'info',
    });
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'docx.fields')).toBeUndefined();
  });

  it('keeps unrelated fields in compatibility warnings', async () => {
    const exported = await createDocxBlob({
      type: 'document',
      pageSize: 'a4',
      html: [
        '<figcaption data-document-caption="true" data-caption-kind="figure" data-caption-id="figure-one">',
        'Architecture',
        '</figcaption>',
      ].join(''),
    });
    const archive = await JSZip.loadAsync(exported);
    const documentXml = await archive.file('word/document.xml')?.async('text');
    expect(documentXml).toBeTruthy();
    if (!documentXml) return;
    archive.file(
      'word/document.xml',
      documentXml.replace(
        '</w:body>',
        '<w:p><w:fldSimple w:instr=" AUTOTEXT Example "><w:r><w:t>Example</w:t></w:r></w:fldSimple></w:p></w:body>'
      )
    );
    const source = await archive.generateAsync({ type: 'blob' });
    const report = await analyzeDocxCompatibility(
      new File([source], 'Captions and fields.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      }),
      []
    );

    expect(report.issues.find((issue) => issue.code === 'docx.captions')).toMatchObject({ severity: 'info' });
    expect(report.issues.find((issue) => issue.code === 'docx.fields')).toMatchObject({ severity: 'warning' });
  });

  it('selects the bookmark enclosing a caption field when other bookmarks share its paragraph', async () => {
    const exported = await createDocxBlob({
      type: 'document',
      pageSize: 'a4',
      html: [
        '<figcaption data-document-caption="true" data-caption-kind="figure" data-caption-id="figure-one">',
        'Architecture',
        '</figcaption>',
        '<p><span data-document-cross-reference="true" data-reference-target-id="figure-one">图 1</span></p>',
      ].join(''),
    });
    const archive = await JSZip.loadAsync(exported);
    const documentXml = await archive.file('word/document.xml')?.async('text');
    expect(documentXml).toBeTruthy();
    if (!documentXml) return;
    const document = parseXml(documentXml);
    const captionBookmark = descendants(document, 'bookmarkStart').find((bookmark) =>
      attribute(bookmark, 'name')?.startsWith('A3SCaption_')
    );
    const referenceField = docxFieldOccurrences(document).find((field) => /^REF\s/i.test(field.instruction));
    expect(captionBookmark).toBeTruthy();
    expect(referenceField).toBeTruthy();
    if (!captionBookmark || !referenceField) return;
    const unrelatedStart = document.createElementNS(WORD_NAMESPACE, 'w:bookmarkStart');
    unrelatedStart.setAttributeNS(WORD_NAMESPACE, 'w:id', '900');
    unrelatedStart.setAttributeNS(WORD_NAMESPACE, 'w:name', 'Unrelated');
    const unrelatedEnd = document.createElementNS(WORD_NAMESPACE, 'w:bookmarkEnd');
    unrelatedEnd.setAttributeNS(WORD_NAMESPACE, 'w:id', '900');
    captionBookmark.before(unrelatedStart, unrelatedEnd);
    const trailingText = document.createElementNS(WORD_NAMESPACE, 'w:t');
    trailingText.textContent = ' tail';
    referenceField.end.parentNode?.append(trailingText);
    archive.file('word/document.xml', new XMLSerializer().serializeToString(document));
    const source = await archive.generateAsync({ type: 'blob' });
    const reopened = await importWorkFile(
      new File([source], 'Caption bookmarks.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      })
    );
    expect(reopened.content.type).toBe('document');
    if (reopened.content.type !== 'document') return;

    expect(reopened.content.html).not.toContain('docx-caption-Unrelated');
    expect(reopened.content.html).toContain('data-document-cross-reference');
    expect(reopened.content.html).toContain('图 1');
    expect(reopened.content.html).toContain('tail');
  });

  it('imports simple caption REF fields without consuming adjacent text', async () => {
    const exported = await createDocxBlob({
      type: 'document',
      pageSize: 'a4',
      html: [
        '<figcaption data-document-caption="true" data-caption-kind="figure" data-caption-id="figure-one">',
        'Architecture',
        '</figcaption>',
      ].join(''),
    });
    const archive = await JSZip.loadAsync(exported);
    const documentXml = await archive.file('word/document.xml')?.async('text');
    const bookmarkName = documentXml
      ? /<w:bookmarkStart[^>]+w:name="(A3SCaption_[^"]+)"/.exec(documentXml)?.[1]
      : undefined;
    expect(bookmarkName).toBeTruthy();
    if (!documentXml || !bookmarkName) return;
    archive.file(
      'word/document.xml',
      documentXml.replace(
        '</w:body>',
        [
          '<w:p><w:r><w:t xml:space="preserve">See </w:t></w:r>',
          `<w:fldSimple w:instr=" REF ${bookmarkName} \\h ">`,
          '<w:r><w:t>1</w:t></w:r></w:fldSimple>',
          '<w:r><w:t xml:space="preserve"> tail</w:t></w:r></w:p>',
          '</w:body>',
        ].join('')
      )
    );
    const source = await archive.generateAsync({ type: 'blob' });
    const reopened = await importWorkFile(
      new File([source], 'Simple caption reference.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      })
    );
    expect(reopened.content.type).toBe('document');
    if (reopened.content.type !== 'document') return;

    expect(reopened.content.html).toContain('data-document-cross-reference');
    expect(reopened.content.html).toContain('See ');
    expect(reopened.content.html).toContain(' tail');
  });
});
