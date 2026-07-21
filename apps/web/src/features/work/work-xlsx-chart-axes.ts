import { attribute, directChild, directChildren, firstDescendant } from './work-ooxml-package';
import { cachedText, escapeXml, formulaReference, richText, stringCacheXml } from './work-xlsx-chart-values';
import {
  normalizeWorkSpreadsheetChartAxes,
  workSpreadsheetChartAxisDefaultLabelPosition,
  workSpreadsheetChartAxisIsCategoryAxis,
  workSpreadsheetChartAxisShowsMajorGridlinesByDefault,
} from './work-spreadsheet-chart-axis';
import {
  normalizeWorkSpreadsheetChartAxisGroup,
  type WorkSpreadsheetChart,
  type WorkSpreadsheetChartAxes,
  type WorkSpreadsheetChartAxis,
  type WorkSpreadsheetChartAxisPosition,
  type WorkSpreadsheetChartType,
  workSpreadsheetChartSupportsAxes,
  workSpreadsheetChartUsesNumericXAxis,
} from './work-types';

export function parseXlsxChartAxes(
  document: Document,
  chartType: WorkSpreadsheetChartType,
  hasSecondaryAxes: boolean,
  preserveVisibleDefaultGridlines = false
): WorkSpreadsheetChartAxes | undefined {
  if (!workSpreadsheetChartSupportsAxes(chartType)) return undefined;
  const plotArea = firstDescendant(document, 'plotArea');
  if (!plotArea) return undefined;
  const axes: WorkSpreadsheetChartAxes = {};
  for (const axis of directChildren(plotArea).filter(
    (node) => node.localName === 'catAx' || node.localName === 'valAx' || node.localName === 'dateAx'
  )) {
    const position = attribute(directChild(axis, 'axPos') ?? axis, 'val');
    if (position !== 'b' && position !== 'l' && position !== 't' && position !== 'r') continue;
    const key: WorkSpreadsheetChartAxisPosition =
      position === 'b' ? 'bottom' : position === 'l' ? 'left' : position === 't' ? 'top' : 'right';
    const title = directChild(axis, 'title');
    const text = title ? cachedText(title) || richText(title) || undefined : undefined;
    const titleReference = title ? formulaReference(title) : undefined;
    const displaySettings = parseXlsxAxisDisplaySettings(axis, chartType, key);
    const valueSettings =
      axis.localName === 'valAx'
        ? parseXlsxValueAxisSettings(axis, chartType, key, preserveVisibleDefaultGridlines)
        : undefined;
    if (!text && !titleReference && !displaySettings && !valueSettings) continue;
    axes[key] = {
      ...(text ? { title: text } : {}),
      ...(titleReference ? { titleReference } : {}),
      ...displaySettings,
      ...valueSettings,
    };
  }
  return normalizeWorkSpreadsheetChartAxes(axes, chartType, hasSecondaryAxes);
}

function parseXlsxAxisDisplaySettings(
  axis: Element,
  chartType: WorkSpreadsheetChartType,
  position: WorkSpreadsheetChartAxisPosition
): WorkSpreadsheetChartAxis | undefined {
  const orientation = attribute(directChild(directChild(axis, 'scaling') ?? axis, 'orientation') ?? axis, 'val');
  const rawLabelPosition = attribute(directChild(axis, 'tickLblPos') ?? axis, 'val');
  const labelPosition =
    rawLabelPosition === 'nextTo' ||
    rawLabelPosition === 'high' ||
    rawLabelPosition === 'low' ||
    rawLabelPosition === 'none'
      ? rawLabelPosition
      : undefined;
  const rawTickMark = attribute(directChild(axis, 'majorTickMark') ?? axis, 'val');
  const majorTickMark =
    rawTickMark === 'in'
      ? 'inside'
      : rawTickMark === 'out'
        ? 'outside'
        : rawTickMark === 'cross'
          ? 'cross'
          : rawTickMark === 'none'
            ? 'none'
            : undefined;
  const rawInterval = workSpreadsheetChartAxisIsCategoryAxis(chartType, position)
    ? finiteAttributeValue(directChild(axis, 'tickLblSkip'))
    : undefined;
  const labelInterval =
    Number.isInteger(rawInterval) && Number(rawInterval) >= 1 && Number(rawInterval) <= 31_999
      ? Number(rawInterval)
      : undefined;
  const defaultLabelPosition = workSpreadsheetChartAxisDefaultLabelPosition(chartType, position);
  const settings: WorkSpreadsheetChartAxis = {
    ...(orientation === 'maxMin' ? { reverseOrder: true } : {}),
    ...(labelPosition && labelPosition !== defaultLabelPosition ? { labelPosition } : {}),
    ...(majorTickMark && majorTickMark !== 'none' ? { majorTickMark } : {}),
    ...(labelInterval !== undefined ? { labelInterval } : {}),
  };
  return Object.keys(settings).length ? settings : undefined;
}

