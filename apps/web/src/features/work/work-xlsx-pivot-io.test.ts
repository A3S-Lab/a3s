import type { Cell, Selection } from '@fortune-sheet/core';
import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { createWorkArtifactBlob, importWorkFile } from './work-file-io';
import { createSpreadsheetPivotFromSelection, refreshSpreadsheetPivotTables } from './work-spreadsheet-pivots';
import { createWorkArtifact } from './work-templates';

describe('XLSX pivot table interoperability', () => {
  it('exports and reimports worksheet-backed pivot definitions, caches, records, relationships, and results', async () => {
    const { artifact, report } = pivotArtifact();

    const blob = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(blob);
    const workbookXml = await archive.file('xl/workbook.xml')?.async('text');
    const workbookRelationships = await archive.file('xl/_rels/workbook.xml.rels')?.async('text');
    const reportRelationships = await archive.file('xl/worksheets/_rels/sheet2.xml.rels')?.async('text');
    const pivotXml = await archive.file('xl/pivotTables/pivotTable1.xml')?.async('text');
    const cacheXml = await archive.file('xl/pivotCache/pivotCacheDefinition1.xml')?.async('text');
    const recordsXml = await archive.file('xl/pivotCache/pivotCacheRecords1.xml')?.async('text');
    const contentTypes = await archive.file('[Content_Types].xml')?.async('text');

    expect(workbookXml).toContain('<pivotCaches>');
    expect(workbookXml).toMatch(/<pivotCache cacheId="1" r:id="rId\d+"\/>/);
    expect(workbookRelationships).toContain('/pivotCacheDefinition');
    expect(reportRelationships).toContain('/pivotTable');
    expect(pivotXml).toContain('name="SalesPivot"');
    expect(pivotXml).toContain('cacheId="1"');
    expect(pivotXml).toContain('<location ref="A1:D4"');
    expect(pivotXml).toContain('<field x="0"/>');
    expect(pivotXml).toContain('<field x="2"/>');
    expect(pivotXml).toContain('name="收入合计" fld="3" subtotal="sum"');
    expect(cacheXml).toContain('<worksheetSource ref="A1:D6" sheet="Sales"/>');
    expect(cacheXml).toContain('<cacheFields count="4">');
    expect(recordsXml).toContain('<pivotCacheRecords');
    expect(recordsXml).toContain('count="5"');
    expect(recordsXml?.match(/<r>/g)).toHaveLength(5);
    expect(contentTypes).toContain('pivotTable+xml');
    expect(contentTypes).toContain('pivotCacheDefinition+xml');
    expect(contentTypes).toContain('pivotCacheRecords+xml');

    const reopened = await importWorkFile(
      new File([blob], 'Sales report.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    const reopenedSource = reopened.content.sheets.find((sheet) => sheet.name === 'Sales')!;
    const reopenedReport = reopened.content.sheets.find((sheet) => sheet.name === report.name)!;
    expect(reopenedReport.pivotTables?.[0]).toMatchObject({
      name: 'SalesPivot',
      sourceSheetId: reopenedSource.id,
      sourceReference: 'A1:D6',
      anchor: 'A1',
      outputReference: 'A1:D4',
      rowFields: [0],
      columnFields: [2],
      values: [{ fieldIndex: 3, aggregation: 'sum', caption: '收入合计' }],
      rowGrandTotals: true,
      columnGrandTotals: true,
      styleName: 'PivotStyleLight16',
      refreshOnLoad: true,
    });
    expect(reopenedReport.data?.[1]?.[3]?.v).toBe(70);
    expect(reopened.compatibility?.issues).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          code: 'xlsx.pivots',
          severity: 'info',
          message: expect.stringContaining('editable'),
        }),
      ])
    );
  });

  it('keeps external-source pivot results visible as cells and reports the native definition as unsupported', async () => {
    const { artifact, report } = pivotArtifact();
    const archive = await JSZip.loadAsync(await createWorkArtifactBlob(artifact));
    const cachePath = 'xl/pivotCache/pivotCacheDefinition1.xml';
    const cacheXml = await archive.file(cachePath)?.async('text');
    archive.file(cachePath, cacheXml!.replace('cacheSource type="worksheet"', 'cacheSource type="external"'));
    const bytes = await archive.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });

    const reopened = await importWorkFile(
      new File([bytes], 'External pivot.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );

    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    const reopenedReport = reopened.content.sheets.find((sheet) => sheet.name === report.name)!;
    expect(reopenedReport.pivotTables).toBeUndefined();
    expect(reopenedReport.data?.[1]?.[3]?.v).toBe(70);
    expect(reopened.compatibility?.issues).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          code: 'xlsx.pivots.external-source',
          severity: 'warning',
          message: expect.stringContaining('ordinary cells'),
        }),
      ])
    );
  });

  it('exports and reimports a single-selection report filter as a native page field', async () => {
    const { artifact } = pivotArtifact();
    if (artifact.content.type !== 'spreadsheet') throw new Error('Expected spreadsheet fixture');
    const report = artifact.content.sheets.find((sheet) => sheet.pivotTables?.length)!;
    const pivot = report.pivotTables![0];
    pivot.columnFields = [];
    pivot.reportFilters = [{ fieldIndex: 2, selectedItem: 'Q1' }];
    artifact.content = refreshSpreadsheetPivotTables(artifact.content);

    const blob = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(blob);
    const pivotXml = await archive.file('xl/pivotTables/pivotTable1.xml')?.async('text');

    expect(pivotXml).toContain('axis="axisPage"');
    expect(pivotXml).toContain('<pageFields count="1"><pageField fld="2" hier="-1" item="0"/></pageFields>');
    expect(pivotXml).toContain('<location ref="A3:B6"');

    const reopened = await importWorkFile(
      new File([blob], 'Filtered sales report.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    const reopenedReport = reopened.content.sheets.find((sheet) => sheet.name === report.name)!;
    expect(reopenedReport.pivotTables?.[0]).toMatchObject({
      anchor: 'A1',
      outputReference: 'A1:B6',
      columnFields: [],
      reportFilters: [{ fieldIndex: 2, selectedItem: 'Q1' }],
    });
    expect(reopenedReport.data?.[0]?.[0]?.v).toBe('Quarter');
    expect(reopenedReport.data?.[0]?.[1]?.v).toBe('Q1');
    expect(reopenedReport.data?.[3]?.[1]?.v).toBe(30);
  });

  it('keeps multi-selection report filters cached and reports the unsupported layout precisely', async () => {
    const { artifact, report } = pivotArtifact();
    if (artifact.content.type !== 'spreadsheet') throw new Error('Expected spreadsheet fixture');
    const pivot = artifact.content.sheets.find((sheet) => sheet.pivotTables?.length)!.pivotTables![0];
    pivot.columnFields = [];
    pivot.reportFilters = [{ fieldIndex: 2, selectedItem: 'Q1' }];
    artifact.content = refreshSpreadsheetPivotTables(artifact.content);
    const archive = await JSZip.loadAsync(await createWorkArtifactBlob(artifact));
    const pivotPath = 'xl/pivotTables/pivotTable1.xml';
    const pivotXml = await archive.file(pivotPath)?.async('text');
    archive.file(pivotPath, pivotXml!.replace('axis="axisPage"', 'axis="axisPage" multipleItemSelectionAllowed="1"'));
    const bytes = await archive.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });

    const reopened = await importWorkFile(
      new File([bytes], 'Multi-filter sales report.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );

    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    const reopenedReport = reopened.content.sheets.find((sheet) => sheet.name === report.name)!;
    expect(reopenedReport.pivotTables).toBeUndefined();
    expect(reopenedReport.data?.[3]?.[1]?.v).toBe(30);
    expect(reopened.compatibility?.issues).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          code: 'xlsx.pivots.report-filter-multi-select',
          severity: 'warning',
        }),
      ])
    );
  });
});

