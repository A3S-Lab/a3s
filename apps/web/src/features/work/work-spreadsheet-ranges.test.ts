import { describe, expect, it } from 'vitest';
import {
  formatSpreadsheetCellRanges,
  isValidSpreadsheetDefinedName,
  normalizeSpreadsheetPrintArea,
  normalizeSpreadsheetPrintTitleColumns,
  normalizeSpreadsheetPrintTitleRows,
  parseSpreadsheetCellRanges,
  spreadsheetPrintTitleBounds,
  qualifySpreadsheetRange,
  spreadsheetPrintBounds,
  stripSpreadsheetSheetQualifier,
} from './work-spreadsheet-ranges';

describe('Work spreadsheet ranges', () => {
  it('validates Excel-compatible workbook names without rejecting CJK names', () => {
    expect(isValidSpreadsheetDefinedName('Revenue_2026')).toBe(true);
    expect(isValidSpreadsheetDefinedName('收入目标')).toBe(true);
    expect(isValidSpreadsheetDefinedName('A1')).toBe(false);
    expect(isValidSpreadsheetDefinedName('R1C1')).toBe(false);
    expect(isValidSpreadsheetDefinedName('_xlnm.Print_Area')).toBe(false);
  });

  it('normalizes, qualifies, and unqualifies print-area references', () => {
    expect(normalizeSpreadsheetPrintArea('=$a$1:$c$20, e1:f4')).toBe('$A$1:$C$20,E1:F4');
    expect(normalizeSpreadsheetPrintArea('A0:C4')).toBeNull();
    const qualified = qualifySpreadsheetRange('$A$1:$C$20,E1:F4', "Q1's Plan");
    expect(qualified).toBe("'Q1''s Plan'!$A$1:$C$20,'Q1''s Plan'!E1:F4");
    expect(stripSpreadsheetSheetQualifier(qualified, "Q1's Plan")).toBe('$A$1:$C$20,E1:F4');
  });

  it('converts editable A1 lists to and from FortuneSheet cell ranges', () => {
    const ranges = parseSpreadsheetCellRanges('$c$8:$a$2, F4');
    expect(ranges).toEqual([
      { row: [1, 7], column: [0, 2] },
      { row: [3, 3], column: [5, 5] },
    ]);
    expect(formatSpreadsheetCellRanges(ranges!)).toBe('A2:C8,F4');
    expect(parseSpreadsheetCellRanges('A:A')).toBeNull();
    expect(parseSpreadsheetCellRanges('A0:C4')).toBeNull();
  });

  it('bounds disjoint, row-only, and column-only print areas to populated sheet dimensions', () => {
    expect(spreadsheetPrintBounds('$B$2:$C$3,E5:F8', 5, 4)).toEqual({
      startRow: 1,
      endRow: 5,
      startColumn: 1,
      endColumn: 4,
    });
    expect(spreadsheetPrintBounds('$B:$D', 20, 10)).toEqual({
      startRow: 0,
      endRow: 20,
      startColumn: 1,
      endColumn: 3,
    });
    expect(spreadsheetPrintBounds('$3:$7', 5, 10)).toEqual({
      startRow: 2,
      endRow: 5,
      startColumn: 0,
      endColumn: 10,
    });
  });

  it('normalizes and bounds repeated print-title rows and columns', () => {
    expect(normalizeSpreadsheetPrintTitleRows('=$3:$1')).toBe('$1:$3');
    expect(normalizeSpreadsheetPrintTitleColumns('=c:$a')).toBe('$A:$C');
    expect(normalizeSpreadsheetPrintTitleRows('$A:$C')).toBeNull();
    expect(normalizeSpreadsheetPrintTitleColumns('$1:$3')).toBeNull();
    expect(spreadsheetPrintTitleBounds('$1:$3', '$B:$D', 1, 2)).toEqual({
      rows: [0, 1],
      columns: [1, 2],
    });
  });
});
