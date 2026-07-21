import type { Cell, Selection } from '@fortune-sheet/core';
import type { WorkAgentProposalTarget } from './work-agent-proposal';
import { chartSeriesErrorBarContext, chartSeriesTrendlineContext } from './work-chart-series-analysis-context';
import {
  normalizeWorkSpreadsheetChartAxes,
  workSpreadsheetChartAxisLabelPositionLabel,
  workSpreadsheetChartAxisPositionLabel,
  workSpreadsheetChartAxisTickMarkLabel,
} from './work-spreadsheet-chart-axis';
import {
  normalizeWorkSpreadsheetChartGapWidth,
  normalizeWorkSpreadsheetChartGrouping,
  normalizeWorkSpreadsheetChartLegendOverlay,
  normalizeWorkSpreadsheetChartLegendPosition,
  normalizeWorkSpreadsheetChartOverlap,
  normalizeWorkSpreadsheetChartSmoothLines,
  workSpreadsheetChartGroupingLabel,
  workSpreadsheetChartLegendPositionLabel,
  workSpreadsheetChartSupportsBarSpacing,
  workSpreadsheetChartSupportsGrouping,
  workSpreadsheetChartSupportsSmoothLines,
} from './work-spreadsheet-chart-layout';
import { spreadsheetChartSeriesStyleContext } from './work-spreadsheet-chart-series-style';
import { parseSpreadsheetChartReference, resolveSpreadsheetChart } from './work-spreadsheet-charts';
import {
  effectiveSpreadsheetCalculationSettings,
  spreadsheetFormulaRangesForSelection,
} from './work-spreadsheet-formulas';
import {
  spreadsheetPivotAggregationLabel,
  spreadsheetPivotFields,
  spreadsheetPivotIntersects,
} from './work-spreadsheet-pivots';
import { parseSpreadsheetCellRanges } from './work-spreadsheet-ranges';
import {
  normalizeWorkSpreadsheetChartAxisGroup,
  normalizeWorkSpreadsheetCombinationSeriesType,
  normalizeWorkSpreadsheetDataLabels,
  type WorkSpreadsheetChart,
  type WorkSpreadsheetChartSeries,
  type WorkSpreadsheetChartType,
  type WorkSpreadsheetContent,
  type WorkSpreadsheetFormulaRangeType,
  workSpreadsheetChartTypeLabel,
  workSpreadsheetCombinationSeriesTypeLabel,
  workSpreadsheetDataLabelPositionLabel,
} from './work-types';

export interface WorkSpreadsheetAgentSelection {
  sheetId: string;
  sheetName: string;
  reference: string;
  cellCount: number;
  context: string;
  clipboard: string;
  truncated: boolean;
  proposalTargets: WorkAgentProposalTarget[];
}

