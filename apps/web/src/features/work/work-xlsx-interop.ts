import type { Sheet } from '@fortune-sheet/core';
import JSZip from 'jszip';
import { attribute, directChild, directChildren, firstDescendant, OoxmlPackage, parseXml } from './work-ooxml-package';
import { sheetHasProtectionState } from './work-spreadsheet-protection';
import {
  type FortuneConditionalFormatRule,
  readXlsxConditionalFormats,
  readXlsxDifferentialFormats,
  writeXlsxConditionalFormats,
  XlsxDifferentialFormatWriter,
} from './work-xlsx-conditional-format';
import {
  readXlsxProtection,
  writeXlsxProtection,
  XlsxCellProtectionWriter,
  type XlsxProtectionFeatures,
} from './work-xlsx-protection';
import {
  readXlsxManualPageBreaks,
  writeXlsxManualPageBreaks,
  type XlsxManualPageBreaks,
} from './work-xlsx-page-breaks';
import { readXlsxWorksheetCharts, type XlsxWorksheetChart } from './work-xlsx-charts';
import { readXlsxWorksheetImages, type XlsxWorksheetImage } from './work-xlsx-images';
import { readXlsxPageSetup, writeXlsxPageSetup, type XlsxPageSetup } from './work-xlsx-page-setup';
import type { WorkSpreadsheetContent } from './work-types';

type FrozenPane = NonNullable<Sheet['frozen']>;

export interface XlsxDataValidation {
  references: string[];
  item: FortuneDataValidationItem;
}

export interface XlsxSheetFeatures {
  frozen?: FrozenPane;
  validations: XlsxDataValidation[];
  conditionalFormats: FortuneConditionalFormatRule[];
  protection: XlsxProtectionFeatures;
  pageBreaks: XlsxManualPageBreaks;
  pageSetup?: XlsxPageSetup;
  images: XlsxWorksheetImage[];
  charts: XlsxWorksheetChart[];
}

export interface FortuneDataValidationItem {
  type: string;
  type2: string;
  rangeTxt: string;
  value1: string;
  value2: string;
  validity: string;
  remote: boolean;
  prohibitInput: boolean;
  hintShow: boolean;
  hintValue: string;
  checked?: boolean;
}

export async function readXlsxSheetFeatures(buffer: ArrayBuffer): Promise<Map<string, XlsxSheetFeatures>> {
  const archive = await OoxmlPackage.load(buffer);
  const worksheetParts = await readWorksheetParts(archive);
  const styles = archive.has('xl/styles.xml') ? await archive.xml('xl/styles.xml') : null;
  const differentialFormats = readXlsxDifferentialFormats(styles);
  const features = new Map<string, XlsxSheetFeatures>();
  const imageBudget = { bytes: 0 };
  for (const [sheetName, partPath] of worksheetParts) {
    if (!archive.has(partPath)) continue;
    const document = await archive.xml(partPath);
    features.set(sheetName, {
      frozen: parseFrozenPane(document) ?? undefined,
      validations: parseDataValidations(document),
      conditionalFormats: readXlsxConditionalFormats(document, differentialFormats),
      protection: readXlsxProtection(document, styles),
      pageBreaks: readXlsxManualPageBreaks(document),
      pageSetup: readXlsxPageSetup(document),
      images: await readXlsxWorksheetImages(archive, partPath, document, imageBudget),
      charts: await readXlsxWorksheetCharts(archive, partPath, document),
    });
  }
  return features;
}

