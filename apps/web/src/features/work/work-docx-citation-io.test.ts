import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { createDocumentBibliography, renderDocumentBibliographyHtml } from './work-document-citations';
import { readDocxBibliography } from './work-docx-bibliography';
import { diagnoseDocxCitations } from './work-docx-citation-diagnostics';
import { createDocxBlob } from './work-docx-export';
import { importWorkFile } from './work-file-io';
import { OoxmlPackage, parseXml } from './work-ooxml-package';
import type { WorkDocumentBibliography, WorkDocumentContent } from './work-types';

const BIBLIOGRAPHY_NAMESPACE = 'http://schemas.openxmlformats.org/officeDocument/2006/bibliography';

describe('Work DOCX citation interoperability', () => {
  it('round-trips citation fields, bibliography fields, and Word source custom XML', async () => {
    const bibliography: WorkDocumentBibliography = {
      ...createDocumentBibliography('apa'),
      sources: [
        {
          id: 'source-smith',
          tag: 'Smith2026',
          sourceType: 'JournalArticle',
          title: 'Agent-Native Office Systems',
          year: '2026',
          contributors: {
            Author: {
              people: [
                { first: 'Jane', middle: 'Q.', last: 'Smith' },
                { first: 'Ming', last: 'Li' },
              ],
            },
            Editor: { corporate: 'A3S Lab' },
          },
          journalName: 'Journal of Agentic Software',
          volume: '4',
          issue: '2',
          pages: '12-28',
          url: 'https://a3s.dev/research',
          standardNumber: '10.1000/a3s.2026',
          additionalFields: { Comments: 'Imported metadata' },
        },
      ],
    };
    const content: WorkDocumentContent = {
      type: 'document',
      pageSize: 'a4',
      bibliography,
      html: [
        '<p>Research ',
        '<span data-document-citation="true" data-citation-tags="Smith2026" ',
        'data-citation-instruction="CITATION Smith2026 \\l 2052">(Smith &amp; Li, 2026)</span>',
        ' explains the model.</p>',
        renderDocumentBibliographyHtml(bibliography),
      ].join(''),
    };

    const exported = await createDocxBlob(content);
    const archive = await JSZip.loadAsync(exported);
    const documentXml = await archive.file('word/document.xml')?.async('text');
    const sourcesXml = await archive.file('customXml/item1.xml')?.async('text');
    const propertiesXml = await archive.file('customXml/itemProps1.xml')?.async('text');
    const customRelationships = await archive.file('customXml/_rels/item1.xml.rels')?.async('text');
    const documentRelationships = await archive.file('word/_rels/document.xml.rels')?.async('text');
    const contentTypes = await archive.file('[Content_Types].xml')?.async('text');

    expect(documentXml).toContain('CITATION Smith2026');
    expect(documentXml).toContain('BIBLIOGRAPHY');
    expect(sourcesXml).toContain('<b:Sources');
    expect(sourcesXml).toContain('<Tag>Smith2026</Tag>');
    expect(sourcesXml).toContain('<SourceType>JournalArticle</SourceType>');
    expect(sourcesXml).toContain('<Author><Author><NameList>');
    expect(sourcesXml).toContain('<Last>Smith</Last>');
    expect(sourcesXml).toContain('<Editor><Corporate>A3S Lab</Corporate>');
    expect(sourcesXml).toContain('<Comments>Imported metadata</Comments>');
    expect(propertiesXml).toContain('bibliography');
    expect(customRelationships).toContain('customXmlProps');
    expect(documentRelationships).toContain('relationships/customXml');
    expect(contentTypes).toContain('application/vnd.openxmlformats-officedocument.customXmlProperties+xml');

    const reopened = await importWorkFile(
      new File([exported], 'Citations.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      })
    );
    expect(reopened.content.type).toBe('document');
    if (reopened.content.type !== 'document') return;

    expect(reopened.content.bibliography).toMatchObject({
      style: 'apa',
      sources: [
        {
          tag: 'Smith2026',
          sourceType: 'JournalArticle',
          title: 'Agent-Native Office Systems',
          year: '2026',
          journalName: 'Journal of Agentic Software',
          additionalFields: { Comments: 'Imported metadata' },
          contributors: {
            Author: {
              people: [
                { first: 'Jane', middle: 'Q.', last: 'Smith' },
                { first: 'Ming', last: 'Li' },
              ],
            },
            Editor: { corporate: 'A3S Lab' },
          },
        },
      ],
    });
    expect(reopened.content.html).toContain('data-document-citation');
    expect(reopened.content.html).toContain('data-citation-tags="Smith2026"');
    expect(reopened.content.html).toContain('data-document-bibliography');
    expect(reopened.content.html).toContain('Agent-Native Office Systems');
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'docx.citations')).toMatchObject({
      severity: 'info',
      message: expect.stringContaining('1 citation field'),
    });
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'docx.bibliography')).toMatchObject({
      severity: 'info',
    });
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'docx.fields')).toBeUndefined();
  });

  it('ignores custom XML that only resembles the Word bibliography namespace', async () => {
    const archive = new JSZip();
    archive.file(
      'customXml/item1.xml',
      [
        '<b:Sources xmlns:b="https://example.com/bibliography">',
        '<b:Source><b:Tag>External2026</b:Tag><b:SourceType>Book</b:SourceType>',
        '<b:Title>Unrelated custom XML</b:Title></b:Source>',
        '</b:Sources>',
      ].join('')
    );
    const result = await readDocxBibliography(
      await OoxmlPackage.load(await archive.generateAsync({ type: 'arraybuffer' }))
    );

    expect(result.sourcePartCount).toBe(0);
    expect(result.bibliography).toBeUndefined();
  });

  it('preserves an uncommon Word style path and reports the APA preview fallback', async () => {
    const archive = new JSZip();
    archive.file(
      'customXml/item1.xml',
      [
        `<b:Sources xmlns:b="${BIBLIOGRAPHY_NAMESPACE}" SelectedStyle="\\ISO690.XSL" StyleName="ISO 690">`,
        '<b:Source><b:Tag>Iso2026</b:Tag><b:SourceType>Book</b:SourceType>',
        '<b:Title>Interoperable References</b:Title><b:Year>2026</b:Year></b:Source>',
        '</b:Sources>',
      ].join('')
    );
    const packageArchive = await OoxmlPackage.load(await archive.generateAsync({ type: 'arraybuffer' }));
    const result = await readDocxBibliography(packageArchive);
    const issues = await diagnoseDocxCitations(
      packageArchive,
      parseXml(
        '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>'
      )
    );

    expect(result.bibliography).toMatchObject({
      style: 'apa',
      selectedStyle: '\\ISO690.XSL',
      styleName: 'ISO 690',
    });
    expect(result.uncommonStyle).toBe('ISO 690');
    expect(issues.find((issue) => issue.code === 'docx.citations.style')).toMatchObject({
      severity: 'warning',
      message: expect.stringContaining('ISO 690'),
    });
  });
});
