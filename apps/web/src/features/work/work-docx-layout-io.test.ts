import JSZip from 'jszip';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createDocxBlob } from './work-docx-export';
import { documentSectionDomAttributes, documentSections } from './work-document-section';
import { exportWorkArtifact, importWorkFile } from './work-file-io';
import { createWorkArtifact } from './work-templates';
import type { WorkDocumentSectionLayout } from './work-types';

describe('Work DOCX layout interoperability', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('round-trips basic page layout, page chrome, numbering, and explicit breaks through DOCX', async () => {
    const artifact = createWorkArtifact('blank-document');
    artifact.title = 'Layout proof';
    artifact.content = {
      type: 'document',
      pageSize: 'letter',
      orientation: 'landscape',
      margins: { top: 20, right: 21, bottom: 22, left: 23 },
      headerText: 'A3S Work',
      footerText: 'Internal',
      showPageNumbers: true,
      pageNumberStart: 4,
      html: '<p>First page</p><div data-page-break="true"></div><p>Second page</p>',
    };

    let exported: Blob | null = null;
    vi.spyOn(URL, 'createObjectURL').mockImplementation((value) => {
      if (value instanceof Blob) exported = value;
      return 'blob:a3s-work-docx-layout';
    });
    vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});

    await exportWorkArtifact(artifact);
    expect(exported).toBeInstanceOf(Blob);
    if (!exported) return;

    const packageArchive = await JSZip.loadAsync(exported);
    const documentXml = await packageArchive.file('word/document.xml')?.async('text');
    expect(documentXml).toMatch(/<w:br[^>]+w:type="page"/);

    const reopened = await importWorkFile(
      new File([exported], 'Layout proof.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      })
    );
    expect(reopened.content.type).toBe('document');
    if (reopened.content.type !== 'document') return;
    expect(reopened.content).toMatchObject({
      pageSize: 'letter',
      orientation: 'landscape',
      margins: { top: 20, right: 21, bottom: 22, left: 23 },
      headerText: 'A3S Work',
      footerText: 'Internal',
      showPageNumbers: true,
      pageNumberStart: 4,
    });
    expect(reopened.content.html).toContain('data-page-break="true"');
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'docx.page-layout')?.message).toContain(
      'explicit page breaks are preserved'
    );
  });

  it('round-trips section boundaries, section page setup, and equal-width columns through DOCX', async () => {
    const artifact = createWorkArtifact('blank-document');
    artifact.title = 'Section proof';
    const first: WorkDocumentSectionLayout = {
      pageSize: 'a4',
      orientation: 'portrait',
      margins: { top: 20, right: 21, bottom: 22, left: 23 },
      columns: { count: 2, spacing: 10, separator: true },
      breakAfter: 'continuous',
      headerText: 'First section',
      footerText: 'First footer',
      showPageNumbers: true,
      pageNumberStart: 3,
    };
    const second: WorkDocumentSectionLayout = {
      pageSize: 'letter',
      orientation: 'landscape',
      margins: { top: 24, right: 25, bottom: 26, left: 27 },
      columns: { count: 3, spacing: 8, separator: false },
      breakAfter: 'oddPage',
      headerText: 'Second section',
      footerText: 'Second footer',
      showPageNumbers: true,
      pageNumberStart: 9,
    };
    artifact.content = {
      type: 'document',
      pageSize: first.pageSize,
      orientation: first.orientation,
      margins: first.margins,
      columns: first.columns,
      headerText: first.headerText,
      footerText: first.footerText,
      showPageNumbers: first.showPageNumbers,
      pageNumberStart: first.pageNumberStart,
      html: `${sectionHtml(first, 'section-one', '<p>Alpha section</p>')}${sectionHtml(
        second,
        'section-two',
        '<p>Beta section</p>'
      )}`,
    };

    let exported: Blob | null = null;
    vi.spyOn(URL, 'createObjectURL').mockImplementation((value) => {
      if (value instanceof Blob) exported = value;
      return 'blob:a3s-work-docx-sections';
    });
    vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});

    await exportWorkArtifact(artifact);
    expect(exported).toBeInstanceOf(Blob);
    if (!exported) return;

    const packageArchive = await JSZip.loadAsync(exported);
    const documentXml = await packageArchive.file('word/document.xml')?.async('text');
    expect(documentXml).toMatch(/<w:type w:val="continuous"/);
    expect(documentXml).toMatch(/<w:type w:val="oddPage"/);
    expect(documentXml).toMatch(/<w:cols[^>]+w:num="2"[^>]+w:sep="true"/);
    expect(documentXml).toMatch(/<w:cols[^>]+w:num="3"/);

    const reopened = await importWorkFile(
      new File([exported], 'Section proof.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      })
    );
    expect(reopened.content.type).toBe('document');
    if (reopened.content.type !== 'document') return;
    const sections = documentSections(reopened.content);
    expect(sections).toHaveLength(2);
    expect(sections.map((section) => section.html)).toEqual([
      expect.stringContaining('Alpha section'),
      expect.stringContaining('Beta section'),
    ]);
    expect(sections[0].layout).toMatchObject({
      pageSize: 'a4',
      orientation: 'portrait',
      columns: { count: 2, spacing: 10, separator: true },
      breakAfter: 'continuous',
      headerText: 'First section',
      footerText: 'First footer',
      showPageNumbers: true,
      pageNumberStart: 3,
    });
    expect(sections[1].layout).toMatchObject({
      pageSize: 'letter',
      orientation: 'landscape',
      columns: { count: 3, spacing: 8, separator: false },
      breakAfter: 'oddPage',
      headerText: 'Second section',
      footerText: 'Second footer',
      showPageNumbers: true,
      pageNumberStart: 9,
    });
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'docx.sections')).toMatchObject({
      severity: 'info',
    });
    expect(reopened.compatibility?.issues.map((issue) => issue.code)).not.toContain('docx.sections.unsupported');
  });

  it('round-trips unequal source column widths and gaps', async () => {
    const artifact = createWorkArtifact('blank-document');
    if (artifact.content.type !== 'document') return;
    artifact.content.columns = { count: 2, spacing: 10, separator: false };
    const archive = await JSZip.loadAsync(await createDocxBlob(artifact.content));
    const documentXml = await archive.file('word/document.xml')?.async('text');
    expect(documentXml).toBeTruthy();
    archive.file(
      'word/document.xml',
      documentXml?.replace(
        /<w:cols[^>]*\/>/,
        '<w:cols w:num="2" w:equalWidth="0"><w:col w:w="3000" w:space="720"/><w:col w:w="6000"/></w:cols>'
      ) ?? ''
    );
    const source = await archive.generateAsync({ type: 'blob' });

    const reopened = await importWorkFile(
      new File([source], 'Unequal columns.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      })
    );
    expect(reopened.content.type).toBe('document');
    if (reopened.content.type !== 'document') return;
    expect(documentSections(reopened.content)[0].layout.columns).toMatchObject({
      count: 2,
      custom: [
        { widthPercent: 33.3, spacing: 12.7 },
        { widthPercent: 66.7, spacing: 0 },
      ],
    });
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'docx.sections')?.message).toContain(
      'custom-width'
    );
    expect(reopened.compatibility?.issues.map((issue) => issue.code)).not.toContain('docx.sections.unsupported');

    const roundTripArchive = await JSZip.loadAsync(await createDocxBlob(reopened.content));
    const roundTripXml = await roundTripArchive.file('word/document.xml')?.async('text');
    expect(roundTripXml).toMatch(/<w:cols[^>]+w:num="2"[^>]+w:equalWidth="false"/);
    expect(roundTripXml).toMatch(/<w:col w:w="\d+" w:space="720"\/>/);
    expect(roundTripXml).toMatch(/<w:col w:w="\d+"\/>/);
  });

  it('reports source sections with more than six columns', async () => {
    const artifact = createWorkArtifact('blank-document');
    if (artifact.content.type !== 'document') return;
    const archive = await JSZip.loadAsync(await createDocxBlob(artifact.content));
    const documentXml = await archive.file('word/document.xml')?.async('text');
    expect(documentXml).toBeTruthy();
    archive.file(
      'word/document.xml',
      documentXml?.replace(/<w:cols[^>]*\/>/, '<w:cols w:num="7" w:equalWidth="1"/>') ?? ''
    );
    const source = await archive.generateAsync({ type: 'blob' });

    const reopened = await importWorkFile(
      new File([source], 'Seven columns.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      })
    );
    expect(
      reopened.compatibility?.issues.find((issue) => issue.code === 'docx.sections.unsupported')?.message
    ).toContain('more than six columns');
  });
});

function sectionHtml(layout: WorkDocumentSectionLayout, id: string, html: string): string {
  const section = document.createElement('section');
  for (const [name, value] of Object.entries(documentSectionDomAttributes(layout, id))) {
    section.setAttribute(name, value);
  }
  section.innerHTML = html;
  return section.outerHTML;
}