export function chartTextTitleXml(title: string, reference: string | undefined): string {
  const text = reference?.trim()
    ? `<c:strRef><c:f>${escapeXml(reference.replace(/^=/, ''))}</c:f>${stringCacheXml([title])}</c:strRef>`
    : [
        '<c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="zh-CN" sz="1400" b="0"/>',
        `<a:t>${escapeXml(title)}</a:t></a:r><a:endParaRPr lang="zh-CN"/></a:p></c:rich>`,
      ].join('');
  return `<c:title><c:tx>${text}</c:tx><c:layout/><c:overlay val="0"/></c:title>`;
}

function chartAxisScalingXml(axis: WorkSpreadsheetChartAxis | undefined, includeBounds: boolean): string {
  return [
    `<c:scaling><c:orientation val="${axis?.reverseOrder === true ? 'maxMin' : 'minMax'}"/>`,
    includeBounds && Number.isFinite(axis?.maximum) ? `<c:max val="${Number(axis?.maximum)}"/>` : '',
    includeBounds && Number.isFinite(axis?.minimum) ? `<c:min val="${Number(axis?.minimum)}"/>` : '',
    '</c:scaling>',
  ].join('');
}

function chartAxisTickLabelPositionXml(
  axis: WorkSpreadsheetChartAxis | undefined,
  chartType: WorkSpreadsheetChartType,
  position: WorkSpreadsheetChartAxisPosition
): string {
  const value = axis?.labelPosition ?? workSpreadsheetChartAxisDefaultLabelPosition(chartType, position);
  return `<c:tickLblPos val="${value}"/>`;
}

function chartAxisMajorTickMarkXml(axis: WorkSpreadsheetChartAxis | undefined): string {
  const value = axis?.majorTickMark;
  if (!value || value === 'none') return '';
  const nativeValue = value === 'inside' ? 'in' : value === 'outside' ? 'out' : 'cross';
  return `<c:majorTickMark val="${nativeValue}"/>`;
}

function chartCategoryAxisLabelIntervalXml(axis: WorkSpreadsheetChartAxis | undefined): string {
  return Number.isInteger(axis?.labelInterval) && Number(axis?.labelInterval) >= 1
    ? `<c:tickLblSkip val="${Number(axis?.labelInterval)}"/>`
    : '';
}

