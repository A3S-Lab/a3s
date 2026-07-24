const DELIMITER_ROW = /^\|\s*:?-+:?\s*(?:\|\s*:?-+:?\s*)+\|/;
const COMPLETE_DELIMITER_ROW = /^\|\s*:?-+:?\s*(?:\|\s*:?-+:?\s*)+\|$/;

interface Fence {
  marker: '`' | '~';
  length: number;
}

export function normalizeCollapsedMarkdownTables(content: string): string {
  let fence: Fence | null = null;
  const repaired = content
    .split('\n')
    .map((line) => {
      const fenceRun = line.match(/^ {0,3}(`{3,}|~{3,})/)?.[1];
      if (fence) {
        if (fenceRun?.startsWith(fence.marker) && fenceRun.length >= fence.length) fence = null;
        return line;
      }
      if (fenceRun) {
        fence = { marker: fenceRun[0] as Fence['marker'], length: fenceRun.length };
        return line;
      }
      return repairCollapsedTableLine(line);
    })
    .join('\n');
  return repairShortDelimiterRows(repaired);
}

function repairShortDelimiterRows(content: string): string {
  let fence: Fence | null = null;
  const lines = content.split('\n');
  return lines
    .map((line, index) => {
      const fenceRun = line.match(/^ {0,3}(`{3,}|~{3,})/)?.[1];
      if (fence) {
        if (fenceRun?.startsWith(fence.marker) && fenceRun.length >= fence.length) fence = null;
        return line;
      }
      if (fenceRun) {
        fence = { marker: fenceRun[0] as Fence['marker'], length: fenceRun.length };
        return line;
      }

      const delimiter = line.trim();
      if (!COMPLETE_DELIMITER_ROW.test(delimiter)) return line;
      const header = lines[index - 1]?.trim();
      if (!header?.startsWith('|') || !header.endsWith('|')) return line;

      const headerColumnCount = countUnescapedPipes(header) - 1;
      const delimiterColumnCount = countUnescapedPipes(delimiter) - 1;
      if (delimiterColumnCount < 2 || headerColumnCount <= delimiterColumnCount) return line;

      const indentation = line.slice(0, line.indexOf('|'));
      if (indentation.length > 3) return line;
      return `${indentation}${delimiter}${'---|'.repeat(headerColumnCount - delimiterColumnCount)}`;
    })
    .join('\n');
}

function repairCollapsedTableLine(line: string): string {
  const delimiterBoundary = /\|\s*\|(?=\s*:?-+:?\s*\|)/g.exec(line);
  if (!delimiterBoundary) return line;

  const headerEnd = delimiterBoundary.index + 1;
  const delimiterStart = delimiterBoundary.index + delimiterBoundary[0].lastIndexOf('|');
  const delimiterAndRows = line.slice(delimiterStart);
  const delimiter = delimiterAndRows.match(DELIMITER_ROW)?.[0];
  if (!delimiter) return line;

  const columnCount = countUnescapedPipes(delimiter) - 1;
  const headerSource = line.slice(0, headerEnd);
  const headerStart = unescapedPipeIndexes(headerSource).at(-(columnCount + 1));
  if (headerStart === undefined) return line;
  const prefix = headerSource.slice(0, headerStart);
  const prose = prefix.trim();
  const indentation = prose ? '' : prefix;
  if (!prose && indentation.length > 3) return line;
  const normalizedHeader = headerSource.slice(headerStart).trim();
  if (
    columnCount < 2 ||
    !normalizedHeader.startsWith('|') ||
    !normalizedHeader.endsWith('|') ||
    countUnescapedPipes(normalizedHeader) - 1 !== columnCount
  ) {
    return line;
  }

  const rowSource = delimiterAndRows.slice(delimiter.length).trim();
  if (rowSource && !rowSource.startsWith('|')) return line;
  const rows = rowSource ? splitRows(rowSource, columnCount) : [];
  if (!rows) return line;

  return [
    ...(prose ? [prose] : []),
    `${indentation}${normalizedHeader}`,
    `${indentation}${delimiter.trim()}`,
    ...rows.map((row) => `${indentation}${row}`),
  ].join('\n');
}

function splitRows(source: string, columnCount: number): string[] | null {
  const rows: string[] = [];
  let remaining = source;
  while (remaining) {
    if (!remaining.startsWith('|')) return null;
    const rowEnd = indexOfUnescapedPipe(remaining, columnCount + 1);
    if (rowEnd === -1) {
      rows.push(remaining.trimEnd());
      break;
    }
    rows.push(remaining.slice(0, rowEnd + 1).trimEnd());
    remaining = remaining.slice(rowEnd + 1).trimStart();
  }
  return rows.length ? rows : null;
}

function countUnescapedPipes(value: string): number {
  return unescapedPipeIndexes(value).length;
}

function unescapedPipeIndexes(value: string): number[] {
  const indexes: number[] = [];
  for (let index = 0; index < value.length; index += 1) {
    if (value[index] === '|' && !isEscaped(value, index)) indexes.push(index);
  }
  return indexes;
}

function indexOfUnescapedPipe(value: string, occurrence: number): number {
  let count = 0;
  for (let index = 0; index < value.length; index += 1) {
    if (value[index] !== '|' || isEscaped(value, index)) continue;
    count += 1;
    if (count === occurrence) return index;
  }
  return -1;
}

function isEscaped(value: string, index: number): boolean {
  let slashes = 0;
  for (let cursor = index - 1; cursor >= 0 && value[cursor] === '\\'; cursor -= 1) slashes += 1;
  return slashes % 2 === 1;
}
