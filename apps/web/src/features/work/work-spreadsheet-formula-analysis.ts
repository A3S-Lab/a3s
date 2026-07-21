import type { Cell } from '@fortune-sheet/core';
import {
  effectiveSpreadsheetCalculationSettings,
  formulaHasExternalReference,
  formulaHasStructuredReference,
  spreadsheetCellAddress,
  spreadsheetFormulaRangeConflict,
  unsupportedSpreadsheetFormulaFunctions,
  volatileSpreadsheetFormulaFunctions,
} from './work-spreadsheet-formulas';
import type { WorkSpreadsheetContent } from './work-types';

export interface SpreadsheetFormulaSummary {
  formulaCells: number;
  cachedErrorCells: number;
  arrayRanges: number;
  dynamicArrayRanges: number;
  dataTableRanges: number;
  normalizedSharedFormulaGroups: number;
  normalizedSharedFormulaCells: number;
  externalReferenceCells: number;
  structuredReferenceCells: number;
  volatileFormulaCells: number;
  unsupportedFunctions: string[];
  spillConflicts: number;
}

export interface SpreadsheetFormulaDiagnostic {
  code: string;
  severity: 'info' | 'warning' | 'error';
  title: string;
  message: string;
  locations: string[];
}

export interface SpreadsheetFormulaAnalysis {
  summary: SpreadsheetFormulaSummary;
  diagnostics: SpreadsheetFormulaDiagnostic[];
}

interface FormulaScan {
  summary: SpreadsheetFormulaSummary;
  locations: {
    errors: string[];
    external: string[];
    structured: string[];
    volatile: string[];
    unsupported: string[];
    conflicts: string[];
    dataTables: string[];
  };
}

export function spreadsheetFormulaCount(content: WorkSpreadsheetContent): number {
  return content.sheets.reduce(
    (count, sheet) =>
      count +
      (sheet.data ?? []).reduce(
        (sheetCount, row) => sheetCount + row.reduce((rowCount, cell) => rowCount + (cell?.f ? 1 : 0), 0),
        0
      ),
    0
  );
}

export function spreadsheetFormulaSummary(content: WorkSpreadsheetContent): SpreadsheetFormulaSummary {
  return scanSpreadsheetFormulas(content).summary;
}

export function spreadsheetFormulaDiagnostics(content: WorkSpreadsheetContent): SpreadsheetFormulaDiagnostic[] {
  return spreadsheetFormulaAnalysis(content).diagnostics;
}

export function spreadsheetFormulaAnalysis(content: WorkSpreadsheetContent): SpreadsheetFormulaAnalysis {
  const scan = scanSpreadsheetFormulas(content);
  return {
    summary: scan.summary,
    diagnostics: createSpreadsheetFormulaDiagnostics(content, scan),
  };
}

function createSpreadsheetFormulaDiagnostics(
  content: WorkSpreadsheetContent,
  { summary, locations }: FormulaScan
): SpreadsheetFormulaDiagnostic[] {
  const diagnostics: SpreadsheetFormulaDiagnostic[] = [];
  const settings = effectiveSpreadsheetCalculationSettings(content.calculation);

  if (settings.mode === 'manual') {
    diagnostics.push({
      code: 'calculation.manual',
      severity: 'warning',
      title: '手动计算',
      message: '工作簿将以手动模式保存；Work 只在显式重算时刷新整簿或选区缓存，正在编辑的单个公式仍可能即时计算。',
      locations: [],
    });
  } else if (settings.mode === 'automatic-except-data-tables') {
    diagnostics.push({
      code: 'calculation.automatic-except-data-tables',
      severity: 'info',
      title: '自动计算（数据表除外）',
      message: '普通公式自动计算；模拟运算表需要显式重算。',
      locations: [],
    });
  }
  if (settings.iterativeCalculation) {
    diagnostics.push({
      code: 'calculation.iterative',
      severity: 'info',
      title: '迭代计算',
      message: `循环引用最多迭代 ${settings.maximumIterations} 次，收敛阈值为 ${settings.maximumChange}。`,
      locations: [],
    });
  }
  if (!settings.fullPrecision) {
    diagnostics.push({
      code: 'calculation.displayed-precision',
      severity: 'warning',
      title: '按显示精度计算',
      message: '计算可能使用格式化后的显示精度，结果会与全精度计算不同。',
      locations: [],
    });
  }
  if (summary.cachedErrorCells) {
    diagnostics.push({
      code: 'formula.cached-errors',
      severity: 'error',
      title: '公式错误',
      message: `${summary.cachedErrorCells} 个公式单元格包含缓存错误值；错误会在预览、打印与导出中保留。`,
      locations: locations.errors,
    });
  }
  if (summary.externalReferenceCells) {
    diagnostics.push({
      code: 'formula.external-references',
      severity: 'warning',
      title: '外部工作簿引用',
      message: `${summary.externalReferenceCells} 个公式引用外部工作簿；Work 保留公式和缓存结果，但不会刷新外部文件。`,
      locations: locations.external,
    });
  }
  if (summary.structuredReferenceCells) {
    diagnostics.push({
      code: 'formula.structured-references',
      severity: 'warning',
      title: '结构化引用',
      message: `${summary.structuredReferenceCells} 个公式使用表格结构化引用；公式会原样保存，但当前计算引擎不解析 Excel 表对象。`,
      locations: locations.structured,
    });
  }
  if (summary.unsupportedFunctions.length) {
    diagnostics.push({
      code: 'formula.unsupported-functions',
      severity: 'warning',
      title: '当前引擎不支持的函数',
      message: `${summary.unsupportedFunctions.join('、')} 会原样往返并保留缓存结果，但需要 Excel 或兼容应用重新计算。`,
      locations: locations.unsupported,
    });
  }
  if (summary.volatileFormulaCells) {
    diagnostics.push({
      code: 'formula.volatile',
      severity: 'info',
      title: '易失性公式',
      message: `${summary.volatileFormulaCells} 个公式会在重算时随时间、随机数或引用状态变化。`,
      locations: locations.volatile,
    });
  }
  if (summary.normalizedSharedFormulaGroups) {
    diagnostics.push({
      code: 'formula.shared-normalized',
      severity: 'info',
      title: '共享公式已展开',
      message: `${summary.normalizedSharedFormulaGroups} 组、${summary.normalizedSharedFormulaCells} 个共享公式单元格已展开为等价的逐单元格公式。`,
      locations: [],
    });
  }
  if (summary.arrayRanges || summary.dynamicArrayRanges) {
    diagnostics.push({
      code: 'formula.array-ranges',
      severity: 'info',
      title: '数组公式',
      message: `${summary.arrayRanges} 个传统数组范围和 ${summary.dynamicArrayRanges} 个动态数组范围会以原生 XLSX 分组保存。`,
      locations: [],
    });
  }
  if (summary.dataTableRanges) {
    diagnostics.push({
      code: 'formula.data-tables',
      severity: 'warning',
      title: '模拟运算表',
      message: `${summary.dataTableRanges} 个模拟运算表会保留输入引用和缓存结果，但 Work 不在浏览器中重新运行情景分析。`,
      locations: locations.dataTables,
    });
  }
  if (summary.spillConflicts) {
    diagnostics.push({
      code: 'formula.spill-conflicts',
      severity: 'error',
      title: '数组范围冲突',
      message: `${summary.spillConflicts} 个数组或模拟运算表范围与独立公式、合并单元格或其他公式范围冲突；保存时将按普通公式规范化以避免无效 XLSX。`,
      locations: locations.conflicts,
    });
  }
  return diagnostics;
}

