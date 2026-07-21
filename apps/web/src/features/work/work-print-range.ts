export interface WorkPrintRangeResult {
  pageIndexes: number[];
  error: string | null;
}

export function parseWorkPrintRange(value: string, pageCount: number): WorkPrintRangeResult {
  if (!Number.isSafeInteger(pageCount) || pageCount < 1) {
    return { pageIndexes: [], error: '当前文件没有可打印页面。' };
  }

  const normalized = value
    .trim()
    .replace(/[，、；;]/g, ',')
    .replace(/[–—]/g, '-');
  if (!normalized) {
    return { pageIndexes: [], error: '请输入要打印的页码。' };
  }

  const selected = new Set<number>();
  for (const rawToken of normalized.split(',')) {
    const token = rawToken.trim();
    const match = token.match(/^(\d+)(?:\s*-\s*(\d+))?$/);
    if (!match) {
      return { pageIndexes: [], error: '页码格式无效，请使用类似 1-3, 5 的格式。' };
    }

    const start = Number(match[1]);
    const end = Number(match[2] ?? match[1]);
    if (start < 1 || end < 1 || start > pageCount || end > pageCount) {
      return { pageIndexes: [], error: `页码必须在 1 到 ${pageCount} 之间。` };
    }
    if (start > end) {
      return { pageIndexes: [], error: '页码范围的起始页不能大于结束页。' };
    }
    for (let page = start; page <= end; page += 1) selected.add(page - 1);
  }

  return {
    pageIndexes: [...selected].sort((left, right) => left - right),
    error: null,
  };
}