export function spreadsheetAgentSelection(
  content: WorkSpreadsheetContent,
  sheetId: string,
  selection: Pick<Selection, 'row' | 'column'>,
  maximumCells = 200
): WorkSpreadsheetAgentSelection | null {
  const sheet = content.sheets.find((candidate) => candidate.id === sheetId) ?? content.sheets[0];
  if (!sheet || selection.row.length < 2 || selection.column.length < 2) return null;
  const rowStart = Math.max(0, Math.min(selection.row[0], selection.row[1]));
  const rowEnd = Math.max(rowStart, Math.max(selection.row[0], selection.row[1]));
  const columnStart = Math.max(0, Math.min(selection.column[0], selection.column[1]));
  const columnEnd = Math.max(columnStart, Math.max(selection.column[0], selection.column[1]));
  const rowCount = rowEnd - rowStart + 1;
  const columnCount = columnEnd - columnStart + 1;
  const cellCount = rowCount * columnCount;
  const limit = Math.max(1, Math.floor(maximumCells));
  const rows: string[] = [];
  const formulas: string[] = [];
  const proposalTargets: WorkAgentProposalTarget[] = [];
  let included = 0;

  for (let row = rowStart; row <= rowEnd && included < limit; row += 1) {
    const values: string[] = [];
    for (let column = columnStart; column <= columnEnd && included < limit; column += 1) {
      const cell = sheet.data?.[row]?.[column] ?? null;
      const cellReference = spreadsheetCellReference(row, column);
      values.push(spreadsheetCellText(cell));
      proposalTargets.push({
        id: cellReference,
        label: `${sheet.name}!${cellReference}`,
        before: spreadsheetCellSourceText(cell),
      });
      if (cell?.f) {
        const sourceFormula = sheet.formulaMetadata?.sourceFormulas?.[cellReference] ?? cell.f;
        formulas.push(`${cellReference}：${normalizeCellText(sourceFormula)}`);
      }
      included += 1;
    }
    rows.push(values.join('\t'));
  }

  const reference = `${spreadsheetCellReference(rowStart, columnStart)}:${spreadsheetCellReference(rowEnd, columnEnd)}`;
  const clipboard = rows.join('\n');
  const truncated = cellCount > included;
  const chartContext = relatedChartContext(content, sheet, {
    rowStart,
    rowEnd,
    columnStart,
    columnEnd,
  });
  const formulaRanges = spreadsheetFormulaRangesForSelection(sheet, {
    startRow: rowStart,
    endRow: rowEnd,
    startColumn: columnStart,
    endColumn: columnEnd,
  });
  const formulaContext =
    formulas.length || formulaRanges.length
      ? [
          `计算模式：${calculationModeLabel(effectiveSpreadsheetCalculationSettings(content.calculation).mode)}`,
          ...formulaRanges.map(
            (range) => `${range.reference}：${formulaRangeTypeLabel(range.type)}（锚点 ${range.anchor}）`
          ),
        ]
      : [];
  const pivotContext = relatedPivotContext(content, sheet, {
    rowStart,
    rowEnd,
    columnStart,
    columnEnd,
  });
  const context = [
    `工作表：${sheet.name}`,
    `选区：${reference}`,
    `单元格数量：${cellCount}`,
    '内容（制表符分隔，空白单元格保留）：',
    clipboard,
    ...(formulas.length ? ['', '公式：', ...formulas] : []),
    ...(formulaContext.length ? ['', '公式与计算：', ...formulaContext] : []),
    ...(pivotContext.length ? ['', '关联数据透视表：', ...pivotContext] : []),
    ...(chartContext.length ? ['', '关联图表：', ...chartContext] : []),
    ...(truncated ? ['', `[选区过大，仅包含前 ${included} 个单元格]`] : []),
  ].join('\n');

  return {
    sheetId: sheet.id ?? sheetId,
    sheetName: sheet.name,
    reference,
    cellCount,
    context,
    clipboard,
    truncated,
    proposalTargets,
  };
}

