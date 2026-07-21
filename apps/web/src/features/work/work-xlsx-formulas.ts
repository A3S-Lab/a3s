import type { Cell } from '@fortune-sheet/core';
import JSZip from 'jszip';
import type { WorkSheet } from 'xlsx';
import {
  attribute,
  descendants,
  directChild,
  directChildren,
  firstDescendant,
  OoxmlPackage,
  parseXml,
} from './work-ooxml-package';
import {
  editableSpreadsheetFormula,
  effectiveSpreadsheetCalculationSettings,
  spreadsheetCellAddress,
  spreadsheetFormulaForXlsx,
  spreadsheetFormulaRangeConflict,
  spreadsheetFormulaRangeForCell,
} from './work-spreadsheet-formulas';
import type {
  WorkSpreadsheetCalculationMode,
  WorkSpreadsheetCalculationSettings,
  WorkSpreadsheetContent,
  WorkSpreadsheetDataTableOptions,
  WorkSpreadsheetFormulaMetadata,
  WorkSpreadsheetFormulaRange,
  WorkSpreadsheetSheet,
} from './work-types';

export interface XlsxFormulaCell {
  address: string;
  formula: string;
  type: string;
  cachedError?: string;
}

export interface XlsxFormulaSheetFeatures {
  formulas: XlsxFormulaCell[];
  ranges: WorkSpreadsheetFormulaRange[];
  sourceFormulas: Record<string, string>;
  sharedFormulaGroups: number;
  sharedFormulaCells: number;
  unsupportedFormulaAttributes: string[];
}

export interface XlsxFormulaFeatures {
  calculation: WorkSpreadsheetCalculationSettings;
  sheets: Map<string, XlsxFormulaSheetFeatures>;
  unsupportedCalculationAttributes: string[];
}

type XlsxFormulaCellObject = {
  f: string;
  v?: Cell['v'];
  t?: string;
  F?: string;
  D?: boolean;
};

export async function readXlsxFormulaFeatures(buffer: ArrayBuffer): Promise<XlsxFormulaFeatures> {
  return readXlsxFormulaFeaturesFromPackage(await OoxmlPackage.load(buffer));
}

export async function readXlsxFormulaFeaturesFromPackage(archive: OoxmlPackage): Promise<XlsxFormulaFeatures> {
  if (!archive.has('xl/workbook.xml')) {
    return {
      calculation: effectiveSpreadsheetCalculationSettings(undefined),
      sheets: new Map(),
      unsupportedCalculationAttributes: [],
    };
  }
  const workbook = await archive.xml('xl/workbook.xml');
  const worksheetParts = await readWorksheetParts(archive, workbook);
  const dynamicMetadata = await readDynamicArrayMetadata(archive);
  const sheets = new Map<string, XlsxFormulaSheetFeatures>();
  for (const [sheetName, partPath] of worksheetParts) {
    if (!archive.has(partPath)) continue;
    sheets.set(sheetName, readWorksheetFormulaFeatures(await archive.xml(partPath), dynamicMetadata));
  }
  return {
    calculation: readCalculationSettings(workbook),
    sheets,
    unsupportedCalculationAttributes: readUnsupportedCalculationAttributes(workbook),
  };
}

export function createSpreadsheetFormulaMetadata(
  worksheet: WorkSheet,
  features: XlsxFormulaSheetFeatures | undefined
): WorkSpreadsheetFormulaMetadata | undefined {
  const sourceFormulas: Record<string, string> = {};
  const rangeByReference = new Map(
    (features?.ranges ?? []).map((range) => [range.reference.toUpperCase().replaceAll('$', ''), range])
  );

  for (const [address, value] of Object.entries(worksheet)) {
    if (address.startsWith('!') || !value || typeof value !== 'object') continue;
    const cell = value as { f?: string; F?: string; D?: boolean };
    if (cell.f) {
      const source = features?.sourceFormulas[address.toUpperCase()] ?? cell.f;
      if (editableSpreadsheetFormula(source) !== source) sourceFormulas[address.toUpperCase()] = source;
    }
    if (!cell.f || !cell.F) continue;
    const reference = cell.F.toUpperCase().replaceAll('$', '');
    const existing = rangeByReference.get(reference);
    if (existing) {
      if (cell.D && existing.type === 'array') {
        rangeByReference.set(reference, { ...existing, type: 'dynamic-array' });
      }
      continue;
    }
    rangeByReference.set(reference, {
      type: cell.D ? 'dynamic-array' : 'array',
      anchor: address.toUpperCase(),
      reference: cell.F,
      formula: features?.sourceFormulas[address.toUpperCase()] ?? cell.f,
    });
  }

  const ranges = Array.from(rangeByReference.values());
  const metadata: WorkSpreadsheetFormulaMetadata = {
    ranges: ranges.length ? ranges : undefined,
    sourceFormulas: Object.keys(sourceFormulas).length ? sourceFormulas : undefined,
    normalizedSharedFormulaGroups: features?.sharedFormulaGroups || undefined,
    normalizedSharedFormulaCells: features?.sharedFormulaCells || undefined,
  };
  return Object.values(metadata).some((value) => value !== undefined) ? metadata : undefined;
}