export function chartAxesXml(
  chart: WorkSpreadsheetChart,
  categoryAxisId: number,
  valueAxisId: number,
  secondaryCategoryAxisId: number,
  secondaryValueAxisId: number
): string {
  if (workSpreadsheetChartUsesNumericXAxis(chart.type)) {
    return xyChartAxesXml(chart, categoryAxisId, valueAxisId);
  }
  if (chart.type === 'combination') {
    return combinationChartAxesXml(chart, categoryAxisId, valueAxisId, secondaryCategoryAxisId, secondaryValueAxisId);
  }
  const horizontalBars = chart.type === 'bar';
  const categoryAxis = horizontalBars ? chart.axes?.left : chart.axes?.bottom;
  const categoryAxisPosition: WorkSpreadsheetChartAxisPosition = horizontalBars ? 'left' : 'bottom';
  const categoryAxisTitle = chartAxisTitleXml(categoryAxis);
  const valueAxisPosition: WorkSpreadsheetChartAxisPosition = horizontalBars ? 'bottom' : 'left';
  const valueAxis = horizontalBars ? chart.axes?.bottom : chart.axes?.left;
  return [
    '<c:catAx>',
    `<c:axId val="${categoryAxisId}"/>`,
    `${chartAxisScalingXml(categoryAxis, false)}<c:delete val="0"/>`,
    `<c:axPos val="${horizontalBars ? 'l' : 'b'}"/>`,
    categoryAxisTitle,
    chartAxisMajorTickMarkXml(categoryAxis),
    chartAxisTickLabelPositionXml(categoryAxis, chart.type, categoryAxisPosition),
    `<c:crossAx val="${valueAxisId}"/><c:crosses val="autoZero"/>`,
    `<c:auto val="1"/><c:lblAlgn val="ctr"/><c:lblOffset val="100"/>${chartCategoryAxisLabelIntervalXml(categoryAxis)}`,
    '</c:catAx>',
    '<c:valAx>',
    `<c:axId val="${valueAxisId}"/>`,
    `${chartAxisScalingXml(valueAxis, true)}<c:delete val="0"/>`,
    `<c:axPos val="${horizontalBars ? 'b' : 'l'}"/>`,
    chartValueAxisGridlinesXml(valueAxis, chart.type, valueAxisPosition),
    chartAxisTitleXml(valueAxis),
    `${chartValueAxisNumberFormatXml(valueAxis)}${chartAxisMajorTickMarkXml(
      valueAxis
    )}${chartAxisTickLabelPositionXml(valueAxis, chart.type, valueAxisPosition)}`,
    `<c:crossAx val="${categoryAxisId}"/><c:crosses val="autoZero"/><c:crossBetween val="between"/>`,
    chartValueAxisMajorUnitXml(valueAxis),
    '</c:valAx>',
  ].join('');
}

function parseXlsxValueAxisSettings(
  axis: Element,
  chartType: WorkSpreadsheetChartType,
  position: WorkSpreadsheetChartAxisPosition,
  preserveVisibleDefaultGridlines: boolean
): WorkSpreadsheetChartAxis | undefined {
  const scaling = directChild(axis, 'scaling');
  const minimum = finiteAttributeValue(scaling ? directChild(scaling, 'min') : null);
  const maximum = finiteAttributeValue(scaling ? directChild(scaling, 'max') : null);
  const majorUnit = finiteAttributeValue(directChild(axis, 'majorUnit'));
  const showMajorGridlines = directChildren(axis, 'majorGridlines').length === 1;
  const defaultGridlines = workSpreadsheetChartAxisShowsMajorGridlinesByDefault(chartType, position);
  const numberFormatNode = directChild(axis, 'numFmt');
  const numberFormat = attribute(numberFormatNode ?? axis, 'formatCode')?.trim();
  const numberFormatSourceLinked = numberFormatNode
    ? readBooleanValue(attribute(numberFormatNode, 'sourceLinked'))
    : undefined;
  const defaultNumberFormat = (!numberFormat || numberFormat === 'General') && numberFormatSourceLinked !== false;
  const settings: WorkSpreadsheetChartAxis = {
    ...(minimum !== undefined ? { minimum } : {}),
    ...(maximum !== undefined ? { maximum } : {}),
    ...(majorUnit !== undefined && majorUnit > 0 ? { majorUnit } : {}),
    ...(showMajorGridlines !== defaultGridlines || (showMajorGridlines && preserveVisibleDefaultGridlines)
      ? { showMajorGridlines }
      : {}),
    ...(!defaultNumberFormat && numberFormat ? { numberFormat } : {}),
    ...(!defaultNumberFormat && numberFormatSourceLinked !== undefined ? { numberFormatSourceLinked } : {}),
  };
  return Object.keys(settings).length ? settings : undefined;
}

