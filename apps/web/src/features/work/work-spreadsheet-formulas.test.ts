import { describe, expect, it } from 'vitest';
import { spreadsheetFormulaDiagnostics, spreadsheetFormulaSummary } from './work-spreadsheet-formula-analysis';
import {
  editableSpreadsheetFormula,
  effectiveSpreadsheetCalculationSettings,
  spreadsheetFormulaForXlsx,
} from './work-spreadsheet-formulas';
import { createWorkArtifact } from './work-templates';

describe('Work spreadsheet formulas', () => {
  it('normalizes calculation settings to deterministic workbook limits', () => {
    expect(
      effectiveSpreadsheetCalculationSettings({
        mode: 'manual',
        fullCalculationOnLoad: true,
        forceFullCalculation: true,
        iterativeCalculation: true,
        maximumIterations: 99_999,
        maximumChange: -1,
        fullPrecision: false,
      })
    ).toEqual({
      mode: 'manual',
      fullCalculationOnLoad: true,
      forceFullCalculation: true,
      iterativeCalculation: true,
      maximumIterations: 10_000,
      maximumChange: 0.001,
      fullPrecision: false,
    });
  });

  it('finds formula compatibility risks, cached errors, and grouped-range conflicts', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    if (artifact.content.type !== 'spreadsheet') throw new Error('Spreadsheet fixture is invalid');
    const sheet = artifact.content.sheets[0];
    sheet.name = '模型';
    sheet.data = [
      [
        { f: "='[Budget.xlsx]Plan'!A1", v: 12, m: '12' },
        { f: '=SEQUENCE(2)', v: 1, m: '1' },
        { f: '=ROW()', v: 1, m: '1' },
        { v: 10, m: '10' },
      ],
      [
        { f: '=SUM(Sales[Amount])', v: 24, m: '24' },
        { v: 2, m: '2' },
        { f: '=ROW()', v: 2, m: '2' },
        { v: 11, m: '11' },
      ],
      [{ f: '=NOW()', v: 46_000, m: '2025-12-09' }, null, null, null],
      [{ f: '=XLOOKUP(1,B1:B2,C1:C2)', v: 42, m: '#N/A', ct: { t: 'e' } }, null, null, null],
    ];
    sheet.formulaMetadata = {
      sourceFormulas: {
        A4: '_xlfn.XLOOKUP(1,B1:B2,C1:C2)',
        B1: '_xlfn.SEQUENCE(2)',
      },
      normalizedSharedFormulaGroups: 1,
      normalizedSharedFormulaCells: 3,
      ranges: [
        {
          type: 'dynamic-array',
          anchor: 'B1',
          reference: 'B1:B2',
          formula: '_xlfn.SEQUENCE(2)',
        },
        {
          type: 'array',
          anchor: 'C1',
          reference: 'C1:C2',
          formula: 'ROW()',
        },
        {
          type: 'data-table',
          anchor: 'D1',
          reference: 'D1:E2',
          dataTable: { input1Reference: '$A$1' },
        },
      ],
    };
    artifact.content.calculation = {
      mode: 'manual',
      fullCalculationOnLoad: false,
      forceFullCalculation: false,
      iterativeCalculation: true,
      maximumIterations: 50,
      maximumChange: 0.0001,
      fullPrecision: false,
    };

    expect(spreadsheetFormulaSummary(artifact.content)).toEqual({
      formulaCells: 7,
      cachedErrorCells: 1,
      arrayRanges: 1,
      dynamicArrayRanges: 1,
      dataTableRanges: 1,
      normalizedSharedFormulaGroups: 1,
      normalizedSharedFormulaCells: 3,
      externalReferenceCells: 1,
      structuredReferenceCells: 1,
      volatileFormulaCells: 1,
      unsupportedFunctions: ['XLOOKUP'],
      spillConflicts: 1,
    });
    expect(spreadsheetFormulaDiagnostics(artifact.content).map((item) => item.code)).toEqual(
      expect.arrayContaining([
        'calculation.manual',
        'calculation.iterative',
        'calculation.displayed-precision',
        'formula.cached-errors',
        'formula.external-references',
        'formula.structured-references',
        'formula.unsupported-functions',
        'formula.volatile',
        'formula.shared-normalized',
        'formula.array-ranges',
        'formula.data-tables',
        'formula.spill-conflicts',
      ])
    );
  });

  it('keeps original future-function prefixes until the editable formula changes', () => {
    expect(editableSpreadsheetFormula('_xlfn._xlws.FILTER(A1:A3,A1:A3>1)')).toBe('FILTER(A1:A3,A1:A3>1)');
    expect(editableSpreadsheetFormula('="_xlfn.FILTER("&_xlfn._xlws.FILTER(A1:A3,A1:A3>1)')).toBe(
      '="_xlfn.FILTER("&FILTER(A1:A3,A1:A3>1)'
    );
    expect(spreadsheetFormulaForXlsx('=FILTER(A1:A3,A1:A3>1)', '_xlfn._xlws.FILTER(A1:A3,A1:A3>1)')).toBe(
      '_xlfn._xlws.FILTER(A1:A3,A1:A3>1)'
    );
    expect(spreadsheetFormulaForXlsx('=SEQUENCE(4)')).toBe('_xlfn.SEQUENCE(4)');
    expect(spreadsheetFormulaForXlsx('="XLOOKUP("&SUM(A1:A2)')).toBe('"XLOOKUP("&SUM(A1:A2)');
    expect(spreadsheetFormulaForXlsx('=FUTUREMODEL(A2)', '_xlfn.FUTUREMODEL(A1)')).toBe('_xlfn.FUTUREMODEL(A2)');
    expect(spreadsheetFormulaForXlsx('=SUM(A1:A4)', '_xlfn.SEQUENCE(4)')).toBe('SUM(A1:A4)');
  });
});