function pivotArtifact() {
  const artifact = createWorkArtifact('blank-spreadsheet');
  if (artifact.content.type !== 'spreadsheet') throw new Error('Expected spreadsheet fixture');
  const source = artifact.content.sheets[0];
  source.name = 'Sales';
  source.data = [
    cells('Region', 'Product', 'Quarter', 'Revenue'),
    cells('East', 'Alpha', 'Q1', 10),
    cells('East', 'Beta', 'Q1', 20),
    cells('West', 'Alpha', 'Q1', 30),
    cells('East', 'Alpha', 'Q2', 40),
    cells('West', 'Beta', 'Q2', 50),
  ];
  const created = createSpreadsheetPivotFromSelection(artifact.content, source.id!, selection(0, 5, 0, 3));
  if (created.error) throw new Error(created.error);
  const report = created.content.sheets.find((sheet) => sheet.id === created.ownerSheetId)!;
  const pivot = report.pivotTables![0];
  pivot.name = 'SalesPivot';
  pivot.rowFields = [0];
  pivot.columnFields = [2];
  pivot.values = [{ fieldIndex: 3, aggregation: 'sum', caption: '收入合计' }];
  artifact.content = refreshSpreadsheetPivotTables(created.content);
  return { artifact, report };
}

function cells(...items: Array<string | number>): Cell[] {
  return items.map((value) => ({ v: value, m: String(value) }));
}

function selection(rowStart: number, rowEnd: number, columnStart: number, columnEnd: number): Selection {
  return { row: [rowStart, rowEnd], column: [columnStart, columnEnd] };
}
