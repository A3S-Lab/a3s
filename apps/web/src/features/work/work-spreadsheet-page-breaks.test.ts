import { describe, expect, it } from 'vitest';
import {
  formatSpreadsheetColumnPageBreaks,
  formatSpreadsheetRowPageBreaks,
  parseSpreadsheetColumnPageBreaks,
  parseSpreadsheetRowPageBreaks,
} from './work-spreadsheet-page-breaks';

describe('Work spreadsheet page breaks', () => {
  it('parses, sorts, deduplicates, and formats manual row page breaks', () => {
    expect(parseSpreadsheetRowPageBreaks('35, 20; 35', 59)).toEqual([19, 34]);
    expect(formatSpreadsheetRowPageBreaks([34, 19])).toBe('20, 35');
  });

  it('parses, sorts, deduplicates, and formats manual column page breaks', () => {
    expect(parseSpreadsheetColumnPageBreaks('K, e, $K', 25)).toEqual([4, 10]);
    expect(formatSpreadsheetColumnPageBreaks([10, 4])).toBe('E, K');
  });

  it('rejects breaks at the first edge or outside the worksheet', () => {
    expect(parseSpreadsheetRowPageBreaks('1', 59)).toBeNull();
    expect(parseSpreadsheetRowPageBreaks('61', 59)).toBeNull();
    expect(parseSpreadsheetColumnPageBreaks('A', 25)).toBeNull();
    expect(parseSpreadsheetColumnPageBreaks('AA', 25)).toBeNull();
  });
});
