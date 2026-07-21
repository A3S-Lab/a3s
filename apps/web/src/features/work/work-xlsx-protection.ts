import type { Sheet } from '@fortune-sheet/core';
import { attribute, descendants, directChild, directChildren, firstDescendant } from './work-ooxml-package';
import {
  DEFAULT_PROTECTION_HINT,
  editableRangeRequiresCredentials,
  normalizeSheetProtectionAuthority,
  type FortuneSheetEditableRange,
  type FortuneSheetProtectionAuthority,
  type SpreadsheetCellProtectionRange,
} from './work-spreadsheet-protection';
import {
  formatSpreadsheetCellRanges,
  parseSpreadsheetCellRanges,
  type SpreadsheetCellRange,
} from './work-spreadsheet-ranges';

interface CellProtectionStyle {
  locked: boolean;
  hidden: boolean;
}

export interface XlsxProtectionFeatures {
  authority?: FortuneSheetProtectionAuthority;
  cellProtectionRanges: SpreadsheetCellProtectionRange[];
}

export interface XlsxProtectionDiagnostic {
  code: string;
  message: string;
  severity: 'info' | 'warning';
}

export function readXlsxProtection(worksheet: Document, styles: Document | null): XlsxProtectionFeatures {
  const protectedRanges = readProtectedRanges(worksheet);
  const protection = directChild(worksheet.documentElement, 'sheetProtection');
  const authority =
    protection || protectedRanges.length
      ? {
          ...authorityFromElement(protection),
          allowRangeList: protectedRanges,
        }
      : undefined;
  return {
    authority,
    cellProtectionRanges: readCellProtectionRanges(worksheet, styles),
  };
}

export function writeXlsxProtection(worksheet: Document, sheet: Sheet, styles?: XlsxCellProtectionWriter): void {
  const root = worksheet.documentElement;
  for (const element of directChildren(root, 'sheetProtection')) element.remove();
  for (const element of directChildren(root, 'protectedRanges')) element.remove();

  const authority = normalizeSheetProtectionAuthority(sheet.config?.authority);
  if (authority.sheet === 1) {
    const element = worksheet.createElementNS(root.namespaceURI, 'sheetProtection');
    copyAttributes(element, authority.xlsxAttributes);
    writeProtectionAttributes(element, authority);
    insertWorksheetFeature(root, element, [
      'protectedRanges',
      'scenarios',
      'autoFilter',
      'sortState',
      'dataConsolidate',
      'customSheetViews',
      'mergeCells',
      'phoneticPr',
      'conditionalFormatting',
      'dataValidations',
      'hyperlinks',
      'printOptions',
      'pageMargins',
      'pageSetup',
      'headerFooter',
      'drawing',
      'legacyDrawing',
      'ignoredErrors',
      'extLst',
    ]);
  }

  writeProtectedRanges(worksheet, authority.allowRangeList);
  if (styles) writeCellProtectionStyles(worksheet, sheet, styles);
}

export class XlsxCellProtectionWriter {
  private readonly cellXfs: Element;
  private readonly generated = new Map<string, number>();
  changed = false;

  constructor(private readonly styles: Document) {
    const root = styles.documentElement;
    this.cellXfs = directChild(root, 'cellXfs') ?? styles.createElementNS(root.namespaceURI, 'cellXfs');
    if (!this.cellXfs.parentElement) {
      const anchor = directChildren(root).find((child) =>
        ['cellStyles', 'dxfs', 'tableStyles', 'colors', 'extLst'].includes(child.localName)
      );
      root.insertBefore(this.cellXfs, anchor ?? null);
    }
    if (!directChildren(this.cellXfs, 'xf').length) {
      const base = styles.createElementNS(root.namespaceURI, 'xf');
      base.setAttribute('numFmtId', '0');
      base.setAttribute('fontId', '0');
      base.setAttribute('fillId', '0');
      base.setAttribute('borderId', '0');
      base.setAttribute('xfId', '0');
      this.cellXfs.append(base);
      this.changed = true;
    }
    this.updateCount();
  }