export function createXlsxFormulaCell(
  cell: Cell,
  row: number,
  column: number,
  sheet: WorkSpreadsheetSheet
): XlsxFormulaCellObject {
  const address = spreadsheetCellAddress(row, column);
  const range = spreadsheetFormulaRangeForCell(sheet, row, column);
  const sourceFormula = sheet.formulaMetadata?.sourceFormulas?.[address] ?? range?.formula;
  const result: XlsxFormulaCellObject = {
    f: spreadsheetFormulaForXlsx(cell.f ?? '', sourceFormula),
    v: cell.ct?.t === 'e' ? xlsxErrorValue(cell.v, cell.m) : cell.v,
  };
  if (cell.ct?.t) result.t = cell.ct.t;
  if (range && range.type !== 'data-table' && !spreadsheetFormulaRangeConflict(sheet, range)) {
    result.F = range.reference;
    if (range.type === 'dynamic-array') result.D = true;
  }
  return result;
}

export function createXlsxErrorCell(cell: Cell): { t: 'e'; v?: Cell['v'] } {
  return {
    t: 'e',
    v: xlsxErrorValue(cell.v, cell.m),
  };
}

export async function patchXlsxFormulaFeatures(
  buffer: ArrayBuffer,
  content: WorkSpreadsheetContent
): Promise<ArrayBuffer> {
  const archive = await OoxmlPackage.load(buffer);
  const zip = await JSZip.loadAsync(buffer);
  if (archive.has('xl/workbook.xml')) {
    const entry = zip.file('xl/workbook.xml');
    if (entry) {
      const workbook = parseXml(await entry.async('text'), 'xl/workbook.xml');
      writeCalculationSettings(workbook, content.calculation);
      zip.file('xl/workbook.xml', new XMLSerializer().serializeToString(workbook));
    }
  }

  const dataTables = content.sheets.flatMap((sheet) =>
    (sheet.formulaMetadata?.ranges ?? []).flatMap((range) =>
      range.type === 'data-table' && !spreadsheetFormulaRangeConflict(sheet, range) ? [{ sheet, range }] : []
    )
  );
  if (!dataTables.length || !archive.has('xl/workbook.xml')) {
    return zip.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });
  }

  const workbook = await archive.xml('xl/workbook.xml');
  const worksheetParts = await readWorksheetParts(archive, workbook);
  for (const sheet of content.sheets) {
    const ranges = dataTables.filter((item) => item.sheet === sheet).map((item) => item.range);
    if (!ranges.length) continue;
    const partPath = worksheetParts.get(sheet.name.slice(0, 31) || '工作表');
    const entry = partPath ? zip.file(partPath) : null;
    if (!partPath || !entry) continue;
    const document = parseXml(await entry.async('text'), partPath);
    for (const range of ranges) writeDataTableFormula(document, range);
    zip.file(partPath, new XMLSerializer().serializeToString(document));
  }
  return zip.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });
}

function readWorksheetFormulaFeatures(
  document: Document,
  dynamicMetadataIndexes: ReadonlySet<number>
): XlsxFormulaSheetFeatures {
  const formulas: XlsxFormulaCell[] = [];
  const ranges: WorkSpreadsheetFormulaRange[] = [];
  const sourceFormulas: Record<string, string> = {};
  const sharedGroups = new Set<string>();
  const unsupportedFormulaAttributes = new Set<string>();
  let sharedFormulaCells = 0;

  for (const cell of descendants(document, 'c')) {
    const formula = directChild(cell, 'f');
    if (!formula) continue;
    const address = (attribute(cell, 'r') ?? '').toUpperCase();
    if (!address) continue;
    const type = attribute(formula, 't') ?? 'normal';
    for (const name of unsupportedFormulaAttributesForElement(formula, type)) {
      unsupportedFormulaAttributes.add(name);
    }
    const source = formula.textContent ?? '';
    if (source) sourceFormulas[address] = source;
    const cachedError = attribute(cell, 't') === 'e' ? (directChild(cell, 'v')?.textContent ?? '') : undefined;
    formulas.push({ address, formula: source, type, cachedError });

    if (type === 'shared') {
      sharedFormulaCells += 1;
      sharedGroups.add(attribute(formula, 'si') ?? `cell:${address}`);
      continue;
    }
    const reference = attribute(formula, 'ref');
    if (type === 'array' && reference) {
      const metadataIndex = positiveInteger(attribute(cell, 'cm'));
      ranges.push({
        type: metadataIndex && dynamicMetadataIndexes.has(metadataIndex) ? 'dynamic-array' : 'array',
        anchor: address,
        reference,
        formula: source || undefined,
      });
    } else if (type === 'dataTable') {
      ranges.push({
        type: 'data-table',
        anchor: address,
        reference: reference || address,
        formula: source || undefined,
        dataTable: readDataTableOptions(formula),
      });
    }
  }
  return {
    formulas,
    ranges,
    sourceFormulas,
    sharedFormulaGroups: sharedGroups.size,
    sharedFormulaCells,
    unsupportedFormulaAttributes: Array.from(unsupportedFormulaAttributes).sort(),
  };
}

