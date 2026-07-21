import { describe, expect, it } from 'vitest';
import { parseWorkPrintRange } from './work-print-range';

describe('Work print ranges', () => {
  it('normalizes individual pages, ranges, duplicates, and localized separators', () => {
    expect(parseWorkPrintRange('1-3, 5, 3', 6)).toEqual({
      pageIndexes: [0, 1, 2, 4],
      error: null,
    });
    expect(parseWorkPrintRange('2–4、6', 6)).toEqual({
      pageIndexes: [1, 2, 3, 5],
      error: null,
    });
  });

  it('rejects empty, descending, malformed, and out-of-bounds ranges', () => {
    expect(parseWorkPrintRange('', 5).error).toContain('页码');
    expect(parseWorkPrintRange('4-2', 5).error).toContain('起始页');
    expect(parseWorkPrintRange('1-two', 5).error).toContain('格式');
    expect(parseWorkPrintRange('1, 6', 5).error).toContain('1 到 5');
  });

  it('reports that an artifact has no printable pages', () => {
    expect(parseWorkPrintRange('1', 0)).toEqual({
      pageIndexes: [],
      error: '当前文件没有可打印页面。',
    });
  });
});