  styleId(baseStyleId: number, locked: boolean, hidden: boolean): number {
    const styles = directChildren(this.cellXfs, 'xf');
    const baseIndex = Number.isInteger(baseStyleId) && styles[baseStyleId] ? baseStyleId : 0;
    if (sameProtection(readProtectionStyle(styles[baseIndex]), { locked, hidden })) return baseIndex;
    const key = `${baseIndex}:${locked ? 1 : 0}:${hidden ? 1 : 0}`;
    const cached = this.generated.get(key);
    if (cached !== undefined) return cached;

    const clone = styles[baseIndex].cloneNode(true) as Element;
    for (const element of directChildren(clone, 'protection')) element.remove();
    clone.setAttribute('applyProtection', '1');
    const protection = this.styles.createElementNS(this.styles.documentElement.namespaceURI, 'protection');
    protection.setAttribute('locked', locked ? '1' : '0');
    protection.setAttribute('hidden', hidden ? '1' : '0');
    clone.insertBefore(protection, directChild(clone, 'extLst') ?? null);
    const index = styles.length;
    this.cellXfs.append(clone);
    this.generated.set(key, index);
    this.changed = true;
    this.updateCount();
    return index;
  }

  serialize(): string {
    return new XMLSerializer().serializeToString(this.styles);
  }

  private updateCount() {
    this.cellXfs.setAttribute('count', String(directChildren(this.cellXfs, 'xf').length));
  }
}

export function diagnoseXlsxProtection(worksheet: Document, styles: Document | null): XlsxProtectionDiagnostic[] {
  const protection = directChild(worksheet.documentElement, 'sheetProtection');
  const protectedRanges = readProtectedRanges(worksheet);
  if (!protection && !protectedRanges.length) return [];
  const diagnostics: XlsxProtectionDiagnostic[] = [
    {
      code: 'xlsx.protection',
      message:
        'Sheet lock state, locked and unlocked cells, selection rules, and passwordless editable ranges are preserved and enforced.',
      severity: 'info',
    },
  ];
  const sourceAttributes = protection ? attributes(protection) : {};
  if (hasCredentialAttributes(sourceAttributes)) {
    diagnostics.push({
      code: 'xlsx.protection.password',
      message:
        'The source password verifier is preserved, but Work cannot authenticate it; locked cells remain protected.',
      severity: 'warning',
    });
  }
  if (protectedRanges.some(editableRangeRequiresCredentials)) {
    diagnostics.push({
      code: 'xlsx.protection.range-credentials',
      message:
        'Password- or permission-backed editable ranges are preserved but remain locked because Work cannot authenticate their credentials.',
      severity: 'warning',
    });
  }
  if (protection) {
    diagnostics.push({
      code: 'xlsx.protection.permissions',
      message:
        'Action-specific protection permissions are preserved; Work enforces cell editing and selection but not every Excel row, column, object, filter, or pivot permission.',
      severity: 'warning',
    });
  }
  if (hasHiddenFormula(worksheet, styles)) {
    diagnostics.push({
      code: 'xlsx.protection.hidden-formulas',
      message:
        'Formula-hidden protection is preserved for XLSX export, but formulas can still appear in the Work formula bar.',
      severity: 'warning',
    });
  }
  return diagnostics;
}

function authorityFromElement(element: Element | undefined): FortuneSheetProtectionAuthority {
  const defaults = normalizeSheetProtectionAuthority(undefined);
  if (!element) return defaults;
  const source = attributes(element);
  return {
    ...defaults,
    sheet: booleanAttribute(element, 'sheet', false) ? 1 : 0,
    selectLockedCells: booleanAttribute(element, 'selectLockedCells', false) ? 0 : 1,
    selectunLockedCells: booleanAttribute(element, 'selectUnlockedCells', false) ? 0 : 1,
    formatCells: booleanAttribute(element, 'formatCells', true) ? 0 : 1,
    formatColumns: booleanAttribute(element, 'formatColumns', true) ? 0 : 1,
    formatRows: booleanAttribute(element, 'formatRows', true) ? 0 : 1,
    insertColumns: booleanAttribute(element, 'insertColumns', true) ? 0 : 1,
    insertRows: booleanAttribute(element, 'insertRows', true) ? 0 : 1,
    insertHyperlinks: booleanAttribute(element, 'insertHyperlinks', true) ? 0 : 1,
    deleteColumns: booleanAttribute(element, 'deleteColumns', true) ? 0 : 1,
    deleteRows: booleanAttribute(element, 'deleteRows', true) ? 0 : 1,
    sort: booleanAttribute(element, 'sort', true) ? 0 : 1,
    filter: booleanAttribute(element, 'autoFilter', true) ? 0 : 1,
    usePivotTablereports: booleanAttribute(element, 'pivotTables', true) ? 0 : 1,
    editObjects: booleanAttribute(element, 'objects', false) ? 0 : 1,
    editScenarios: booleanAttribute(element, 'scenarios', false) ? 0 : 1,
    defaultSheetHintText: DEFAULT_PROTECTION_HINT,
    xlsxAttributes: Object.keys(source).length ? source : undefined,
  };
}

