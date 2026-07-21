import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { createDocxBlob } from './work-docx-export';
import { documentSections } from './work-document-section';
import { importWorkFile } from './work-file-io';
import { analyzeDocxCompatibility } from './work-office-diagnostics';
import type { WorkDocumentContent, WorkDocumentPageChrome } from './work-types';

describe('Work DOCX rich page chrome interoperability', () => {
  it('round-trips rich default, first-page, and even-page headers and footers', async () => {
    const pageChrome: WorkDocumentPageChrome = {
      differentFirstPage: true,
      differentOddEvenPages: true,
      default: {
        headerHtml: '<p style="text-align: center"><strong>Default header</strong></p>',
        footerHtml: '<p><em>Default footer</em></p>',
        showPageNumber: true,
      },
      first: {
        headerHtml: '<p style="text-align: right"><u>First header</u></p>',
        footerHtml: '<p>First footer</p>',
        showPageNumber: false,
      },
      even: {
        headerHtml: '<p><span style="color: #c2410c"><a href="https://a3s.dev">Even header</a></span></p>',
        footerHtml: '<p>Even footer</p>',
        showPageNumber: true,
      },
    };
    const content: WorkDocumentContent = {
      type: 'document',
      pageSize: 'a4',
      pageChrome,
      html: '<p>Document body</p>',
    };

    const exported = await createDocxBlob(content);
    const archive = await JSZip.loadAsync(exported);
    const documentXml = await archive.file('word/document.xml')?.async('text');
    const settingsXml = await archive.file('word/settings.xml')?.async('text');
    const headerParts = await Promise.all(
      Object.keys(archive.files)
        .filter((path) => /^word\/header\d+\.xml$/.test(path))
        .map(async (path) => [path, (await archive.file(path)?.async('text')) ?? ''] as const)
    );
    const footerParts = await Promise.all(
      Object.keys(archive.files)
        .filter((path) => /^word\/footer\d+\.xml$/.test(path))
        .map(async (path) => [path, (await archive.file(path)?.async('text')) ?? ''] as const)
    );
    const headers = headerParts.map(([, xml]) => xml).join('');
    const footers = footerParts.map(([, xml]) => xml).join('');

    expect(documentXml).toMatch(/<w:headerReference w:type="default"/);
    expect(documentXml).toMatch(/<w:headerReference w:type="first"/);
    expect(documentXml).toMatch(/<w:headerReference w:type="even"/);
    expect(documentXml).toContain('<w:titlePg');
    expect(settingsXml).toContain('<w:evenAndOddHeaders');
    expect(headers).toContain('Default header');
    expect(headers).toContain('First header');
    expect(headers).toContain('Even header');
    expect(headers).toContain('<w:b');
    expect(headers).toContain('<w:u');
    expect(headers).toContain('<w:jc w:val="center"');
    expect(headers).toContain('<w:jc w:val="right"');
    expect(headers).toContain('<w:color w:val="C2410C"');
    expect(footers).toContain('Default footer');
    expect(footers).toContain('First footer');
    expect(footers).toContain('Even footer');
    expect(footers).toContain('PAGE');

    const defaultFooter = footerParts.find(([, xml]) => xml.includes('Default footer'));
    expect(defaultFooter).toBeTruthy();
    if (!defaultFooter) return;
    archive.file(
      defaultFooter[0],
      defaultFooter[1].replace(
        '</w:ftr>',
        [
          '<w:p><w:r>',
          '<w:fldChar w:fldCharType="begin"/>',
          '<w:instrText xml:space="preserve"> PAGE </w:instrText>',
          '<w:fldChar w:fldCharType="separate"/>',
          '<w:t>42</w:t>',
          '<w:fldChar w:fldCharType="end"/>',
          '<w:t xml:space="preserve"> / Confidential</w:t>',
          '</w:r></w:p></w:ftr>',
        ].join('')
      )
    );
    const source = await archive.generateAsync({ type: 'blob' });
    const reopened = await importWorkFile(
      new File([source], 'Rich page chrome.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      })
    );
    expect(reopened.content.type).toBe('document');
    if (reopened.content.type !== 'document') return;
    const layout = documentSections(reopened.content)[0].layout;
    expect(layout.pageChrome).toMatchObject({
      differentFirstPage: true,
      differentOddEvenPages: true,
      default: { showPageNumber: true },
      first: { showPageNumber: false },
      even: { showPageNumber: true },
    });
    expect(layout.pageChrome?.default.headerHtml).toContain('<strong>Default header</strong>');
    expect(layout.pageChrome?.default.headerHtml).toContain('text-align: center');
    expect(layout.pageChrome?.first.headerHtml).toContain('<u>First header</u>');
    expect(layout.pageChrome?.first.headerHtml).toContain('text-align: right');
    expect(layout.pageChrome?.even.headerHtml).toContain('href="https://a3s.dev"');
    expect(layout.pageChrome?.even.headerHtml).toContain('color:');
    expect(layout.pageChrome?.default.footerHtml).toContain('/ Confidential');
    expect(layout.pageChrome?.default.footerHtml).not.toContain('42');
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'docx.headers')).toMatchObject({
      severity: 'info',
      message: expect.stringContaining('first-page'),
    });
  });

  it('reports unsupported fields, content controls, and advanced positioning', async () => {
    const exported = await createDocxBlob({
      type: 'document',
      pageSize: 'a4',
      pageChrome: {
        differentFirstPage: false,
        differentOddEvenPages: false,
        default: { headerHtml: '<p>Header</p>', footerHtml: '', showPageNumber: false },
        first: { headerHtml: '', footerHtml: '', showPageNumber: false },
        even: { headerHtml: '', footerHtml: '', showPageNumber: false },
      },
      html: '<p>Document body</p>',
    });
    const archive = await JSZip.loadAsync(exported);
    const headerPath = Object.keys(archive.files).find((path) => /^word\/header\d+\.xml$/.test(path));
    const headerXml = headerPath ? await archive.file(headerPath)?.async('text') : undefined;
    expect(headerPath).toBeTruthy();
    expect(headerXml).toBeTruthy();
    if (!headerPath || !headerXml) return;
    archive.file(
      headerPath,
      headerXml.replace(
        '</w:hdr>',
        [
          '<w:sdt><w:sdtContent><w:p><w:r><w:t>Controlled</w:t></w:r></w:p></w:sdtContent></w:sdt>',
          '<w:p><w:fldSimple w:instr=" NUMPAGES "><w:r><w:t>4</w:t></w:r></w:fldSimple></w:p>',
          '<w:p><w:pPr><w:tabs><w:tab w:val="left" w:pos="720"/></w:tabs></w:pPr>',
          '<w:r><w:drawing><wp:anchor/></w:drawing></w:r></w:p>',
          '</w:hdr>',
        ].join('')
      )
    );
    const source = await archive.generateAsync({ type: 'blob' });
    const report = await analyzeDocxCompatibility(
      new File([source], 'Unsupported page chrome.docx', {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      }),
      []
    );
    expect(report.issues.find((issue) => issue.code === 'docx.headers')).toMatchObject({ severity: 'info' });
    expect(report.issues.map((issue) => issue.code)).toEqual(
      expect.arrayContaining(['docx.headers.fields', 'docx.headers.content-controls', 'docx.headers.positioning'])
    );
  });
});