function relatedPivotContext(
  content: WorkSpreadsheetContent,
  sheet: WorkSpreadsheetContent['sheets'][number],
  selection: { rowStart: number; rowEnd: number; columnStart: number; columnEnd: number }
): string[] {
  const related = new Map<
    string,
    {
      pivot: NonNullable<WorkSpreadsheetContent['sheets'][number]['pivotTables']>[number];
      owner: WorkSpreadsheetContent['sheets'][number];
    }
  >();
  for (const owner of content.sheets) {
    for (const pivot of owner.pivotTables ?? []) {
      if (
        owner.id === sheet.id &&
        spreadsheetPivotIntersects(owner, {
          startRow: selection.rowStart,
          endRow: selection.rowEnd,
          startColumn: selection.columnStart,
          endColumn: selection.columnEnd,
        }).some((candidate) => candidate.id === pivot.id)
      ) {
        related.set(pivot.id, { pivot, owner });
        continue;
      }
      if (pivot.sourceSheetId !== sheet.id) continue;
      const source = parseSpreadsheetCellRanges(pivot.sourceReference)?.[0];
      if (
        source &&
        source.row[0] <= selection.rowEnd &&
        source.row[1] >= selection.rowStart &&
        source.column[0] <= selection.columnEnd &&
        source.column[1] >= selection.columnStart
      ) {
        related.set(pivot.id, { pivot, owner });
      }
    }
  }
  return Array.from(related.values()).map(({ pivot, owner }) => {
    const fields = spreadsheetPivotFields(content, pivot);
    const rows =
      pivot.rowFields
        .map((index) => fields[index]?.name)
        .filter(Boolean)
        .join('、') || '无';
    const columns =
      pivot.columnFields
        .map((index) => fields[index]?.name)
        .filter(Boolean)
        .join('、') || '无';
    const values =
      pivot.values
        .map(
          (value) =>
            `${value.caption || fields[value.fieldIndex]?.name || `字段 ${value.fieldIndex + 1}`}（${spreadsheetPivotAggregationLabel(
              value.aggregation
            )}）`
        )
        .join('、') || '无';
    const filters =
      (pivot.reportFilters ?? [])
        .map((filter) => {
          const field = fields[filter.fieldIndex]?.name ?? `字段 ${filter.fieldIndex + 1}`;
          const selected =
            filter.selectedItem === undefined ? '（全部）' : spreadsheetPivotFilterDisplay(filter.selectedItem);
          return `${field}=${selected}`;
        })
        .join('、') || '无';
    const sourceSheet = content.sheets.find((candidate) => candidate.id === pivot.sourceSheetId);
    return `${pivot.name}（来源：${sourceSheet?.name ?? '缺失工作表'}!${pivot.sourceReference}；行：${rows}；列：${columns}；值：${values}；筛选：${filters}；输出：${owner.name}!${pivot.outputReference ?? pivot.anchor}）`;
  });
}

function spreadsheetPivotFilterDisplay(value: string | number | boolean | null): string {
  if (value === null) return '（空白）';
  if (typeof value === 'boolean') return value ? 'TRUE' : 'FALSE';
  return String(value);
}

function relatedChartContext(
  content: WorkSpreadsheetContent,
  sheet: WorkSpreadsheetContent['sheets'][number],
  selection: { rowStart: number; rowEnd: number; columnStart: number; columnEnd: number }
): string[] {
  return (sheet.charts ?? []).flatMap((chart) => {
    const references = [
      chart.titleReference,
      chart.categoryReference,
      chart.axes?.bottom?.titleReference,
      chart.axes?.left?.titleReference,
      chart.axes?.top?.titleReference,
      chart.axes?.right?.titleReference,
      ...chart.series.flatMap((series) => [
        series.nameReference,
        series.valuesReference,
        series.xValuesReference,
        series.bubbleSizesReference,
        ...(series.errorBars ?? []).flatMap((errorBars) => [errorBars.plusReference, errorBars.minusReference]),
      ]),
    ].filter((reference): reference is string => Boolean(reference));
    const related = references.some((reference) => {
      const range = parseSpreadsheetChartReference(content, sheet, reference);
      if (!range || range.sheet.id !== sheet.id) return false;
      return (
        range.startRow <= selection.rowEnd &&
        range.endRow >= selection.rowStart &&
        range.startColumn <= selection.columnEnd &&
        range.endColumn >= selection.columnStart
      );
    });
    if (!related) return [];
    const resolved = resolveSpreadsheetChart(content, sheet, chart);
    const axisTitles = chartAxisTitleContext(resolved);
    const axisSettings = chartAxisSettingsContext(resolved);
    const layout = chartLayoutContext(resolved);
    return [
      `${resolved.title || resolved.name}（${workSpreadsheetChartTypeLabel(resolved.type)}；${
        axisTitles ? `${axisTitles}；` : ''
      }${
        axisSettings ? `${axisSettings}；` : ''
      }${layout ? `${layout}；` : ''}分类：${resolved.categories.slice(0, 12).join('、') || '无'}；${resolved.series
        .slice(0, 6)
        .map((series) => {
          if (resolved.type === 'scatter' || resolved.type === 'bubble') {
            const xValues = series.xValues?.length ? series.xValues : series.values.map((_, index) => index + 1);
            return `${series.name}：X=${xValues.slice(0, 12).join('、')}；Y=${series.values.slice(0, 12).join('、')}${
              resolved.type === 'bubble' ? `；大小=${series.bubbleSizes?.slice(0, 12).join('、') || '无'}` : ''
            }${spreadsheetChartSeriesStyleContext(series.style)}${seriesDataLabelContext(
              series,
              resolved.type
            )}${chartSeriesErrorBarContext(series, resolved.type)}${chartSeriesTrendlineContext(series)}`;
          }
          if (resolved.type === 'combination') {
            const seriesType = normalizeWorkSpreadsheetCombinationSeriesType(series.chartType);
            const axisGroup = normalizeWorkSpreadsheetChartAxisGroup(series.axisGroup);
            return `${series.name}（${workSpreadsheetCombinationSeriesTypeLabel(seriesType)}，${
              axisGroup === 'secondary' ? '次坐标轴' : '主坐标轴'
            }）=${series.values.slice(0, 12).join('、')}${spreadsheetChartSeriesStyleContext(
              series.style
            )}${seriesDataLabelContext(
              series,
              resolved.type
            )}${chartSeriesErrorBarContext(series, resolved.type)}${chartSeriesTrendlineContext(series)}`;
          }
          return `${series.name}=${series.values.slice(0, 12).join('、')}${spreadsheetChartSeriesStyleContext(
            series.style
          )}${seriesDataLabelContext(
            series,
            resolved.type
          )}${chartSeriesErrorBarContext(series, resolved.type)}${chartSeriesTrendlineContext(series)}`;
        })
        .join('；')}）`,
    ];
  });
}