function readProtectedRanges(worksheet: Document): FortuneSheetEditableRange[] {
  const container = directChild(worksheet.documentElement, 'protectedRanges');
  if (!container) return [];
  return directChildren(container, 'protectedRange').flatMap((element, index) => {
    const ranges = parseSqref(attribute(element, 'sqref'));
    if (!ranges.length) return [];
    return [
      {
        name: attribute(element, 'name')?.trim() || `Range ${index + 1}`,
        sqref: formatSpreadsheetCellRanges(ranges),
        xlsxAttributes: attributes(element),
      },
    ];
  });
}

function readCellProtectionRanges(worksheet: Document, styles: Document | null): SpreadsheetCellProtectionRange[] {
  const xfs = styles ? directChildren(firstDescendant(styles, 'cellXfs') ?? styles.documentElement, 'xf') : [];
  if (!xfs.length) return [];
  const styleList = xfs.map(readProtectionStyle);
  const bounds = worksheetBounds(worksheet);
  const ranges: SpreadsheetCellProtectionRange[] = [];
  if (!sameProtection(styleList[0], { locked: true, hidden: false })) {
    ranges.push({ range: bounds, ...styleList[0] });
  }
  for (const column of descendants(worksheet, 'col')) {
    const style = indexedStyle(styleList, attribute(column, 'style'));
    const minimum = positiveInteger(attribute(column, 'min'));
    const maximum = positiveInteger(attribute(column, 'max'));
    if (!style || minimum === null || maximum === null) continue;
    ranges.push({
      range: { row: [...bounds.row], column: [minimum - 1, maximum - 1] },
      ...style,
    });
  }
  for (const row of descendants(worksheet, 'row')) {
    const style = indexedStyle(styleList, attribute(row, 's'));
    const rowNumber = positiveInteger(attribute(row, 'r'));
    if (!style || rowNumber === null) continue;
    ranges.push({
      range: { row: [rowNumber - 1, rowNumber - 1], column: [...bounds.column] },
      ...style,
    });
  }
  for (const cell of descendants(worksheet, 'c')) {
    const style = indexedStyle(styleList, attribute(cell, 's'));
    const coordinate = parseCellReference(attribute(cell, 'r'));
    if (!style || !coordinate) continue;
    ranges.push({
      range: { row: [coordinate.row, coordinate.row], column: [coordinate.column, coordinate.column] },
      ...style,
    });
  }
  return ranges;
}

function writeProtectionAttributes(element: Element, authority: FortuneSheetProtectionAuthority): void {
  element.setAttribute('sheet', '1');
  element.setAttribute('selectLockedCells', authority.selectLockedCells ? '0' : '1');
  element.setAttribute('selectUnlockedCells', authority.selectunLockedCells ? '0' : '1');
  const mappings: Array<[keyof FortuneSheetProtectionAuthority, string]> = [
    ['formatCells', 'formatCells'],
    ['formatColumns', 'formatColumns'],
    ['formatRows', 'formatRows'],
    ['insertColumns', 'insertColumns'],
    ['insertRows', 'insertRows'],
    ['insertHyperlinks', 'insertHyperlinks'],
    ['deleteColumns', 'deleteColumns'],
    ['deleteRows', 'deleteRows'],
    ['sort', 'sort'],
    ['filter', 'autoFilter'],
    ['usePivotTablereports', 'pivotTables'],
    ['editObjects', 'objects'],
    ['editScenarios', 'scenarios'],
  ];
  for (const [key, name] of mappings) element.setAttribute(name, authority[key] ? '0' : '1');
}