function readCalculationSettings(workbook: Document): WorkSpreadsheetCalculationSettings {
  const calculation = directChild(workbook.documentElement, 'calcPr');
  return effectiveSpreadsheetCalculationSettings({
    mode: calculationMode(attribute(calculation ?? workbook.documentElement, 'calcMode')),
    fullCalculationOnLoad: booleanAttribute(calculation, 'fullCalcOnLoad', false),
    forceFullCalculation: booleanAttribute(calculation, 'forceFullCalc', false),
    iterativeCalculation: booleanAttribute(calculation, 'iterate', false),
    maximumIterations: numericAttribute(calculation, 'iterateCount', 100),
    maximumChange: numericAttribute(calculation, 'iterateDelta', 0.001),
    fullPrecision: booleanAttribute(calculation, 'fullPrecision', true),
  });
}

function readUnsupportedCalculationAttributes(workbook: Document): string[] {
  const calculation = directChild(workbook.documentElement, 'calcPr');
  if (!calculation) return [];
  const supported = new Set([
    'calcId',
    'calcMode',
    'forceFullCalc',
    'fullCalcOnLoad',
    'fullPrecision',
    'iterate',
    'iterateCount',
    'iterateDelta',
  ]);
  return Array.from(calculation.attributes)
    .map((item) => item.localName)
    .filter((name) => !supported.has(name))
    .sort();
}

function writeCalculationSettings(workbook: Document, source: WorkSpreadsheetCalculationSettings | undefined): void {
  const settings = effectiveSpreadsheetCalculationSettings(source);
  const root = workbook.documentElement;
  let calculation = directChild(root, 'calcPr');
  if (!calculation) {
    calculation = workbook.createElementNS(root.namespaceURI, 'calcPr');
    root.insertBefore(calculation, directChild(root, 'extLst') ?? null);
  }
  calculation.setAttribute('calcMode', xlsxCalculationMode(settings.mode));
  calculation.setAttribute('fullCalcOnLoad', booleanValue(settings.fullCalculationOnLoad));
  calculation.setAttribute('forceFullCalc', booleanValue(settings.forceFullCalculation));
  calculation.setAttribute('iterate', booleanValue(settings.iterativeCalculation));
  calculation.setAttribute('iterateCount', String(settings.maximumIterations));
  calculation.setAttribute('iterateDelta', String(settings.maximumChange));
  calculation.setAttribute('fullPrecision', booleanValue(settings.fullPrecision));
}