export async function patchXlsxSheetFeatures(
  buffer: ArrayBuffer,
  content: WorkSpreadsheetContent
): Promise<ArrayBuffer> {
  const { sheets } = content;
  const pageBreaksBySheetId = new Map((content.pageBreaks ?? []).map((pageBreaks) => [pageBreaks.sheetId, pageBreaks]));
  const pageSetupsBySheetId = new Map((content.pageSetups ?? []).map((pageSetup) => [pageSetup.sheetId, pageSetup]));
  if (
    !sheets.some(
      (sheet) =>
        sheet.frozen ||
        Object.keys(sheet.dataVerification ?? {}).length ||
        sheet.luckysheet_conditionformat_save?.length ||
        sheetHasProtectionState(sheet) ||
        Boolean(
          sheet.id &&
            (pageBreaksBySheetId.get(sheet.id)?.rows?.length ?? 0) +
              (pageBreaksBySheetId.get(sheet.id)?.columns?.length ?? 0)
        ) ||
        Boolean(sheet.id && pageSetupsBySheetId.has(sheet.id))
    )
  ) {
    return buffer;
  }
  const archive = await OoxmlPackage.load(buffer);
  const worksheetParts = await readWorksheetParts(archive);
  const zip = await JSZip.loadAsync(buffer);
  const styles = archive.has('xl/styles.xml') ? await archive.xml('xl/styles.xml') : null;
  const differentialFormats = styles ? new XlsxDifferentialFormatWriter(styles) : undefined;
  const cellProtection = styles ? new XlsxCellProtectionWriter(styles) : undefined;

  for (const sheet of sheets) {
    const pageBreaks = sheet.id ? pageBreaksBySheetId.get(sheet.id) : undefined;
    const pageSetup = sheet.id ? pageSetupsBySheetId.get(sheet.id) : undefined;
    if (
      !sheet.frozen &&
      !Object.keys(sheet.dataVerification ?? {}).length &&
      !sheet.luckysheet_conditionformat_save?.length &&
      !sheetHasProtectionState(sheet) &&
      !(pageBreaks?.rows?.length || pageBreaks?.columns?.length) &&
      !pageSetup
    ) {
      continue;
    }
    const exportedName = sheet.name.slice(0, 31) || '工作表';
    const partPath = worksheetParts.get(exportedName);
    const entry = partPath ? zip.file(partPath) : null;
    if (!partPath || !entry) continue;
    const document = parseXml(await entry.async('text'), partPath);
    if (sheet.frozen) writeFrozenPane(document, sheet.frozen);
    writeDataValidations(document, sheet.dataVerification);
    writeXlsxConditionalFormats(document, sheet.luckysheet_conditionformat_save, differentialFormats);
    writeXlsxProtection(document, sheet, cellProtection);
    writeXlsxPageSetup(document, pageSetup);
    writeXlsxManualPageBreaks(document, pageBreaks);
    zip.file(partPath, new XMLSerializer().serializeToString(document));
  }
  if (differentialFormats?.changed || cellProtection?.changed) {
    zip.file('xl/styles.xml', differentialFormats?.serialize() ?? cellProtection?.serialize() ?? '');
  }

  return zip.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });
}

async function readWorksheetParts(archive: OoxmlPackage): Promise<Map<string, string>> {
  if (!archive.has('xl/workbook.xml')) return new Map();
  const workbook = await archive.xml('xl/workbook.xml');
  const relationships = await archive.relationships('xl/workbook.xml');
  const parts = new Map<string, string>();
  for (const sheet of firstDescendant(workbook, 'sheets')?.children ?? []) {
    if (!(sheet instanceof Element) || sheet.localName !== 'sheet') continue;
    const name = attribute(sheet, 'name');
    const relationship = relationships.get(attribute(sheet, 'r:id') ?? '');
    if (name && relationship?.type.endsWith('/worksheet')) parts.set(name, relationship.target);
  }
  return parts;
}

function parseFrozenPane(document: Document): FrozenPane | null {
  const pane = firstDescendant(document, 'pane');
  const state = attribute(pane ?? document.documentElement, 'state')?.toLowerCase();
  if (!pane || (state !== 'frozen' && state !== 'frozensplit')) return null;
  const columns = frozenCount(attribute(pane, 'xSplit'));
  const rows = frozenCount(attribute(pane, 'ySplit'));
  if (!rows && !columns) return null;
  return {
    type: rows && columns ? 'rangeBoth' : rows ? 'rangeRow' : 'rangeColumn',
    range: {
      row_focus: Math.max(0, rows - 1),
      column_focus: Math.max(0, columns - 1),
    },
  };
}

function parseDataValidations(document: Document): XlsxDataValidation[] {
  const validations = firstDescendant(document, 'dataValidations');
  if (!validations) return [];
  return directChildren(validations, 'dataValidation').flatMap((element) => {
    const type = fortuneValidationType(attribute(element, 'type'));
    const references = (attribute(element, 'sqref') ?? '')
      .split(/\s+/)
      .map((value) => value.trim())
      .filter(Boolean);
    if (!type || !references.length) return [];
    const formula1 = firstDescendant(element, 'formula1')?.textContent?.trim() ?? '';
    const formula2 = firstDescendant(element, 'formula2')?.textContent?.trim() ?? '';
    const item: FortuneDataValidationItem = {
      type,
      type2: fortuneValidationOperator(attribute(element, 'operator')),
      rangeTxt: references.join(','),
      value1: type === 'dropdown' ? parseListFormula(formula1) : formula1,
      value2: formula2,
      validity: '',
      remote: false,
      prohibitInput: booleanAttribute(element, 'showErrorMessage'),
      hintShow: booleanAttribute(element, 'showInputMessage'),
      hintValue: attribute(element, 'prompt') ?? '',
      checked: false,
    };
    return [{ references, item }];
  });
}

