import { describe, expect, it } from 'vitest';
import {
  effectiveSpreadsheetPageSetup,
  fitSpreadsheetAxisCapacity,
  spreadsheetPageCapacity,
} from './work-spreadsheet-page-setup';

describe('Work spreadsheet page setup', () => {
  it('derives deterministic page capacities from paper, orientation, margins, and percentage scaling', () => {
    const defaults = effectiveSpreadsheetPageSetup(undefined);
    expect(spreadsheetPageCapacity(defaults)).toEqual({ rows: 34, columns: 10 });
    expect(spreadsheetPageCapacity({ ...defaults, scale: 200 })).toEqual({ rows: 17, columns: 5 });
    expect(spreadsheetPageCapacity({ ...defaults, orientation: 'portrait' })).toEqual({
      rows: 51,
      columns: 6,
    });
    expect(spreadsheetPageCapacity({ ...defaults, paperSize: 'a3' })).toEqual({ rows: 51, columns: 14 });
    expect(spreadsheetPageCapacity({ ...defaults, paperSize: 'a5' })).toEqual({ rows: 21, columns: 6 });
    expect(spreadsheetPageCapacity({ ...defaults, paperSize: 'legal' })).toEqual({ rows: 35, columns: 12 });
    expect(spreadsheetPageCapacity({ ...defaults, paperSize: 'tabloid' })).toEqual({ rows: 47, columns: 15 });
  });

  it('derives fit-to-page capacity while reserving repeated title space', () => {
    expect(fitSpreadsheetAxisCapacity(0, 19, undefined, 2, 34)).toBe(10);
    expect(fitSpreadsheetAxisCapacity(5, 24, [0, 1], 1, 34)).toBe(22);
    expect(fitSpreadsheetAxisCapacity(0, 19, undefined, 0, 34)).toBe(34);
  });

  it('normalizes malformed persisted paper sizes to A4', () => {
    expect(
      effectiveSpreadsheetPageSetup({
        sheetId: 'sheet-1',
        paperSize: 'unsupported-printer-paper' as never,
      }).paperSize
    ).toBe('a4');
  });
});
