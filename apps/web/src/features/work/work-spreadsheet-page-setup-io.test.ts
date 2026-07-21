import JSZip from 'jszip';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { exportWorkArtifact, importWorkFile } from './work-file-io';

describe('Work spreadsheet page-setup interoperability', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('imports, diagnoses, exports, and reopens editable XLSX page setup', async () => {
    const fixture = await pageSetupFixture();
    const artifact = await importWorkFile(fixture);
    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    const sheet = artifact.content.sheets[0];
    expect(artifact.content.pageSetups).toEqual([
      {
        sheetId: sheet.id,
        paperSize: 'letter',
        orientation: 'portrait',
        scale: 85,
        fitToPage: true,
        fitToWidth: 1,
        fitToHeight: 2,
        horizontalCentered: true,
        verticalCentered: false,
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
          left: 12.7,
          right: 15.24,
          top: 17.78,
          bottom: 20.32,
          header: 7.62,
          footer: 10.16,
        },
      },
    ]);
    expect(artifact.compatibility?.issues.find((issue) => issue.code === 'xlsx.page-setup')).toMatchObject({
      severity: 'info',
      message: expect.stringContaining('PDF pagination'),
    });
    expect(artifact.compatibility?.issues.find((issue) => issue.code === 'xlsx.header-footer')).toMatchObject({
      severity: 'info',
      message: expect.stringContaining('editable'),
    });
    expect(artifact.compatibility?.issues.map((issue) => issue.code)).toEqual(
      expect.arrayContaining([
        'xlsx.header-footer.variants',
        'xlsx.header-footer.formatting',
        'xlsx.header-footer.images',
      ])
    );

    let exported: Blob | null = null;
    vi.spyOn(URL, 'createObjectURL').mockImplementation((value) => {
      if (value instanceof Blob) exported = value;
      return 'blob:a3s-work-page-setup';
    });
    vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});

    await exportWorkArtifact(artifact);
    expect(exported).toBeInstanceOf(Blob);
    if (!exported) return;
    const archive = await JSZip.loadAsync(exported);
    const worksheetXml = await archive.file('xl/worksheets/sheet1.xml')?.async('text');
    expect(worksheetXml).toContain('<pageSetUpPr fitToPage="1"/>');
    expect(worksheetXml).toContain('<printOptions horizontalCentered="1" verticalCentered="0"/>');
    expect(worksheetXml).toContain(
      '<pageMargins left="0.5" right="0.6" top="0.7" bottom="0.8" header="0.3" footer="0.4"/>'
    );
    expect(worksheetXml).toContain(
      '<pageSetup paperSize="1" orientation="portrait" scale="85" fitToWidth="1" fitToHeight="2" pageOrder="downThenOver" firstPageNumber="7" useFirstPageNumber="1"/>'
    );
    expect(worksheetXml).toContain(
      '<headerFooter scaleWithDoc="0" alignWithMargins="0"><oddHeader>&amp;LConfidential&amp;C&amp;A - &amp;F&amp;RPage &amp;P of &amp;N</oddHeader><oddFooter>&amp;L&amp;D&amp;CInternal&amp;R&amp;T</oddFooter></headerFooter>'
    );

    const reopened = await importWorkFile(
      new File([exported], 'Page setup reopened.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.pageSetups).toEqual([
      {
        ...artifact.content.pageSetups?.[0],
        sheetId: reopened.content.sheets[0].id,
      },
    ]);
  });
});

async function pageSetupFixture(): Promise<File> {
  const XLSX = await import('xlsx');
  const workbook = XLSX.utils.book_new();
  XLSX.utils.book_append_sheet(
    workbook,
    XLSX.utils.aoa_to_sheet([
      ['Metric', 'Value'],
      ['Adoption', 42],
    ]),
    'Plan'
  );
  const bytes = XLSX.write(workbook, { type: 'array', bookType: 'xlsx' }) as ArrayBuffer;
  const archive = await JSZip.loadAsync(bytes);
  const entry = archive.file('xl/worksheets/sheet1.xml');
  if (!entry) throw new Error('Worksheet fixture is missing');
  const document = new DOMParser().parseFromString(await entry.async('text'), 'application/xml');
  const root = document.documentElement;
  const namespace = root.namespaceURI;

  const sheetProperties = document.createElementNS(namespace, 'sheetPr');
  const pageSetupProperties = document.createElementNS(namespace, 'pageSetUpPr');
  pageSetupProperties.setAttribute('fitToPage', '1');
  sheetProperties.append(pageSetupProperties);
  root.insertBefore(sheetProperties, root.firstElementChild);

  const printOptions = document.createElementNS(namespace, 'printOptions');
  printOptions.setAttribute('horizontalCentered', '1');
  printOptions.setAttribute('verticalCentered', '0');
  const margins = document.createElementNS(namespace, 'pageMargins');
  for (const [name, value] of Object.entries({
    left: '0.5',
    right: '0.6',
    top: '0.7',
    bottom: '0.8',
    header: '0.3',
    footer: '0.4',
  })) {
    margins.setAttribute(name, value);
  }
  const pageSetup = document.createElementNS(namespace, 'pageSetup');
  for (const [name, value] of Object.entries({
    paperSize: '1',
    orientation: 'portrait',
    scale: '85',
    fitToWidth: '1',
    fitToHeight: '2',
    pageOrder: 'downThenOver',
    firstPageNumber: '7',
    useFirstPageNumber: '1',
  })) {
    pageSetup.setAttribute(name, value);
  }
  const headerFooter = document.createElementNS(namespace, 'headerFooter');
  headerFooter.setAttribute('scaleWithDoc', '0');
  headerFooter.setAttribute('alignWithMargins', '0');
  headerFooter.setAttribute('differentFirst', '1');
  headerFooter.setAttribute('differentOddEven', '1');
  for (const [name, value] of Object.entries({
    oddHeader: '&LConfidential&C&A - &F&RPage &P of &N',
    oddFooter: '&L&D&CInternal&R&T',
    firstHeader: '&C&BFirst page &G',
    evenFooter: '&CEven page',
  })) {
    const element = document.createElementNS(namespace, name);
    element.textContent = value;
    headerFooter.append(element);
  }
  const anchor = Array.from(root.children).find((element) => element.localName === 'ignoredErrors') ?? null;
  root.insertBefore(printOptions, anchor);
  root.insertBefore(margins, anchor);
  root.insertBefore(pageSetup, anchor);
  root.insertBefore(headerFooter, anchor);
  archive.file('xl/worksheets/sheet1.xml', new XMLSerializer().serializeToString(document));
  return new File([await archive.generateAsync({ type: 'arraybuffer' })], 'Page setup.xlsx', {
    type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  });
}
