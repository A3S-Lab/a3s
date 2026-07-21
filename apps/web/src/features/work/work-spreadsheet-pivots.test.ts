import type { Cell, Selection } from '@fortune-sheet/core';
import { describe, expect, it } from 'vitest';
import {
  createSpreadsheetPivotFromSelection,
  deleteSpreadsheetPivotTable,
  reconcileSpreadsheetPivots,
  refreshSpreadsheetPivotTables,
  spreadsheetPivotOutputContains,
  spreadsheetPivotValidation,
} from './work-spreadsheet-pivots';
import { createWorkArtifact } from './work-templates';
import type { WorkSpreadsheetContent, WorkSpreadsheetPivotTable, WorkSpreadsheetSheet } from './work-types';

describe('spreadsheet pivot tables', () => {
  it('creates a report sheet and deterministically refreshes row, column, value, and grand totals', () => {
    const content = salesWorkbook();
    const source = content.sheets[0];
    const created = createSpreadsheetPivotFromSelection(content, source.id!, selection(0, 5, 0, 3));

    expect(created.error).toBeUndefined();
    expect(created.pivotId).toBeTruthy();
    const report = created.content.sheets.find((sheet) => sheet.id === created.ownerSheetId)!;
    const pivot = report.pivotTables?.[0];
    expect(pivot).toBeTruthy();

    pivot!.rowFields = [0];
    pivot!.columnFields = [2];
    pivot!.values = [{ fieldIndex: 3, aggregation: 'sum', caption: '收入合计' }];
    const refreshed = refreshSpreadsheetPivotTables(created.content);
    const refreshedReport = refreshed.sheets.find((sheet) => sheet.id === created.ownerSheetId)!;

    expect(values(refreshedReport.data)).toEqual([
      ['Region', 'Q1 · 收入合计', 'Q2 · 收入合计', '总计 · 收入合计'],
      ['East', 30, 40, 70],
      ['West', 30, 50, 80],
      ['总计', 60, 90, 150],
    ]);
    expect(refreshedReport.pivotTables?.[0].outputReference).toBe('A1:D4');
    expect(spreadsheetPivotOutputContains(refreshedReport, 2, 3)).toBe(true);
    expect(spreadsheetPivotOutputContains(refreshedReport, 5, 3)).toBe(false);
  });

  it('supports multiple values and statistical aggregations without coercing blanks into numbers', () => {
    const content = salesWorkbook();
    const report = addReportSheet(content);
    report.pivotTables = [
      pivotTable({
        sourceSheetId: content.sheets[0].id!,
        rowFields: [0],
        columnFields: [],
        values: [
          { fieldIndex: 3, aggregation: 'average', caption: '平均收入' },
          { fieldIndex: 1, aggregation: 'counta', caption: '产品数' },
        ],
      }),
    ];
    content.sheets[0].data![4][3] = null;

    const refreshed = refreshSpreadsheetPivotTables(content);

    expect(values(refreshed.sheets[1].data)).toEqual([
      ['Region', '平均收入', '产品数'],
      ['East', 15, 3],
      ['West', 40, 2],
      ['总计', 27.5, 5],
    ]);
  });

  it('filters source records by a single report item and materializes the filter above the report', () => {
    const content = salesWorkbook();
    const report = addReportSheet(content);
    report.pivotTables = [
      pivotTable({
        sourceSheetId: content.sheets[0].id!,
        rowFields: [0],
        columnFields: [],
        reportFilters: [{ fieldIndex: 2, selectedItem: 'Q1' }],
        values: [{ fieldIndex: 3, aggregation: 'sum', caption: '收入' }],
      }),
    ];

    const refreshed = refreshSpreadsheetPivotTables(content);

    expect(values(refreshed.sheets[1].data)).toEqual([
      ['Quarter', 'Q1'],
      ['Region', '收入'],
      ['East', 30],
      ['West', 30],
      ['总计', 60],
    ]);
    expect(refreshed.sheets[1].pivotTables?.[0].outputReference).toBe('A1:B6');
  });

  it('rejects invalid report-filter fields and selected items', () => {
    const content = salesWorkbook();
    const report = addReportSheet(content);
    const pivot = pivotTable({
      sourceSheetId: content.sheets[0].id!,
      reportFilters: [{ fieldIndex: 9 }],
    });
    report.pivotTables = [pivot];

    expect(spreadsheetPivotValidation(content, report.id!, pivot)).toMatchObject({
      valid: false,
      code: 'pivot.filters-invalid',
    });

    pivot.columnFields = [];
    pivot.reportFilters = [{ fieldIndex: 2, selectedItem: 'Q9' }];
    expect(spreadsheetPivotValidation(content, report.id!, pivot)).toMatchObject({
      valid: false,
      code: 'pivot.filter-item-invalid',
    });
  });

  it('clears a previous owned output range when a refreshed report shrinks', () => {
    const content = salesWorkbook();
    const report = addReportSheet(content);
    report.pivotTables = [
      pivotTable({
        sourceSheetId: content.sheets[0].id!,
        rowFields: [0],
        columnFields: [2],
        values: [{ fieldIndex: 3, aggregation: 'sum', caption: '收入' }],
        outputReference: 'A1:D4',
      }),
    ];
    report.data = [
      [{ v: 'stale' }, { v: 'stale' }, { v: 'stale' }, { v: 'stale' }],
      [{ v: 'stale' }, { v: 'stale' }, { v: 'stale' }, { v: 'stale' }],
      [{ v: 'stale' }, { v: 'stale' }, { v: 'stale' }, { v: 'stale' }],
      [{ v: 'stale' }, { v: 'stale' }, { v: 'stale' }, { v: 'stale' }],
    ];
    content.sheets[0].data = content.sheets[0].data!.slice(0, 4);

    const refreshed = refreshSpreadsheetPivotTables(content);

    expect(refreshed.sheets[1].pivotTables?.[0].outputReference).toBe('A1:C4');
    expect(refreshed.sheets[1].data?.[0]?.[3]).toBeNull();
    expect(refreshed.sheets[1].data?.[3]?.[3]).toBeNull();
  });

  it('reports unsafe source and destination overlaps instead of overwriting source data', () => {
    const content = salesWorkbook();
    content.sheets[0].pivotTables = [
      pivotTable({
        sourceSheetId: content.sheets[0].id!,
        sourceReference: 'A1:D6',
        anchor: 'B2',
        rowFields: [0],
        columnFields: [2],
        values: [{ fieldIndex: 3, aggregation: 'sum' }],
      }),
    ];

    expect(spreadsheetPivotValidation(content, content.sheets[0].id!, content.sheets[0].pivotTables[0])).toMatchObject({
      valid: false,
      code: 'pivot.output-overlaps-source',
    });
    const refreshed = refreshSpreadsheetPivotTables(content);
    expect(refreshed.sheets[0].data?.[1]?.[1]?.v).toBe('Alpha');
  });

  it('preserves pivot metadata while reconciling FortuneSheet changes', () => {
    const content = salesWorkbook();
    const report = addReportSheet(content);
    report.pivotTables = [pivotTable({ sourceSheetId: content.sheets[0].id! })];
    const changed = content.sheets.map((sheet) => ({ ...sheet, pivotTables: undefined }));
    changed[0].data = changed[0].data?.map((row) => [...row]);
    changed[0].data![1][3] = { v: 99, m: '99' };

    const reconciled = reconcileSpreadsheetPivots(content, changed);

    expect(reconciled.sheets[1].pivotTables?.[0]).toMatchObject(report.pivotTables![0]);
    expect(reconciled.sheets[1].pivotTables?.[0].outputReference).toBe('A1:D4');
    expect(reconciled.sheets[0].data?.[1]?.[3]?.v).toBe(99);
  });

  it('clears owned cells when a pivot table is deleted and rejects blank headers', () => {
    const content = salesWorkbook();
    const created = createSpreadsheetPivotFromSelection(content, content.sheets[0].id!, selection(0, 5, 0, 3));
    const deleted = deleteSpreadsheetPivotTable(created.content, created.ownerSheetId!, created.pivotId!);
    const report = deleted.sheets.find((sheet) => sheet.id === created.ownerSheetId)!;
    expect(report.pivotTables).toBeUndefined();
    expect(values(report.data)).toEqual([]);

    content.sheets[0].data![0][1] = null;
    expect(createSpreadsheetPivotFromSelection(content, content.sheets[0].id!, selection(0, 5, 0, 3)).error).toContain(
      '标题'
    );
  });
});

