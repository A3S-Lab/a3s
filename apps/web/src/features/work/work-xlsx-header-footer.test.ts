import { describe, expect, it } from 'vitest';
import { resolveSpreadsheetHeaderFooterTemplate } from './work-spreadsheet-header-footer';
import { parseXlsxHeaderFooterSections, serializeXlsxHeaderFooterSections } from './work-xlsx-header-footer';

describe('Work XLSX headers and footers', () => {
  it('maps OOXML sections and dynamic fields to editable friendly tokens', () => {
    const source = '&LConfidential&C&A - &F&RPage &P of &N';

    expect(parseXlsxHeaderFooterSections(source)).toEqual({
      left: 'Confidential',
      center: '{sheet} - {file}',
      right: 'Page {page} of {pages}',
    });
    expect(
      serializeXlsxHeaderFooterSections({
        left: 'R&D',
        center: '{sheet} - {file}',
        right: 'Page {page} of {pages}',
      })
    ).toBe('&LR&&D&C&A - &F&RPage &P of &N');
  });

  it('resolves every friendly print token deterministically', () => {
    expect(
      resolveSpreadsheetHeaderFooterTemplate('{page}/{pages} · {sheet} · {file} · {path} · {date} {time}', {
        page: 7,
        pages: 12,
        sheetName: 'Plan',
        fileName: 'Quarterly plan.xlsx',
        filePath: '/Work/Finance',
        now: new Date(2026, 6, 20, 14, 5),
      })
    ).toBe('7/12 · Plan · Quarterly plan.xlsx · /Work/Finance · 2026-07-20 14:05');
  });
});