function writeDataValidations(document: Document, source: unknown): void {
  const root = document.documentElement;
  for (const existing of directChildren(root, 'dataValidations')) existing.remove();
  const grouped = groupDataValidations(source);
  if (!grouped.length) return;

  const namespace = root.namespaceURI;
  const container = document.createElementNS(namespace, 'dataValidations');
  container.setAttribute('count', String(grouped.length));
  for (const { item, references } of grouped) {
    const validationType = xlsxValidationType(item.type);
    if (!validationType) continue;
    const element = document.createElementNS(namespace, 'dataValidation');
    element.setAttribute('type', validationType);
    const operator = xlsxValidationOperator(item.type2);
    if (operator && validationType !== 'list') element.setAttribute('operator', operator);
    element.setAttribute('allowBlank', '1');
    element.setAttribute('showErrorMessage', item.prohibitInput ? '1' : '0');
    element.setAttribute('showInputMessage', item.hintShow ? '1' : '0');
    if (item.hintValue) element.setAttribute('prompt', item.hintValue.slice(0, 255));
    element.setAttribute('sqref', references.join(' '));
    appendFormula(document, element, 'formula1', xlsxValidationFormula(item));
    if (item.value2) appendFormula(document, element, 'formula2', item.value2);
    container.append(element);
  }
  if (!container.children.length) return;
  container.setAttribute('count', String(container.children.length));
  const anchor = directChildren(root).find((child) =>
    [
      'hyperlinks',
      'printOptions',
      'pageMargins',
      'pageSetup',
      'headerFooter',
      'drawing',
      'legacyDrawing',
      'ignoredErrors',
      'extLst',
    ].includes(child.localName)
  );
  root.insertBefore(container, anchor ?? null);
}

function groupDataValidations(source: unknown): Array<{ item: FortuneDataValidationItem; references: string[] }> {
  if (!source || typeof source !== 'object') return [];
  const groups = new Map<string, { item: FortuneDataValidationItem; references: string[] }>();
  for (const [key, value] of Object.entries(source)) {
    const match = /^(\d+)_(\d+)$/.exec(key);
    const item = fortuneDataValidationItem(value);
    if (!match || !item || !xlsxValidationType(item.type)) continue;
    const signature = JSON.stringify({
      type: item.type,
      type2: item.type2,
      value1: item.value1,
      value2: item.value2,
      prohibitInput: item.prohibitInput,
      hintShow: item.hintShow,
      hintValue: item.hintValue,
    });
    const group = groups.get(signature) ?? { item, references: [] };
    group.references.push(encodeCell(Number(match[1]), Number(match[2])));
    groups.set(signature, group);
  }
  return Array.from(groups.values());
}

function fortuneDataValidationItem(value: unknown): FortuneDataValidationItem | null {
  if (!value || typeof value !== 'object') return null;
  const item = value as Partial<FortuneDataValidationItem>;
  if (typeof item.type !== 'string') return null;
  return {
    type: item.type,
    type2: typeof item.type2 === 'string' ? item.type2 : '',
    rangeTxt: typeof item.rangeTxt === 'string' ? item.rangeTxt : '',
    value1: typeof item.value1 === 'string' ? item.value1 : String(item.value1 ?? ''),
    value2: typeof item.value2 === 'string' ? item.value2 : String(item.value2 ?? ''),
    validity: typeof item.validity === 'string' ? item.validity : '',
    remote: Boolean(item.remote),
    prohibitInput: Boolean(item.prohibitInput),
    hintShow: Boolean(item.hintShow),
    hintValue: typeof item.hintValue === 'string' ? item.hintValue : '',
    checked: Boolean(item.checked),
  };
}

function appendFormula(document: Document, parent: Element, name: string, value: string): void {
  if (!value) return;
  const formula = document.createElementNS(document.documentElement.namespaceURI, name);
  formula.textContent = value.replace(/^=/, '');
  parent.append(formula);
}