function salesWorkbook(): WorkSpreadsheetContent {
  const artifact = createWorkArtifact('blank-spreadsheet');
  if (artifact.content.type !== 'spreadsheet') throw new Error('Expected spreadsheet fixture');
  const sheet = artifact.content.sheets[0];
  sheet.name = 'Sales';
  sheet.data = [
    cells('Region', 'Product', 'Quarter', 'Revenue'),
    cells('East', 'Alpha', 'Q1', 10),
    cells('East', 'Beta', 'Q1', 20),
    cells('West', 'Alpha', 'Q1', 30),
    cells('East', 'Alpha', 'Q2', 40),
    cells('West', 'Beta', 'Q2', 50),
  ];
  sheet.row = 40;
  sheet.column = 12;
  return artifact.content;
}

function addReportSheet(content: WorkSpreadsheetContent): WorkSpreadsheetSheet {
  const report: WorkSpreadsheetSheet = {
    id: 'sheet-report',
    name: 'Pivot report',
    order: content.sheets.length,
    status: 0,
    row: 40,
    column: 12,
    data: Array.from({ length: 40 }, () => Array<Cell | null>(12).fill(null)),
    config: {},
  };
  content.sheets.push(report);
  return report;
}

function pivotTable(overrides: Partial<WorkSpreadsheetPivotTable>): WorkSpreadsheetPivotTable {
  return {
    id: 'pivot-1',
    name: 'PivotTable1',
    sourceSheetId: 'sheet-source',
    sourceReference: 'A1:D6',
    anchor: 'A1',
    rowFields: [0],
    columnFields: [2],
    values: [{ fieldIndex: 3, aggregation: 'sum' }],
    rowGrandTotals: true,
    columnGrandTotals: true,
    styleName: 'PivotStyleLight16',
    refreshOnLoad: true,
    ...overrides,
  };
}

function cells(...items: Array<string | number>): Cell[] {
  return items.map((value) => ({ v: value, m: String(value) }));
}

function selection(rowStart: number, rowEnd: number, columnStart: number, columnEnd: number): Selection {
  return { row: [rowStart, rowEnd], column: [columnStart, columnEnd] };
}

function values(data: Array<Array<Cell | null>> | undefined): unknown[][] {
  return (data ?? [])
    .map((row) => row.map((cell) => cell?.v ?? null))
    .filter((row) => row.some((value) => value !== null))
    .map((row) => {
      let end = row.length;
      while (end && row[end - 1] === null) end -= 1;
      return row.slice(0, end);
    });
}
