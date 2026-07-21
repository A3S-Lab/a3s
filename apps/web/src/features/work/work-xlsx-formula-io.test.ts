import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { createWorkArtifactBlob, importWorkFile } from './work-file-io';
import { descendants, directChild, parseXml } from './work-ooxml-package';

describe('Work XLSX formula interoperability', () => {
  it('imports calculation settings, formula groups, normalized shared formulas, and cached errors', async () => {
    const artifact = await importWorkFile(await createFormulaFixture());
    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    const sheet = artifact.content.sheets[0];

    expect(artifact.content.calculation).toEqual({
      mode: 'manual',
      fullCalculationOnLoad: true,
      forceFullCalculation: true,
      iterativeCalculation: true,
      maximumIterations: 77,
      maximumChange: 0.00001,
      fullPrecision: false,
    });
    expect(sheet.formulaMetadata).toMatchObject({
      normalizedSharedFormulaGroups: 1,
      normalizedSharedFormulaCells: 2,
      sourceFormulas: {
        D1: '_xlfn.SEQUENCE(2)',
        I1: '_xlfn.XLOOKUP(1,A1:A2,B1:B2)',
      },
      ranges: [
        {
          type: 'array',
          anchor: 'C1',
          reference: 'C1:C2',
          formula: 'SUM(A1:A2)',
        },
        {
          type: 'dynamic-array',
          anchor: 'D1',
          reference: 'D1:D2',
          formula: '_xlfn.SEQUENCE(2)',
        },
        {
          type: 'data-table',
          anchor: 'J1',
          reference: 'J1:K2',
          dataTable: {
            input1Reference: '$A$1',
            input2Reference: '$A$2',
            twoDimensional: true,
            rowOriented: false,
            calculateOnLoad: true,
          },
        },
      ],
    });
    expect(sheet.data?.[0]?.[1]?.f).toBe('=A1*2');
    expect(sheet.data?.[1]?.[1]?.f).toBe('=A2*2');
    expect(sheet.data?.[0]?.[3]?.f).toBe('=SEQUENCE(2)');
    expect(sheet.data?.[0]?.[4]).toMatchObject({
      f: '=1/0',
      v: 7,
      m: '#DIV/0!',
      ct: { t: 'e' },
    });
    expect(artifact.compatibility?.issues.map((issue) => issue.code)).toEqual(
      expect.arrayContaining([
        'xlsx.formulas',
        'xlsx.formulas.arrays',
        'xlsx.formulas.shared',
        'xlsx.formulas.data-tables',
        'xlsx.formulas.cached-errors',
        'xlsx.formulas.external-references',
        'xlsx.formulas.structured-references',
        'xlsx.formulas.unsupported-functions',
        'xlsx.formulas.volatile',
        'xlsx.formulas.attributes',
        'xlsx.calculation.mode',
        'xlsx.calculation.iteration',
        'xlsx.calculation.precision',
        'xlsx.calculation.advanced',
      ])
    );
  });

  it('exports native array and data-table formulas, exact calcPr settings, and error caches', async () => {
    const artifact = await importWorkFile(await createFormulaFixture());
    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    const sheet = artifact.content.sheets[0];
    const dynamicAnchor = sheet.data?.[0]?.[3];
    if (dynamicAnchor) dynamicAnchor.f = '=SEQUENCE(3)';
    const cachedError = sheet.data?.[0]?.[4];
    if (cachedError) delete cachedError.v;
    artifact.content.calculation = {
      mode: 'automatic-except-data-tables',
      fullCalculationOnLoad: false,
      forceFullCalculation: false,
      iterativeCalculation: true,
      maximumIterations: 125,
      maximumChange: 0.000001,
      fullPrecision: true,
    };

    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const workbook = parseXml(await requiredPart(archive, 'xl/workbook.xml'), 'xl/workbook.xml');
    const worksheet = parseXml(await requiredPart(archive, 'xl/worksheets/sheet1.xml'), 'sheet1.xml');
    const calculation = directChild(workbook.documentElement, 'calcPr');

    expect(calculation?.getAttribute('calcMode')).toBe('autoNoTable');
    expect(calculation?.getAttribute('fullCalcOnLoad')).toBe('0');
    expect(calculation?.getAttribute('forceFullCalc')).toBe('0');
    expect(calculation?.getAttribute('iterate')).toBe('1');
    expect(calculation?.getAttribute('iterateCount')).toBe('125');
    expect(calculation?.getAttribute('iterateDelta')).toBe('0.000001');
    expect(calculation?.getAttribute('fullPrecision')).toBe('1');

    const sharedFirst = formulaAt(worksheet, 'B1');
    const sharedSecond = formulaAt(worksheet, 'B2');
    expect(sharedFirst?.getAttribute('t')).toBeNull();
    expect(sharedSecond?.getAttribute('t')).toBeNull();
    expect(sharedFirst?.textContent).toBe('A1*2');
    expect(sharedSecond?.textContent).toBe('A2*2');

    const legacyArray = formulaAt(worksheet, 'C1');
    expect(legacyArray?.getAttribute('t')).toBe('array');
    expect(legacyArray?.getAttribute('ref')).toBe('C1:C2');
    expect(legacyArray?.getAttribute('aca')).toBeNull();
    expect(cellAt(worksheet, 'C1')?.getAttribute('cm')).toBeNull();

    const dynamicArray = formulaAt(worksheet, 'D1');
    expect(dynamicArray?.getAttribute('t')).toBe('array');
    expect(dynamicArray?.getAttribute('ref')).toBe('D1:D2');
    expect(dynamicArray?.textContent).toBe('_xlfn.SEQUENCE(3)');
    expect(cellAt(worksheet, 'D1')?.getAttribute('cm')).toBe('1');
    expect(archive.file('xl/metadata.xml')).not.toBeNull();

    const dataTable = formulaAt(worksheet, 'J1');
    expect(dataTable?.getAttribute('t')).toBe('dataTable');
    expect(dataTable?.getAttribute('ref')).toBe('J1:K2');
    expect(dataTable?.getAttribute('r1')).toBe('$A$1');
    expect(dataTable?.getAttribute('r2')).toBe('$A$2');
    expect(dataTable?.getAttribute('dt2D')).toBe('1');
    expect(dataTable?.getAttribute('dtr')).toBe('0');
    expect(dataTable?.getAttribute('ca')).toBe('1');

    const errorCell = cellAt(worksheet, 'E1');
    expect(errorCell?.getAttribute('t')).toBe('e');
    expect(directChild(errorCell!, 'v')?.textContent).toBe('#DIV/0!');
    const staticErrorCell = cellAt(worksheet, 'L1');
    expect(staticErrorCell?.getAttribute('t')).toBe('e');
    expect(directChild(staticErrorCell!, 'v')?.textContent).toBe('#N/A');

    const reopened = await importWorkFile(
      new File([exported], 'Formula round trip.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.calculation).toMatchObject({
      mode: 'automatic-except-data-tables',
      iterativeCalculation: true,
      maximumIterations: 125,
      maximumChange: 0.000001,
      fullPrecision: true,
    });
    expect(reopened.content.sheets[0].formulaMetadata?.ranges?.map((range) => range.type)).toEqual([
      'array',
      'dynamic-array',
      'data-table',
    ]);
    expect(reopened.content.sheets[0].data?.[0]?.[4]).toMatchObject({
      v: 7,
      m: '#DIV/0!',
      ct: { t: 'e' },
    });
    expect(reopened.content.sheets[0].data?.[0]?.[11]).toMatchObject({
      v: 42,
      m: '#N/A',
      ct: { t: 'e' },
    });
  });

  it('normalizes a grouped array to ordinary formulas when an edited spill cell conflicts', async () => {
    const artifact = await importWorkFile(await createFormulaFixture());
    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    const conflictingCell = artifact.content.sheets[0].data?.[1]?.[2];
    if (!conflictingCell) throw new Error('Array spill fixture cell is missing.');
    conflictingCell.f = '=A2';

    const archive = await JSZip.loadAsync(await createWorkArtifactBlob(artifact));
    const worksheet = parseXml(await requiredPart(archive, 'xl/worksheets/sheet1.xml'), 'sheet1.xml');
    expect(formulaAt(worksheet, 'C1')?.getAttribute('t')).toBeNull();
    expect(formulaAt(worksheet, 'C1')?.textContent).toBe('SUM(A1:A2)');
    expect(formulaAt(worksheet, 'C2')?.textContent).toBe('A2');
  });
});

