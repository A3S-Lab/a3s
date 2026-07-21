import { describe, expect, it } from 'vitest';
import { directChildren, parseXml } from './work-ooxml-package';
import { diagnoseXlsxPageSetup } from './work-xlsx-page-setup-diagnostics';
import { readXlsxPageSetup, writeXlsxPageSetup } from './work-xlsx-page-setup';

describe('Work XLSX page setup', () => {
  it('writes and reads percentage scaling with deterministic worksheet ordering', () => {
    const document = parseXml(
      '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>'
    );
    writeXlsxPageSetup(document, {
      sheetId: 'sheet-1',
      paperSize: 'a4',
      orientation: 'landscape',
      scale: 200,
      fitToPage: false,
      fitToWidth: 1,
      fitToHeight: 0,
      horizontalCentered: false,
      verticalCentered: true,
      header: {
        left: 'Confidential',
        center: '{sheet} - {file}',
        right: 'Page {page} of {pages}',
      },
      footer: {
        left: '{date}',
        center: 'Internal',
        right: '{time}',
      },
      pageNumberStart: 7,
      pageOrder: 'downThenOver',
      scaleWithDocument: false,
      alignWithMargins: false,
      margins: {
        top: 12.7,
        right: 12.7,
        bottom: 12.7,
        left: 12.7,
        header: 12.7,
        footer: 12.7,
      },
    });

    expect(directChildren(document.documentElement).map((element) => element.localName)).toEqual([
      'sheetPr',
      'sheetData',
      'printOptions',
      'pageMargins',
      'pageSetup',
      'headerFooter',
    ]);
    expect(new XMLSerializer().serializeToString(document)).toContain('<pageSetUpPr fitToPage="0"/>');
    expect(new XMLSerializer().serializeToString(document)).toContain(
      '<pageSetup paperSize="9" orientation="landscape" scale="200" fitToWidth="1" fitToHeight="0" pageOrder="downThenOver" firstPageNumber="7" useFirstPageNumber="1"/>'
    );
    expect(new XMLSerializer().serializeToString(document)).toContain(
      '<headerFooter scaleWithDoc="0" alignWithMargins="0"><oddHeader>&amp;LConfidential&amp;C&amp;A - &amp;F&amp;RPage &amp;P of &amp;N</oddHeader><oddFooter>&amp;L&amp;D&amp;CInternal&amp;R&amp;T</oddFooter></headerFooter>'
    );
    expect(readXlsxPageSetup(document)).toEqual({
      paperSize: 'a4',
      orientation: 'landscape',
      scale: 200,
      fitToPage: false,
      fitToWidth: 1,
      fitToHeight: 0,
      horizontalCentered: false,
      verticalCentered: true,
      header: {
        left: 'Confidential',
        center: '{sheet} - {file}',
        right: 'Page {page} of {pages}',
      },
      footer: {
        left: '{date}',
        center: 'Internal',
        right: '{time}',
      },
      pageNumberStart: 7,
      pageOrder: 'downThenOver',
      scaleWithDocument: false,
      alignWithMargins: false,
      margins: {
        top: 12.7,
        right: 12.7,
        bottom: 12.7,
        left: 12.7,
        header: 12.7,
        footer: 12.7,
      },
    });
  });

  it.each([
    ['a3', '8'],
    ['a5', '11'],
    ['legal', '5'],
    ['tabloid', '3'],
  ] as const)('round-trips the %s paper-size code', (paperSize, code) => {
    const document = parseXml(
      '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>'
    );

    writeXlsxPageSetup(document, {
      sheetId: 'sheet-1',
      paperSize,
      orientation: 'portrait',
    });

    expect(new XMLSerializer().serializeToString(document)).toContain(
      `<pageSetup paperSize="${code}" orientation="portrait"/>`
    );
    expect(readXlsxPageSetup(document)).toEqual({
      paperSize,
      orientation: 'portrait',
    });
  });

  it.each(['1', '3', '5', '8', '9', '11'])('treats editable paper-size code %s as compatible', (paperSize) => {
    const document = parseXml(
      `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/><pageSetup paperSize="${paperSize}"/></worksheet>`
    );

    expect(diagnoseXlsxPageSetup(document).map((issue) => issue.code)).not.toContain('xlsx.page-setup.paper-size');
  });

  it('reports unsupported printer paper-size codes', () => {
    const document = parseXml(
      '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/><pageSetup paperSize="13"/></worksheet>'
    );

    expect(diagnoseXlsxPageSetup(document).find((issue) => issue.code === 'xlsx.page-setup.paper-size')).toMatchObject({
      severity: 'warning',
      message: expect.stringContaining('code 13'),
    });
  });
});