function chartAxisTitleXml(axis: WorkSpreadsheetChartAxis | undefined): string {
  if (!axis) return '';
  const title = axis.title?.trim();
  if (!title && !axis.titleReference?.trim()) return '';
  return chartTextTitleXml(title ?? '', axis.titleReference);
}

function chartValueAxisGridlinesXml(
  axis: WorkSpreadsheetChartAxis | undefined,
  chartType: WorkSpreadsheetChartType,
  position: WorkSpreadsheetChartAxisPosition
): string {
  const visible = axis?.showMajorGridlines ?? workSpreadsheetChartAxisShowsMajorGridlinesByDefault(chartType, position);
  return visible ? '<c:majorGridlines/>' : '';
}

function chartValueAxisNumberFormatXml(axis: WorkSpreadsheetChartAxis | undefined): string {
  const formatCode = axis?.numberFormat?.trim() || 'General';
  const sourceLinked = axis?.numberFormat
    ? axis.numberFormatSourceLinked === true
    : axis?.numberFormatSourceLinked !== false;
  return `<c:numFmt formatCode="${escapeXml(formatCode)}" sourceLinked="${sourceLinked ? 1 : 0}"/>`;
}

function chartValueAxisMajorUnitXml(axis: WorkSpreadsheetChartAxis | undefined): string {
  return Number.isFinite(axis?.majorUnit) && Number(axis?.majorUnit) > 0
    ? `<c:majorUnit val="${Number(axis?.majorUnit)}"/>`
    : '';
}

function combinationChartAxesXml(
  chart: WorkSpreadsheetChart,
  categoryAxisId: number,
  valueAxisId: number,
  secondaryCategoryAxisId: number,
  secondaryValueAxisId: number
): string {
  const primaryCategoryAxis = chart.axes?.bottom;
  const primaryValueAxis = chart.axes?.left;
  const primary = [
    '<c:catAx>',
    `<c:axId val="${categoryAxisId}"/>`,
    `${chartAxisScalingXml(primaryCategoryAxis, false)}<c:delete val="0"/><c:axPos val="b"/>`,
    chartAxisTitleXml(primaryCategoryAxis),
    chartAxisMajorTickMarkXml(primaryCategoryAxis),
    chartAxisTickLabelPositionXml(primaryCategoryAxis, chart.type, 'bottom'),
    `<c:crossAx val="${valueAxisId}"/><c:crosses val="autoZero"/>`,
    `<c:auto val="1"/><c:lblAlgn val="ctr"/><c:lblOffset val="100"/>${chartCategoryAxisLabelIntervalXml(
      primaryCategoryAxis
    )}`,
    '</c:catAx>',
    '<c:valAx>',
    `<c:axId val="${valueAxisId}"/>`,
    `${chartAxisScalingXml(primaryValueAxis, true)}<c:delete val="0"/><c:axPos val="l"/>`,
    chartValueAxisGridlinesXml(primaryValueAxis, chart.type, 'left'),
    chartAxisTitleXml(primaryValueAxis),
    `${chartValueAxisNumberFormatXml(primaryValueAxis)}${chartAxisMajorTickMarkXml(
      primaryValueAxis
    )}${chartAxisTickLabelPositionXml(primaryValueAxis, chart.type, 'left')}`,
    `<c:crossAx val="${categoryAxisId}"/><c:crosses val="autoZero"/><c:crossBetween val="between"/>`,
    chartValueAxisMajorUnitXml(primaryValueAxis),
    '</c:valAx>',
  ].join('');
  const hasSecondary = chart.series.some(
    (series) => normalizeWorkSpreadsheetChartAxisGroup(series.axisGroup) === 'secondary'
  );
  if (!hasSecondary) return primary;
  const secondaryCategoryAxis = chart.axes?.top;
  const secondaryValueAxis = chart.axes?.right;
  const secondaryCategoryLabelPosition =
    secondaryCategoryAxis?.labelPosition ?? workSpreadsheetChartAxisDefaultLabelPosition(chart.type, 'top');
  const secondary = [
    '<c:catAx>',
    `<c:axId val="${secondaryCategoryAxisId}"/>`,
    `${chartAxisScalingXml(secondaryCategoryAxis, false)}<c:delete val="${
      secondaryCategoryLabelPosition === 'none' ? 1 : 0
    }"/><c:axPos val="t"/>`,
    chartAxisTitleXml(secondaryCategoryAxis),
    chartAxisMajorTickMarkXml(secondaryCategoryAxis),
    chartAxisTickLabelPositionXml(secondaryCategoryAxis, chart.type, 'top'),
    `<c:crossAx val="${secondaryValueAxisId}"/><c:crosses val="max"/>`,
    `<c:auto val="1"/><c:lblAlgn val="ctr"/><c:lblOffset val="100"/>${chartCategoryAxisLabelIntervalXml(
      secondaryCategoryAxis
    )}`,
    '</c:catAx>',
    '<c:valAx>',
    `<c:axId val="${secondaryValueAxisId}"/>`,
    `${chartAxisScalingXml(secondaryValueAxis, true)}<c:delete val="0"/><c:axPos val="r"/>`,
    chartValueAxisGridlinesXml(secondaryValueAxis, chart.type, 'right'),
    chartAxisTitleXml(secondaryValueAxis),
    `${chartValueAxisNumberFormatXml(secondaryValueAxis)}${chartAxisMajorTickMarkXml(
      secondaryValueAxis
    )}${chartAxisTickLabelPositionXml(secondaryValueAxis, chart.type, 'right')}`,
    `<c:crossAx val="${secondaryCategoryAxisId}"/><c:crosses val="max"/><c:crossBetween val="between"/>`,
    chartValueAxisMajorUnitXml(secondaryValueAxis),
    '</c:valAx>',
  ].join('');
  return `${primary}${secondary}`;
}

