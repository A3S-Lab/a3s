export function parseSpreadsheetRowPageBreaks(value: string, maximumRow: number): number[] | null {
  return parsePageBreakList(value, maximumRow, (token) => {
    const match = /^\$?([1-9]\d*)$/.exec(token);
    if (!match) return null;
    const row = Number(match[1]);
    return Number.isSafeInteger(row) ? row - 1 : null;
  });
}

export function formatSpreadsheetRowPageBreaks(breaks: number[] | undefined): string {
  return normalizedBreaks(breaks)
    .map((row) => String(row + 1))
    .join(', ');
}

export function parseSpreadsheetColumnPageBreaks(value: string, maximumColumn: number): number[] | null {
  return parsePageBreakList(value, maximumColumn, decodeColumn);
}

export function formatSpreadsheetColumnPageBreaks(breaks: number[] | undefined): string {
  return normalizedBreaks(breaks).map(encodeColumn).join(', ');
}

function parsePageBreakList(value: string, maximum: number, decode: (token: string) => number | null): number[] | null {
  const tokens = value
    .trim()
    .split(/[,;\s]+/)
    .filter(Boolean);
  if (!tokens.length) return [];
  const limit = Math.max(0, Math.trunc(maximum));
  const breaks: number[] = [];
  for (const token of tokens) {
    const pageBreak = decode(token);
    if (pageBreak === null || pageBreak <= 0 || pageBreak > limit) return null;
    breaks.push(pageBreak);
  }
  return normalizedBreaks(breaks);
}

function normalizedBreaks(breaks: number[] | undefined): number[] {
  return Array.from(new Set((breaks ?? []).filter((value) => Number.isSafeInteger(value) && value > 0))).sort(
    (left, right) => left - right
  );
}

function decodeColumn(token: string): number | null {
  const match = /^\$?([A-Z]+)$/i.exec(token);
  if (!match) return null;
  let value = 0;
  for (const character of match[1].toUpperCase()) {
    value = value * 26 + character.charCodeAt(0) - 64;
    if (!Number.isSafeInteger(value)) return null;
  }
  return value - 1;
}

function encodeColumn(column: number): string {
  let value = column + 1;
  let label = '';
  while (value > 0) {
    value -= 1;
    label = String.fromCharCode(65 + (value % 26)) + label;
    value = Math.floor(value / 26);
  }
  return label;
}