function writeProtectedRanges(document: Document, ranges: FortuneSheetEditableRange[]): void {
  const root = document.documentElement;
  const container = document.createElementNS(root.namespaceURI, 'protectedRanges');
  for (const range of ranges) {
    const parsed = parseSpreadsheetCellRanges(range.sqref);
    if (!range.name.trim() || !parsed?.length) continue;
    const element = document.createElementNS(root.namespaceURI, 'protectedRange');
    copyAttributes(element, range.xlsxAttributes);
    element.setAttribute('name', range.name.trim());
    element.setAttribute('sqref', formatSpreadsheetCellRanges(parsed).replaceAll(',', ' '));
    container.append(element);
  }
  if (!container.children.length) return;
  insertWorksheetFeature(root, container, [
    'scenarios',
    'autoFilter',
    'sortState',
    'dataConsolidate',
    'customSheetViews',
    'mergeCells',
    'phoneticPr',
    'conditionalFormatting',
    'dataValidations',
    'hyperlinks',
    'printOptions',
    'pageMargins',
    'pageSetup',
    'headerFooter',
    'drawing',
    'legacyDrawing',
    'ignoredErrors',
    'extLst',
  ]);
}

function writeCellProtectionStyles(worksheet: Document, sheet: Sheet, styles: XlsxCellProtectionWriter): void {
  for (const [row, values] of (sheet.data ?? []).entries()) {
    for (const [column, source] of values.entries()) {
      const cell = source as (typeof source & { hi?: number }) | null;
      if (!cell || (cell.lo === undefined && cell.hi === undefined)) continue;
      const element = ensureCellElement(worksheet, row, column);
      const baseStyle = nonNegativeInteger(attribute(element, 's')) ?? 0;
      const styleId = styles.styleId(baseStyle, cell.lo !== 0, cell.hi === 1);
      if (styleId) element.setAttribute('s', String(styleId));
      else element.removeAttribute('s');
    }
  }
}

function ensureCellElement(document: Document, rowIndex: number, columnIndex: number): Element {
  const sheetData = directChild(document.documentElement, 'sheetData');
  if (!sheetData) throw new Error('XLSX worksheet does not contain sheetData');
  const rowNumber = rowIndex + 1;
  let row = directChildren(sheetData, 'row').find((element) => attribute(element, 'r') === String(rowNumber));
  if (!row) {
    row = document.createElementNS(document.documentElement.namespaceURI, 'row');
    row.setAttribute('r', String(rowNumber));
    const anchor = directChildren(sheetData, 'row').find(
      (element) => (positiveInteger(attribute(element, 'r')) ?? Number.MAX_SAFE_INTEGER) > rowNumber
    );
    sheetData.insertBefore(row, anchor ?? null);
  }
  const reference = encodeCell(rowIndex, columnIndex);
  let cell = directChildren(row, 'c').find((element) => attribute(element, 'r') === reference);
  if (!cell) {
    cell = document.createElementNS(document.documentElement.namespaceURI, 'c');
    cell.setAttribute('r', reference);
    const anchor = directChildren(row, 'c').find((element) => {
      const coordinate = parseCellReference(attribute(element, 'r'));
      return coordinate ? coordinate.column > columnIndex : false;
    });
    row.insertBefore(cell, anchor ?? null);
    expandWorksheetDimension(document, rowIndex, columnIndex);
  }
  return cell;
}

function worksheetBounds(document: Document): SpreadsheetCellRange {
  const reference = attribute(directChild(document.documentElement, 'dimension') ?? document.documentElement, 'ref');
  const parsed = reference ? parseSpreadsheetCellRanges(reference)?.[0] : undefined;
  if (parsed) {
    return {
      row: [0, parsed.row[1]],
      column: [0, parsed.column[1]],
    };
  }
  const coordinates = descendants(document, 'c')
    .map((cell) => parseCellReference(attribute(cell, 'r')))
    .filter((value): value is { row: number; column: number } => Boolean(value));
  return {
    row: [0, Math.max(0, ...coordinates.map((value) => value.row))],
    column: [0, Math.max(0, ...coordinates.map((value) => value.column))],
  };
}

