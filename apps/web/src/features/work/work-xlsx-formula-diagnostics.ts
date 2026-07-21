import {
  formulaHasExternalReference,
  formulaHasStructuredReference,
  unsupportedSpreadsheetFormulaFunctions,
  volatileSpreadsheetFormulaFunctions,
} from './work-spreadsheet-formulas';
import { readXlsxFormulaFeaturesFromPackage } from './work-xlsx-formulas';
import { OoxmlPackage } from './work-ooxml-package';
import type { WorkCompatibilityIssue } from './work-types';

export async function diagnoseXlsxFormulas(archive: OoxmlPackage): Promise<WorkCompatibilityIssue[]> {
  const features = await readXlsxFormulaFeaturesFromPackage(archive);
  const formulas = Array.from(features.sheets.entries()).flatMap(([sheetName, sheet]) =>
    sheet.formulas.map((formula) => ({ ...formula, sheetName }))
  );
  const ranges = Array.from(features.sheets.entries()).flatMap(([sheetName, sheet]) =>
    sheet.ranges.map((range) => ({ ...range, sheetName }))
  );
  const sharedGroups = Array.from(features.sheets.values()).reduce(
    (count, sheet) => count + sheet.sharedFormulaGroups,
    0
  );
  const sharedCells = Array.from(features.sheets.values()).reduce(
    (count, sheet) => count + sheet.sharedFormulaCells,
    0
  );
  const arrayRanges = ranges.filter((range) => range.type === 'array').length;
  const dynamicRanges = ranges.filter((range) => range.type === 'dynamic-array').length;
  const dataTables = ranges.filter((range) => range.type === 'data-table').length;
  const cachedErrors = formulas.filter((formula) => formula.cachedError).length;
  const externalReferences = formulas.filter((formula) => formulaHasExternalReference(formula.formula));
  const structuredReferences = formulas.filter((formula) => formulaHasStructuredReference(formula.formula));
  const volatileFormulas = formulas.filter((formula) => volatileSpreadsheetFormulaFunctions(formula.formula).length);
  const unsupportedFunctions = new Set(
    formulas.flatMap((formula) => unsupportedSpreadsheetFormulaFunctions(formula.formula))
  );
  const unsupportedFormulaAttributes = new Set(
    Array.from(features.sheets.values()).flatMap((sheet) => sheet.unsupportedFormulaAttributes)
  );
  const issues: WorkCompatibilityIssue[] = [];

  if (formulas.length) {
    issues.push(
      issue(
        'xlsx.formulas',
        `${formulas.length} formula cell(s), cached results, error values, and workbook calculation settings are preserved for XLSX round-trips.`,
        'info'
      )
    );
  }
  if (arrayRanges || dynamicRanges) {
    issues.push(
      issue(
        'xlsx.formulas.arrays',
        `${arrayRanges} legacy array range(s) and ${dynamicRanges} dynamic-array range(s) are preserved as native grouped XLSX formulas; modern spill functions may require Excel or another compatible application to recalculate.`,
        dynamicRanges ? 'warning' : 'info'
      )
    );
  }
  if (sharedGroups) {
    issues.push(
      issue(
        'xlsx.formulas.shared',
        `${sharedGroups} shared-formula group(s) covering ${sharedCells} cell(s) are expanded to equivalent per-cell formulas so edits remain deterministic.`
      )
    );
  }
  if (dataTables) {
    issues.push(
      issue(
        'xlsx.formulas.data-tables',
        `${dataTables} what-if data table(s), input references, and cached results are preserved, but Work does not recalculate scenario tables in the browser.`
      )
    );
  }
  if (cachedErrors) {
    issues.push(
      issue(
        'xlsx.formulas.cached-errors',
        `${cachedErrors} formula cell(s) contain cached Excel error values; the error type and displayed value are preserved in editing, preview, print, and export.`
      )
    );
  }
  if (externalReferences.length) {
    issues.push(
      issue(
        'xlsx.formulas.external-references',
        `${externalReferences.length} formula cell(s) reference external workbooks; formulas and cached values are preserved without refreshing external files.`
      )
    );
  }
  if (structuredReferences.length) {
    issues.push(
      issue(
        'xlsx.formulas.structured-references',
        `${structuredReferences.length} formula cell(s) use structured table references; formulas round-trip unchanged, but the Work calculation engine does not resolve Excel table objects.`
      )
    );
  }
  if (unsupportedFunctions.size) {
    issues.push(
      issue(
        'xlsx.formulas.unsupported-functions',
        `The current browser calculation engine does not evaluate ${Array.from(unsupportedFunctions).sort().join(', ')}; source formulas and cached results are preserved for compatible desktop recalculation.`
      )
    );
  }
  if (volatileFormulas.length) {
    issues.push(
      issue(
        'xlsx.formulas.volatile',
        `${volatileFormulas.length} volatile formula cell(s) can change when the workbook is explicitly recalculated.`,
        'info'
      )
    );
  }
  if (unsupportedFormulaAttributes.size) {
    issues.push(
      issue(
        'xlsx.formulas.attributes',
        `Advanced formula attributes are not represented by the editable model and normalize on export: ${Array.from(unsupportedFormulaAttributes).sort().join(', ')}.`
      )
    );
  }
  if (features.calculation.mode !== 'automatic') {
    issues.push(
      issue(
        'xlsx.calculation.mode',
        features.calculation.mode === 'manual'
          ? 'Manual workbook calculation mode is preserved; cached values remain visible until an explicit recalculation.'
          : 'Automatic calculation except for data tables is preserved; scenario tables require an explicit compatible recalculation.',
        'info'
      )
    );
  }
  if (features.calculation.iterativeCalculation) {
    issues.push(
      issue(
        'xlsx.calculation.iteration',
        `Iterative calculation is preserved with at most ${features.calculation.maximumIterations} iterations and a ${features.calculation.maximumChange} convergence threshold.`,
        'info'
      )
    );
  }
  if (!features.calculation.fullPrecision) {
    issues.push(
      issue(
        'xlsx.calculation.precision',
        'Calculation using displayed precision is preserved and may produce results different from full-precision calculation.'
      )
    );
  }
  if (features.unsupportedCalculationAttributes.length) {
    issues.push(
      issue(
        'xlsx.calculation.advanced',
        `Advanced workbook calculation attributes remain source-only: ${features.unsupportedCalculationAttributes.join(', ')}.`
      )
    );
  }
  return issues;
}

function issue(
  code: string,
  message: string,
  severity: WorkCompatibilityIssue['severity'] = 'warning'
): WorkCompatibilityIssue {
  return {
    code,
    severity,
    feature: 'Formulas and calculation',
    message,
  };
}