function chartLayoutContext(chart: WorkSpreadsheetChart): string {
  const descriptions: string[] = [];
  if (chart.showLegend) {
    const position = normalizeWorkSpreadsheetChartLegendPosition(chart.legendPosition);
    const overlay = normalizeWorkSpreadsheetChartLegendOverlay(chart.legendOverlay) ? '（叠加绘图区）' : '';
    descriptions.push(`图例：${workSpreadsheetChartLegendPositionLabel(position)}${overlay}`);
  }
  if (workSpreadsheetChartSupportsGrouping(chart.type)) {
    const grouping = normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type);
    const settings = [workSpreadsheetChartGroupingLabel(grouping)];
    if (workSpreadsheetChartSupportsBarSpacing(chart.type)) {
      settings.push(`分类间距 ${normalizeWorkSpreadsheetChartGapWidth(chart.gapWidth)}%`);
      settings.push(`系列重叠 ${normalizeWorkSpreadsheetChartOverlap(chart.overlap, grouping)}%`);
    }
    if (
      workSpreadsheetChartSupportsSmoothLines(chart.type) &&
      normalizeWorkSpreadsheetChartSmoothLines(chart.smoothLines)
    ) {
      settings.push('平滑线');
    }
    descriptions.push(`绘图区：${settings.join('，')}`);
  }
  return descriptions.join('；');
}

function chartAxisTitleContext(chart: WorkSpreadsheetChart): string {
  const hasSecondaryAxes =
    chart.type === 'combination' &&
    chart.series.some((series) => normalizeWorkSpreadsheetChartAxisGroup(series.axisGroup) === 'secondary');
  const axes = normalizeWorkSpreadsheetChartAxes(chart.axes, chart.type, hasSecondaryAxes);
  if (!axes) return '';
  const descriptions = (['bottom', 'left', 'top', 'right'] as const).flatMap((position) => {
    const title = axes[position]?.title?.trim();
    return title ? [`${workSpreadsheetChartAxisPositionLabel(position)}“${title}”`] : [];
  });
  return descriptions.length ? `坐标轴标题：${descriptions.join('，')}` : '';
}