async function readWorksheetParts(archive: OoxmlPackage, workbook: Document): Promise<Map<string, string>> {
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

async function readDynamicArrayMetadata(archive: OoxmlPackage): Promise<Set<number>> {
  if (!archive.has('xl/metadata.xml')) return new Set();
  const metadata = await archive.xml('xl/metadata.xml');
  const metadataTypes = directChildren(firstDescendant(metadata, 'metadataTypes') ?? metadata.documentElement);
  const dynamicTypeIndexes = new Set(
    metadataTypes.flatMap((item, index) =>
      item.localName === 'metadataType' && attribute(item, 'name')?.toUpperCase() === 'XLDAPR' ? [index + 1] : []
    )
  );
  if (!dynamicTypeIndexes.size) return new Set();
  const cellMetadata = firstDescendant(metadata, 'cellMetadata');
  if (!cellMetadata) return new Set();
  return new Set(
    directChildren(cellMetadata, 'bk').flatMap((block, index) =>
      directChildren(block, 'rc').some((record) =>
        dynamicTypeIndexes.has(positiveInteger(attribute(record, 't')) ?? -1)
      )
        ? [index + 1]
        : []
    )
  );
}

function readDataTableOptions(formula: Element): WorkSpreadsheetDataTableOptions | undefined {
  const options: WorkSpreadsheetDataTableOptions = {
    input1Reference: attribute(formula, 'r1') ?? undefined,
    input2Reference: attribute(formula, 'r2') ?? undefined,
    twoDimensional: optionalBooleanAttribute(formula, 'dt2D'),
    rowOriented: optionalBooleanAttribute(formula, 'dtr'),
    input1Deleted: optionalBooleanAttribute(formula, 'del1'),
    input2Deleted: optionalBooleanAttribute(formula, 'del2'),
    calculateOnLoad: optionalBooleanAttribute(formula, 'ca'),
  };
  return Object.values(options).some((value) => value !== undefined) ? options : undefined;
}

function unsupportedFormulaAttributesForElement(formula: Element, type: string): string[] {
  const common = ['t'];
  const supported =
    type === 'shared'
      ? new Set([...common, 'ref', 'si'])
      : type === 'array'
        ? new Set([...common, 'ref'])
        : type === 'dataTable'
          ? new Set([...common, 'ca', 'del1', 'del2', 'dt2D', 'dtr', 'r1', 'r2', 'ref'])
          : new Set(common);
  return Array.from(formula.attributes)
    .map((item) => item.localName)
    .filter((name) => !supported.has(name));
}

function writeDataTableFormula(document: Document, range: WorkSpreadsheetFormulaRange): void {
  const anchor = range.anchor.toUpperCase().replaceAll('$', '');
  const cell = descendants(document, 'c').find(
    (candidate) => (attribute(candidate, 'r') ?? '').toUpperCase().replaceAll('$', '') === anchor
  );
  if (!cell) return;
  let formula = directChild(cell, 'f');
  if (!formula) {
    formula = document.createElementNS(document.documentElement.namespaceURI, 'f');
    cell.insertBefore(formula, directChild(cell, 'v') ?? cell.firstChild);
  }
  for (const item of Array.from(formula.attributes)) formula.removeAttribute(item.name);
  formula.setAttribute('t', 'dataTable');
  formula.setAttribute('ref', range.reference);
  formula.textContent = range.formula ?? '';
  const options = range.dataTable;
  if (!options) return;
  setOptionalAttribute(formula, 'r1', options.input1Reference);
  setOptionalAttribute(formula, 'r2', options.input2Reference);
  setOptionalBooleanAttribute(formula, 'dt2D', options.twoDimensional);
  setOptionalBooleanAttribute(formula, 'dtr', options.rowOriented);
  setOptionalBooleanAttribute(formula, 'del1', options.input1Deleted);
  setOptionalBooleanAttribute(formula, 'del2', options.input2Deleted);
  setOptionalBooleanAttribute(formula, 'ca', options.calculateOnLoad);
}

function calculationMode(value: string | null): WorkSpreadsheetCalculationMode {
  if (value === 'manual') return 'manual';
  if (value === 'autoNoTable') return 'automatic-except-data-tables';
  return 'automatic';
}

function xlsxCalculationMode(mode: WorkSpreadsheetCalculationMode): string {
  if (mode === 'manual') return 'manual';
  if (mode === 'automatic-except-data-tables') return 'autoNoTable';
  return 'auto';
}

function booleanAttribute(element: Element | undefined, name: string, fallback: boolean): boolean {
  const value = element ? attribute(element, name)?.toLowerCase() : null;
  if (value === null || value === undefined) return fallback;
  return value === '1' || value === 'true';
}

function optionalBooleanAttribute(element: Element, name: string): boolean | undefined {
  return attribute(element, name) === null ? undefined : booleanAttribute(element, name, false);
}

function numericAttribute(element: Element | undefined, name: string, fallback: number): number {
  const value = Number(element ? attribute(element, name) : null);
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function positiveInteger(value: string | null): number | null {
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : null;
}

function booleanValue(value: boolean): string {
  return value ? '1' : '0';
}

function setOptionalAttribute(element: Element, name: string, value: string | undefined): void {
  if (value !== undefined) element.setAttribute(name, value);
}

function setOptionalBooleanAttribute(element: Element, name: string, value: boolean | undefined): void {
  if (value !== undefined) element.setAttribute(name, booleanValue(value));
}

function xlsxErrorValue(value: Cell['v'], formatted: Cell['m']): Cell['v'] {
  if (typeof value === 'number') return value;
  const codes: Record<string, number> = {
    '#NULL!': 0,
    '#DIV/0!': 7,
    '#VALUE!': 15,
    '#REF!': 23,
    '#NAME?': 29,
    '#NUM!': 36,
    '#N/A': 42,
    '#GETTING_DATA': 43,
  };
  return codes[String(formatted ?? value ?? '').toUpperCase()] ?? value;
}