function expandWorksheetDimension(document: Document, row: number, column: number): void {
  const root = document.documentElement;
  let dimension = directChild(root, 'dimension');
  const bounds = worksheetBounds(document);
  bounds.row[1] = Math.max(bounds.row[1], row);
  bounds.column[1] = Math.max(bounds.column[1], column);
  if (!dimension) {
    dimension = document.createElementNS(root.namespaceURI, 'dimension');
    root.insertBefore(dimension, directChildren(root)[0] ?? null);
  }
  dimension.setAttribute(
    'ref',
    `${encodeCell(bounds.row[0], bounds.column[0])}:${encodeCell(bounds.row[1], bounds.column[1])}`
  );
}

function readProtectionStyle(xf: Element): CellProtectionStyle {
  const protection = directChild(xf, 'protection');
  if (!protection || booleanAttribute(xf, 'applyProtection', true) === false) {
    return { locked: true, hidden: false };
  }
  return {
    locked: booleanAttribute(protection, 'locked', true),
    hidden: booleanAttribute(protection, 'hidden', false),
  };
}

function hasHiddenFormula(worksheet: Document, styles: Document | null): boolean {
  const formulas = descendants(worksheet, 'c').filter((cell) => Boolean(directChild(cell, 'f')));
  if (!formulas.length) return false;
  const ranges = readCellProtectionRanges(worksheet, styles);
  return formulas.some((cell) => {
    const coordinate = parseCellReference(attribute(cell, 'r'));
    if (!coordinate) return false;
    let hidden = false;
    for (const item of ranges) {
      if (
        coordinate.row >= item.range.row[0] &&
        coordinate.row <= item.range.row[1] &&
        coordinate.column >= item.range.column[0] &&
        coordinate.column <= item.range.column[1]
      ) {
        hidden = item.hidden;
      }
    }
    return hidden;
  });
}

function insertWorksheetFeature(root: Element, element: Element, anchors: string[]): void {
  root.insertBefore(element, directChildren(root).find((child) => anchors.includes(child.localName)) ?? null);
}

function parseSqref(value: string | null): SpreadsheetCellRange[] {
  return value ? (parseSpreadsheetCellRanges(value.trim().replace(/\s+/g, ',')) ?? []) : [];
}

function indexedStyle(styles: CellProtectionStyle[], value: string | null): CellProtectionStyle | null {
  const index = nonNegativeInteger(value);
  return index === null ? null : (styles[index] ?? null);
}

function parseCellReference(value: string | null): { row: number; column: number } | null {
  const match = /^\$?([A-Z]{1,3})\$?([1-9]\d*)$/i.exec(value ?? '');
  if (!match) return null;
  let column = 0;
  for (const character of match[1].toUpperCase()) column = column * 26 + character.charCodeAt(0) - 64;
  return { row: Number(match[2]) - 1, column: column - 1 };
}

function encodeCell(row: number, column: number): string {
  let value = column + 1;
  let label = '';
  while (value > 0) {
    value -= 1;
    label = String.fromCharCode(65 + (value % 26)) + label;
    value = Math.floor(value / 26);
  }
  return `${label}${row + 1}`;
}

function attributes(element: Element): Record<string, string> {
  return Object.fromEntries(Array.from(element.attributes).map((item) => [item.name, item.value]));
}

function copyAttributes(element: Element, source: Record<string, string> | undefined): void {
  for (const [name, value] of Object.entries(source ?? {})) {
    if (/^[A-Za-z_][A-Za-z0-9_.-]*$/.test(name)) element.setAttribute(name, value);
  }
}

function hasCredentialAttributes(source: Record<string, string>): boolean {
  return Boolean(source.password || source.hashValue || source.saltValue || source.securityDescriptor);
}

function booleanAttribute(element: Element, name: string, fallback: boolean): boolean {
  const value = attribute(element, name)?.toLowerCase();
  if (value === '1' || value === 'true' || value === 'on') return true;
  if (value === '0' || value === 'false' || value === 'off') return false;
  return fallback;
}

function nonNegativeInteger(value: string | null): number | null {
  if (value === null || !/^\d+$/.test(value)) return null;
  return Number(value);
}

function positiveInteger(value: string | null): number | null {
  const parsed = nonNegativeInteger(value);
  return parsed !== null && parsed > 0 ? parsed : null;
}

function sameProtection(left: CellProtectionStyle, right: CellProtectionStyle): boolean {
  return left.locked === right.locked && left.hidden === right.hidden;
}
