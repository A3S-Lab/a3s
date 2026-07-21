import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { createDocxBlob } from './work-docx-export';
import { importWorkFile } from './work-file-io';
import { analyzeDocxCompatibility } from './work-office-diagnostics';
import type { WorkDocumentContent } from './work-types';

describe('Work DOCX body-field interoperability', () => {
  it('round-trips live page, section, date, and time fields', async () => {
    const content: WorkDocumentContent = {
      type: 'document',
      pageSize: 'a4',
      html: [
        '<p>Page <span data-document-field="true" data-field-kind="page" data-field-instruction="PAGE">2</span>',
        ' of <span data-document-field="true" data-field-kind="numPages" data-field-instruction="NUMPAGES">7</span></p>',
        '<p><span data-document-field="true" data-field-kind="section" data-field-instruction="SECTION">1</span>',
        ' / <span data-document-field="true" data-field-kind="sectionPages" data-field-instruction="SECTIONPAGES">3</span></p>',
        '<p><span data-document-field="true" data-field-kind="date" ',
        'data-field-instruction=\'DATE \\@ "yyyy-MM-dd"\'>2026-07-21</span> ',
        '<span data-document-field="true" data-field-kind="time" ',
        'data-field-instruction=\'TIME \\@ "HH:mm"\'>14:05</span></p>',
      ].join(''),
    };

    const exported = await createDocxBlob(content);
    const archive = await JSZip.loadAsync(exported);
    const documentXml = await archive.file('word/document.xml')?.async('text');
    const settingsXml = await archive.file('word/settings.xml')?.async('text');

    expect(documentXml).toContain('w:instr="PAGE"');
    expect(documentXml).toContain('w:instr="NUMPAGES"');
    expect(documentXml).toContain('w:instr="SECTION"');
    expect(documentXml).toContain('w:instr="SECTIONPAGES"');
    expect(documentXml).toContain('DATE \\@ &quot;yyyy-MM-dd&quot;');
    expect(documentXml).toContain('TIME \\@ &quot;HH:mm&quot;');
    expect(settingsXml).toContain('<w:updateFields');

    const reopened = await importWorkFile(
      new File([exported], 'Body fields.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      })
    );
    expect(reopened.content.type).toBe('document');
    if (reopened.content.type !== 'document') return;
    const document = new DOMParser().parseFromString(reopened.content.html, 'text/html');
    const fields = document.querySelectorAll<HTMLElement>('[data-document-field]');

    expect(fields).toHaveLength(6);
    expect(Array.from(fields, (field) => field.dataset.fieldKind)).toEqual([
      'page',
      'numPages',
      'section',
      'sectionPages',
      'date',
      'time',
    ]);
    expect(fields[0].textContent).toBe('2');
    expect(fields[4].dataset.fieldInstruction).toContain('yyyy-MM-dd');
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'docx.fields.body')).toMatchObject({
      severity: 'info',
    });
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'docx.fields')).toBeUndefined();
  });

  it('imports a complex body field without consuming adjacent text', async () => {
    const exported = await createDocxBlob({
      type: 'document',
      pageSize: 'a4',
      html: '<p>Body fields</p>',
    });
    const archive = await JSZip.loadAsync(exported);
    const documentXml = await archive.file('word/document.xml')?.async('text');
    expect(documentXml).toBeTruthy();
    if (!documentXml) return;
    archive.file(
      'word/document.xml',
      documentXml.replace(
        '</w:body>',
        [
          '<w:p><w:r><w:t xml:space="preserve">Before </w:t></w:r>',
          '<w:r><w:fldChar w:fldCharType="begin"/></w:r>',
          '<w:r><w:instrText xml:space="preserve"> SECTIONPAGES </w:instrText></w:r>',
          '<w:r><w:fldChar w:fldCharType="separate"/></w:r>',
          '<w:r><w:t>4</w:t></w:r>',
          '<w:r><w:fldChar w:fldCharType="end"/></w:r>',
          '<w:r><w:t xml:space="preserve"> after</w:t></w:r></w:p>',
          '</w:body>',
        ].join('')
      )
    );
    const source = await archive.generateAsync({ type: 'blob' });
    const reopened = await importWorkFile(
      new File([source], 'Complex field.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      })
    );
    expect(reopened.content.type).toBe('document');
    if (reopened.content.type !== 'document') return;

    expect(reopened.content.html).toContain('Before ');
    expect(reopened.content.html).toContain(' after');
    expect(reopened.content.html).toContain('data-field-kind="sectionPages"');
    expect(reopened.content.html).toContain('data-field-display="4"');
  });

  it('diagnoses citation fields whose bibliography source is missing', async () => {
    const exported = await createDocxBlob({
      type: 'document',
      pageSize: 'a4',
      html: '<p>Unsupported citation</p>',
    });
    const archive = await JSZip.loadAsync(exported);
    const documentXml = await archive.file('word/document.xml')?.async('text');
    expect(documentXml).toBeTruthy();
    if (!documentXml) return;
    archive.file(
      'word/document.xml',
      documentXml.replace(
        '</w:body>',
        '<w:p><w:fldSimple w:instr=" CITATION SourceOne "><w:r><w:t>(A3S, 2026)</w:t></w:r></w:fldSimple></w:p></w:body>'
      )
    );
    const source = await archive.generateAsync({ type: 'blob' });
    const report = await analyzeDocxCompatibility(
      new File([source], 'Citation.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      }),
      []
    );

    expect(report.issues.find((issue) => issue.code === 'docx.citations.missing-source')).toMatchObject({
      severity: 'warning',
      message: expect.stringContaining('SourceOne'),
    });
    expect(report.issues.find((issue) => issue.code === 'docx.fields')).toBeUndefined();
  });
});