function chartAxisSettingsContext(chart: WorkSpreadsheetChart): string {
  const hasSecondaryAxes =
    chart.type === 'combination' &&
    chart.series.some((series) => normalizeWorkSpreadsheetChartAxisGroup(series.axisGroup) === 'secondary');
  const axes = normalizeWorkSpreadsheetChartAxes(chart.axes, chart.type, hasSecondaryAxes);
  if (!axes) return '';
  const descriptions = (['bottom', 'left', 'top', 'right'] as const).flatMap((position) => {
    const axis = axes[position];
    if (!axis) return [];
    const settings = [
      axis.minimum !== undefined && axis.maximum !== undefined
        ? `范围 ${axis.minimum}–${axis.maximum}`
        : axis.minimum !== undefined
          ? `最小值 ${axis.minimum}`
          : axis.maximum !== undefined
            ? `最大值 ${axis.maximum}`
            : '',
      axis.majorUnit !== undefined ? `主单位 ${axis.majorUnit}` : '',
      axis.numberFormat
        ? `数字格式 ${axis.numberFormat}${axis.numberFormatSourceLinked ? '（链接源）' : ''}`
        : axis.numberFormatSourceLinked
          ? '数字格式链接源'
          : '',
      axis.showMajorGridlines === undefined ? '' : axis.showMajorGridlines ? '显示主要网格线' : '不显示主要网格线',
      axis.reverseOrder ? '逆序' : '',
      axis.labelPosition ? `标签置于${workSpreadsheetChartAxisLabelPositionLabel(axis.labelPosition)}` : '',
      axis.majorTickMark ? `主要刻度线${workSpreadsheetChartAxisTickMarkLabel(axis.majorTickMark)}` : '',
      axis.labelInterval ? `标签间隔 ${axis.labelInterval}` : '',
    ].filter(Boolean);
    return settings.length ? [`${workSpreadsheetChartAxisPositionLabel(position)}（${settings.join('，')}）`] : [];
  });
  return descriptions.length ? `坐标轴设置：${descriptions.join('；')}` : '';
}

function seriesDataLabelContext(series: WorkSpreadsheetChartSeries, chartType: WorkSpreadsheetChartType): string {
  if (!series.dataLabels) return '';
  const labels = normalizeWorkSpreadsheetDataLabels(series.dataLabels, chartType);
  const content = [
    labels.showValue ? '数值' : '',
    labels.showCategoryName ? '分类名称' : '',
    labels.showSeriesName ? '系列名称' : '',
    labels.showPercentage ? '百分比' : '',
    labels.showBubbleSize ? '气泡大小' : '',
  ].filter(Boolean);
  const description = [
    content.join('、') || '无内容字段',
    labels.position ? workSpreadsheetDataLabelPositionLabel(labels.position) : '',
    labels.separator !== undefined ? `分隔符“${labels.separator.replace(/\r?\n/g, '↵')}”` : '',
  ].filter(Boolean);
  return `；数据标签：${description.join('，')}`;
}

function spreadsheetCellText(cell: Cell | null): string {
  if (!cell) return '';
  const value = cell.m ?? cell.v ?? '';
  return normalizeCellText(String(value));
}

export function spreadsheetCellSourceText(cell: Cell | null): string {
  if (!cell) return '';
  if (cell.f) return normalizeCellText(cell.f);
  return normalizeCellText(String(cell.v ?? cell.m ?? ''));
}

function normalizeCellText(value: string): string {
  return value.replace(/\r?\n/g, ' ').replace(/\t/g, ' ').trim();
}

function spreadsheetCellReference(row: number, column: number): string {
  let value = column + 1;
  let label = '';
  while (value > 0) {
    value -= 1;
    label = String.fromCharCode(65 + (value % 26)) + label;
    value = Math.floor(value / 26);
  }
  return `${label}${row + 1}`;
}

function calculationModeLabel(mode: ReturnType<typeof effectiveSpreadsheetCalculationSettings>['mode']): string {
  if (mode === 'manual') return '手动';
  if (mode === 'automatic-except-data-tables') return '自动（模拟运算表除外）';
  return '自动';
}

function formulaRangeTypeLabel(type: WorkSpreadsheetFormulaRangeType): string {
  if (type === 'dynamic-array') return '动态数组';
  if (type === 'data-table') return '模拟运算表';
  return '传统数组';
}