function xyChartAxesXml(chart: WorkSpreadsheetChart, horizontalAxisId: number, verticalAxisId: number): string {
  const horizontalAxis = chart.axes?.bottom;
  const verticalAxis = chart.axes?.left;
  return [
    '<c:valAx>',
    `<c:axId val="${horizontalAxisId}"/>`,
    `${chartAxisScalingXml(horizontalAxis, true)}<c:delete val="0"/>`,
    '<c:axPos val="b"/>',
    chartValueAxisGridlinesXml(horizontalAxis, chart.type, 'bottom'),
    chartAxisTitleXml(horizontalAxis),
    `${chartValueAxisNumberFormatXml(horizontalAxis)}${chartAxisMajorTickMarkXml(
      horizontalAxis
    )}${chartAxisTickLabelPositionXml(horizontalAxis, chart.type, 'bottom')}`,
    `<c:crossAx val="${verticalAxisId}"/><c:crosses val="autoZero"/><c:crossBetween val="midCat"/>`,
    chartValueAxisMajorUnitXml(horizontalAxis),
    '</c:valAx>',
    '<c:valAx>',
    `<c:axId val="${verticalAxisId}"/>`,
    `${chartAxisScalingXml(verticalAxis, true)}<c:delete val="0"/>`,
    '<c:axPos val="l"/>',
    chartValueAxisGridlinesXml(verticalAxis, chart.type, 'left'),
    chartAxisTitleXml(verticalAxis),
    `${chartValueAxisNumberFormatXml(verticalAxis)}${chartAxisMajorTickMarkXml(
      verticalAxis
    )}${chartAxisTickLabelPositionXml(verticalAxis, chart.type, 'left')}`,
    `<c:crossAx val="${horizontalAxisId}"/><c:crosses val="autoZero"/><c:crossBetween val="midCat"/>`,
    chartValueAxisMajorUnitXml(verticalAxis),
    '</c:valAx>',
  ].join('');
}

function finiteAttributeValue(element: Element | null | undefined): number | undefined {
  const source = element ? attribute(element, 'val') : null;
  const value = source?.trim() ? Number(source) : Number.NaN;
  return Number.isFinite(value) ? value : undefined;
}

function readBooleanValue(value: string | null): boolean {
  return value === '1' || value === 'true';
}