async function createFormulaFixture(): Promise<File> {
  const XLSX = await import('xlsx');
  const worksheet = {
    A1: { t: 'n', v: 1 },
    A2: { t: 'n', v: 2 },
    B1: { t: 'n', v: 2, f: 'A1*2' },
    B2: { t: 'n', v: 4, f: 'A2*2' },
    C1: { t: 'n', v: 3, f: 'SUM(A1:A2)', F: 'C1:C2' },
    C2: { t: 'n', v: 3, F: 'C1:C2' },
    D1: { t: 'n', v: 1, f: '_xlfn.SEQUENCE(2)', F: 'D1:D2', D: true },
    D2: { t: 'n', v: 2, F: 'D1:D2' },
    E1: { t: 'e', v: 7, f: '1/0' },
    F1: { t: 'n', v: 15, f: "'[Budget.xlsx]Plan'!A1" },
    G1: { t: 'n', v: 30, f: 'SUM(Sales[Amount])' },
    H1: { t: 'n', v: 46_000, f: 'NOW()' },
    I1: { t: 'n', v: 2, f: '_xlfn.XLOOKUP(1,A1:A2,B1:B2)' },
    J1: { t: 'n', v: 10 },
    J2: { t: 'n', v: 11 },
    K1: { t: 'n', v: 20 },
    K2: { t: 'n', v: 21 },
    L1: { t: 'e', v: 42 },
    '!ref': 'A1:L2',
  };
  const workbook = XLSX.utils.book_new();
  XLSX.utils.book_append_sheet(workbook, worksheet, 'Model');
  const buffer = XLSX.write(workbook, { type: 'array', bookType: 'xlsx' }) as ArrayBuffer;
  const archive = await JSZip.loadAsync(buffer);
  const worksheetDocument = parseXml(
    await requiredPart(archive, 'xl/worksheets/sheet1.xml'),
    'xl/worksheets/sheet1.xml'
  );
  const firstShared = formulaAt(worksheetDocument, 'B1');
  const secondShared = formulaAt(worksheetDocument, 'B2');
  if (!firstShared || !secondShared) throw new Error('Shared-formula fixture cells are missing.');
  firstShared.setAttribute('t', 'shared');
  firstShared.setAttribute('ref', 'B1:B2');
  firstShared.setAttribute('si', '0');
  secondShared.setAttribute('t', 'shared');
  secondShared.setAttribute('si', '0');
  secondShared.textContent = '';
  formulaAt(worksheetDocument, 'C1')?.setAttribute('aca', '1');
  const dataTableCell = cellAt(worksheetDocument, 'J1');
  if (!dataTableCell) throw new Error('Data-table fixture anchor is missing.');
  const dataTable = worksheetDocument.createElementNS(worksheetDocument.documentElement.namespaceURI, 'f');
  dataTable.setAttribute('t', 'dataTable');
  dataTable.setAttribute('ref', 'J1:K2');
  dataTable.setAttribute('dt2D', '1');
  dataTable.setAttribute('dtr', '0');
  dataTable.setAttribute('r1', '$A$1');
  dataTable.setAttribute('r2', '$A$2');
  dataTable.setAttribute('ca', '1');
  dataTableCell.insertBefore(dataTable, directChild(dataTableCell, 'v') ?? null);
  archive.file('xl/worksheets/sheet1.xml', new XMLSerializer().serializeToString(worksheetDocument));

  const workbookDocument = parseXml(await requiredPart(archive, 'xl/workbook.xml'), 'xl/workbook.xml');
  const calculation =
    directChild(workbookDocument.documentElement, 'calcPr') ??
    workbookDocument.createElementNS(workbookDocument.documentElement.namespaceURI, 'calcPr');
  if (!calculation.parentElement) workbookDocument.documentElement.append(calculation);
  calculation.setAttribute('calcMode', 'manual');
  calculation.setAttribute('fullCalcOnLoad', '1');
  calculation.setAttribute('forceFullCalc', '1');
  calculation.setAttribute('iterate', '1');
  calculation.setAttribute('iterateCount', '77');
  calculation.setAttribute('iterateDelta', '0.00001');
  calculation.setAttribute('fullPrecision', '0');
  calculation.setAttribute('calcOnSave', '0');
  archive.file('xl/workbook.xml', new XMLSerializer().serializeToString(workbookDocument));

  return new File([await archive.generateAsync({ type: 'arraybuffer' })], 'Formula compatibility.xlsx', {
    type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  });
}

function cellAt(document: Document, address: string): Element | undefined {
  return descendants(document, 'c').find((cell) => cell.getAttribute('r') === address);
}

function formulaAt(document: Document, address: string): Element | undefined {
  const cell = cellAt(document, address);
  return cell ? directChild(cell, 'f') : undefined;
}

async function requiredPart(archive: JSZip, path: string): Promise<string> {
  const entry = archive.file(path);
  if (!entry) throw new Error(`Fixture part is missing: ${path}`);
  return entry.async('text');
}
