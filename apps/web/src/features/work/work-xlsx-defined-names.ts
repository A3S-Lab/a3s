import type { DefinedName, WorkBook } from 'xlsx';
import {
  isValidSpreadsheetDefinedName,
  normalizeSpreadsheetPrintArea,
  normalizeSpreadsheetPrintTitleColumns,
  normalizeSpreadsheetPrintTitleRows,
  parseSpreadsheetPrintTitles,
  qualifySpreadsheetRange,
  stripSpreadsheetSheetQualifier,
} from './work-spreadsheet-ranges';
import { createWorkId } from './work-templates';
import type {
  WorkSpreadsheetContent,
  WorkSpreadsheetNamedRange,
  WorkSpreadsheetPrintArea,
  WorkSpreadsheetPrintTitles,
} from './work-types';

export function importXlsxDefinedNames(
  workbook: WorkBook,
  sheets: WorkSpreadsheetContent['sheets']
): Pick<WorkSpreadsheetContent, 'namedRanges' | 'printAreas' | 'printTitles'> {
  const namedRanges: WorkSpreadsheetNamedRange[] = [];
  const printAreas: WorkSpreadsheetPrintArea[] = [];
  const printTitles: WorkSpreadsheetPrintTitles[] = [];
  for (const definedName of workbook.Workbook?.Names ?? []) {
    const sheetIndex = typeof definedName.Sheet === 'number' ? definedName.Sheet : undefined;
    const sheet = sheetIndex === undefined ? undefined : sheets[sheetIndex];
    if (definedName.Name.toLowerCase() === '_xlnm.print_area') {
      if (!sheet?.id) continue;
      const reference = normalizeSpreadsheetPrintArea(stripSpreadsheetSheetQualifier(definedName.Ref, sheet.name));
      if (reference) printAreas.push({ sheetId: sheet.id, reference });
      continue;
    }
    if (definedName.Name.toLowerCase() === '_xlnm.print_titles') {
      if (!sheet?.id) continue;
      const parsed = parseSpreadsheetPrintTitles(stripSpreadsheetSheetQualifier(definedName.Ref, sheet.name));
      if (!parsed) continue;
      const rows = parsed.rows
        ? normalizeSpreadsheetPrintTitleRows(`$${parsed.rows[0] + 1}:$${parsed.rows[1] + 1}`)
        : null;
      const columns = parsed.columns
        ? normalizeSpreadsheetPrintTitleColumns(
            `${encodeSpreadsheetColumn(parsed.columns[0])}:${encodeSpreadsheetColumn(parsed.columns[1])}`
          )
        : null;
      printTitles.push({
        sheetId: sheet.id,
        rows: rows ?? undefined,
        columns: columns ?? undefined,
      });
      continue;
    }
    if (/^_xlnm\./i.test(definedName.Name) || !definedName.Ref.trim()) continue;
    namedRanges.push({
      id: createWorkId('name'),
      name: definedName.Name,
      reference: definedName.Ref.replace(/^=/, ''),
      scopeSheetId: sheet?.id,
      comment: definedName.Comment,
    });
  }
  return {
    namedRanges: namedRanges.length ? namedRanges : undefined,
    printAreas: printAreas.length ? printAreas : undefined,
    printTitles: printTitles.length ? printTitles : undefined,
  };
}

export function exportXlsxDefinedNames(content: WorkSpreadsheetContent): DefinedName[] {
  const names: DefinedName[] = [];
  for (const namedRange of content.namedRanges ?? []) {
    if (!isValidSpreadsheetDefinedName(namedRange.name) || !namedRange.reference.trim()) continue;
    const sheetIndex = namedRange.scopeSheetId
      ? content.sheets.findIndex((sheet) => sheet.id === namedRange.scopeSheetId)
      : -1;
    if (namedRange.scopeSheetId && sheetIndex < 0) continue;
    const sheetName = sheetIndex >= 0 ? content.sheets[sheetIndex].name.slice(0, 31) || '工作表' : null;
    const reference =
      sheetName && !namedRange.reference.includes('!')
        ? qualifySpreadsheetRange(namedRange.reference.replace(/^=/, ''), sheetName)
        : namedRange.reference.replace(/^=/, '');
    names.push({
      Name: namedRange.name.trim(),
      Ref: reference,
      Sheet: sheetIndex >= 0 ? sheetIndex : undefined,
      Comment: namedRange.comment?.trim() || undefined,
    });
  }
  for (const printArea of content.printAreas ?? []) {
    const sheetIndex = content.sheets.findIndex((sheet) => sheet.id === printArea.sheetId);
    if (sheetIndex < 0) continue;
    const reference = normalizeSpreadsheetPrintArea(printArea.reference);
    if (!reference) continue;
    names.push({
      Name: '_xlnm.Print_Area',
      Ref: qualifySpreadsheetRange(reference, content.sheets[sheetIndex].name.slice(0, 31) || '工作表'),
      Sheet: sheetIndex,
    });
  }
  for (const printTitle of content.printTitles ?? []) {
    const sheetIndex = content.sheets.findIndex((sheet) => sheet.id === printTitle.sheetId);
    if (sheetIndex < 0) continue;
    const rows = printTitle.rows ? normalizeSpreadsheetPrintTitleRows(printTitle.rows) : null;
    const columns = printTitle.columns ? normalizeSpreadsheetPrintTitleColumns(printTitle.columns) : null;
    if (!rows && !columns) continue;
    names.push({
      Name: '_xlnm.Print_Titles',
      Ref: qualifySpreadsheetRange(
        [rows, columns].filter((reference): reference is string => Boolean(reference)).join(','),
        content.sheets[sheetIndex].name.slice(0, 31) || '工作表'
      ),
      Sheet: sheetIndex,
    });
  }
  return names;
}

function encodeSpreadsheetColumn(index: number): string {
  let value = index + 1;
  let label = '';
  while (value > 0) {
    value -= 1;
    label = String.fromCharCode(65 + (value % 26)) + label;
    value = Math.floor(value / 26);
  }
  return label;
}