function xlsxValidationFormula(item: FortuneDataValidationItem): string {
  const value = item.value1.trim();
  if (item.type !== 'dropdown') return value;
  if (/^=?[^,]+![A-Z]+\d+(?::[A-Z]+\d+)?$/i.test(value) || /^=?\$?[A-Z]+\$?\d+(?::\$?[A-Z]+\$?\d+)?$/i.test(value)) {
    return value.replace(/^=/, '');
  }
  return `"${value.replaceAll('"', '""')}"`;
}

function parseListFormula(value: string): string {
  if (value.startsWith('"') && value.endsWith('"')) return value.slice(1, -1).replaceAll('""', '"');
  return value.replace(/^=/, '');
}

function fortuneValidationType(value: string | null): string | null {
  const types: Record<string, string> = {
    list: 'dropdown',
    whole: 'number_integer',
    decimal: 'number_decimal',
    textLength: 'text_length',
    date: 'date',
  };
  return value ? (types[value] ?? null) : null;
}

function xlsxValidationType(value: string): string | null {
  const types: Record<string, string> = {
    dropdown: 'list',
    number: 'decimal',
    number_integer: 'whole',
    number_decimal: 'decimal',
    text_length: 'textLength',
    date: 'date',
  };
  return types[value] ?? null;
}

function fortuneValidationOperator(value: string | null): string {
  const operators: Record<string, string> = {
    between: 'between',
    notBetween: 'notBetween',
    equal: 'equal',
    notEqual: 'notEqualTo',
    greaterThan: 'moreThanThe',
    lessThan: 'lessThan',
    greaterThanOrEqual: 'greaterOrEqualTo',
    lessThanOrEqual: 'lessThanOrEqualTo',
  };
  return value ? (operators[value] ?? '') : '';
}

function xlsxValidationOperator(value: string): string | null {
  const operators: Record<string, string> = {
    between: 'between',
    notBetween: 'notBetween',
    equal: 'equal',
    notEqualTo: 'notEqual',
    moreThanThe: 'greaterThan',
    lessThan: 'lessThan',
    greaterOrEqualTo: 'greaterThanOrEqual',
    lessThanOrEqualTo: 'lessThanOrEqual',
  };
  return operators[value] ?? null;
}

function booleanAttribute(element: Element, name: string): boolean {
  const value = attribute(element, name)?.toLowerCase();
  return value === '1' || value === 'true';
}

function writeFrozenPane(document: Document, frozen: FrozenPane): void {
  const root = document.documentElement;
  const namespace = root.namespaceURI;
  let sheetViews = directChild(root, 'sheetViews');
  if (!sheetViews) {
    sheetViews = document.createElementNS(namespace, 'sheetViews');
    const anchor = directChildren(root).find((child) =>
      ['sheetFormatPr', 'cols', 'sheetData', 'sheetCalcPr', 'sheetProtection'].includes(child.localName)
    );
    root.insertBefore(sheetViews, anchor ?? null);
  }
  let sheetView = directChild(sheetViews, 'sheetView');
  if (!sheetView) {
    sheetView = document.createElementNS(namespace, 'sheetView');
    sheetView.setAttribute('workbookViewId', '0');
    sheetViews.append(sheetView);
  }
  for (const existing of directChildren(sheetView, 'pane')) existing.remove();

  const { rows, columns } = frozenCounts(frozen);
  if (!rows && !columns) return;
  const activePane = rows && columns ? 'bottomRight' : rows ? 'bottomLeft' : 'topRight';
  const pane = document.createElementNS(namespace, 'pane');
  if (columns) pane.setAttribute('xSplit', String(columns));
  if (rows) pane.setAttribute('ySplit', String(rows));
  pane.setAttribute('topLeftCell', encodeCell(rows, columns));
  pane.setAttribute('activePane', activePane);
  pane.setAttribute('state', 'frozen');
  sheetView.insertBefore(pane, directChildren(sheetView)[0] ?? null);
  for (const selection of directChildren(sheetView, 'selection')) {
    selection.setAttribute('pane', activePane);
  }
}

function frozenCounts(frozen: FrozenPane): { rows: number; columns: number } {
  const freezesRows = ['row', 'both', 'rangeRow', 'rangeBoth'].includes(frozen.type);
  const freezesColumns = ['column', 'both', 'rangeColumn', 'rangeBoth'].includes(frozen.type);
  return {
    rows: freezesRows ? Math.max(1, (frozen.range?.row_focus ?? 0) + 1) : 0,
    columns: freezesColumns ? Math.max(1, (frozen.range?.column_focus ?? 0) + 1) : 0,
  };
}

function frozenCount(value: string | null): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? Math.max(1, Math.trunc(parsed)) : 0;
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