function scanSpreadsheetFormulas(content: WorkSpreadsheetContent): FormulaScan {
  const summary: SpreadsheetFormulaSummary = {
    formulaCells: 0,
    cachedErrorCells: 0,
    arrayRanges: 0,
    dynamicArrayRanges: 0,
    dataTableRanges: 0,
    normalizedSharedFormulaGroups: 0,
    normalizedSharedFormulaCells: 0,
    externalReferenceCells: 0,
    structuredReferenceCells: 0,
    volatileFormulaCells: 0,
    unsupportedFunctions: [],
    spillConflicts: 0,
  };
  const locations = {
    errors: [] as string[],
    external: [] as string[],
    structured: [] as string[],
    volatile: [] as string[],
    unsupported: [] as string[],
    conflicts: [] as string[],
    dataTables: [] as string[],
  };
  const unsupportedFunctions = new Set<string>();

  for (const sheet of content.sheets) {
    summary.normalizedSharedFormulaGroups += sheet.formulaMetadata?.normalizedSharedFormulaGroups ?? 0;
    summary.normalizedSharedFormulaCells += sheet.formulaMetadata?.normalizedSharedFormulaCells ?? 0;
    for (const [row, cells] of (sheet.data ?? []).entries()) {
      for (const [column, cell] of cells.entries()) {
        if (!cell?.f) continue;
        const address = spreadsheetCellAddress(row, column);
        const location = `${sheet.name}!${address}`;
        const formula = sheet.formulaMetadata?.sourceFormulas?.[address] ?? cell.f;
        summary.formulaCells += 1;
        if (isFormulaError(cell)) {
          summary.cachedErrorCells += 1;
          locations.errors.push(location);
        }
        if (formulaHasExternalReference(formula)) {
          summary.externalReferenceCells += 1;
          locations.external.push(location);
        }
        if (formulaHasStructuredReference(formula)) {
          summary.structuredReferenceCells += 1;
          locations.structured.push(location);
        }
        if (volatileSpreadsheetFormulaFunctions(formula).length) {
          summary.volatileFormulaCells += 1;
          locations.volatile.push(location);
        }
        const unsupported = unsupportedSpreadsheetFormulaFunctions(formula);
        if (unsupported.length) {
          for (const name of unsupported) unsupportedFunctions.add(name);
          locations.unsupported.push(location);
        }
      }
    }
    for (const range of sheet.formulaMetadata?.ranges ?? []) {
      if (range.type === 'array') summary.arrayRanges += 1;
      else if (range.type === 'dynamic-array') summary.dynamicArrayRanges += 1;
      else {
        summary.dataTableRanges += 1;
        locations.dataTables.push(`${sheet.name}!${range.reference}`);
      }
      const conflict = spreadsheetFormulaRangeConflict(sheet, range);
      if (conflict) {
        summary.spillConflicts += 1;
        locations.conflicts.push(`${sheet.name}!${range.reference}（${conflict}）`);
      }
    }
  }
  summary.unsupportedFunctions = Array.from(unsupportedFunctions).sort();
  return { summary, locations };
}

function isFormulaError(cell: Cell): boolean {
  return (
    cell.ct?.t === 'e' || /^#(?:NULL!|DIV\/0!|VALUE!|REF!|NAME\?|NUM!|N\/A|GETTING_DATA)$/i.test(String(cell.m ?? ''))
  );
}
