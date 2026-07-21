import { attribute, descendants, directChildren, firstDescendant, OoxmlPackage } from './work-ooxml-package';
import type { WorkCompatibilityIssue } from './work-types';
import { isSupportedXlsxCombinationChartNodes } from './work-xlsx-charts';
import {
  isSupportedXlsxChartLegend,
  isSupportedXlsxChartPlotLayout,
  xlsxChartNodeUsesStackedGrouping,
} from './work-xlsx-chart-layout';
import {
  isSupportedXlsxChartSeriesFormatting,
  xlsxChartSeriesFormattingShapeProperties,
} from './work-xlsx-chart-series-style';

const SUPPORTED_CHART_ELEMENTS = new Set([
  'barChart',
  'lineChart',
  'pieChart',
  'doughnutChart',
  'areaChart',
  'radarChart',
  'scatterChart',
  'bubbleChart',
]);
const TRENDLINE_CHART_ELEMENTS = new Set(['barChart', 'lineChart', 'areaChart', 'scatterChart', 'bubbleChart']);
const ERROR_BAR_CHART_ELEMENTS = new Set(['barChart', 'lineChart', 'areaChart', 'scatterChart', 'bubbleChart']);
const ERROR_BAR_VALUE_TYPES = new Set(['cust', 'fixedVal', 'percentage', 'stdDev', 'stdErr']);
const ERROR_BAR_CHILDREN = new Set(['errDir', 'errBarType', 'errValType', 'noEndCap', 'plus', 'minus', 'val']);
const SUPPORTED_TRENDLINE_TYPES = new Set(['exp', 'linear', 'log', 'movingAvg', 'poly', 'power']);
const SUPPORTED_TRENDLINE_CHILDREN = new Set([
  'name',
  'spPr',
  'trendlineType',
  'order',
  'period',
  'forward',
  'backward',
  'intercept',
  'dispRSq',
  'dispEq',
  'trendlineLbl',
]);
const SUPPORTED_DATA_LABEL_CHILDREN = new Set([
  'delete',
  'spPr',
  'txPr',
  'dLblPos',
  'showLegendKey',
  'showVal',
  'showCatName',
  'showSerName',
  'showPercent',
  'showBubbleSize',
  'separator',
  'showLeaderLines',
]);
const SUPPORTED_DATA_LABEL_POSITIONS = new Set(['bestFit', 'b', 'ctr', 'inBase', 'inEnd', 'l', 'outEnd', 'r', 't']);

export async function diagnoseXlsxCharts(archive: OoxmlPackage): Promise<WorkCompatibilityIssue[]> {
  const chartPaths = archive.paths('xl/charts/').filter((path) => /^xl\/charts\/chart\d+\.xml$/i.test(path));
  if (!chartPaths.length) return [];
  const anchoredPaths = await anchoredChartPaths(archive);
  let supported = 0;
  let unsupportedTypes = 0;
  let combinations = 0;
  let trendlines = 0;
  let errorBars = 0;
  let dataLabels = 0;
  let advancedFormatting = 0;
  let advancedPlotLayout = 0;
  let unsupportedLegends = 0;
  let advancedAxes = 0;
  let axisTitles = 0;
  let axisSettings = 0;
  let categoryAxisSettings = 0;
  let unsupportedReferences = 0;
  let externalData = 0;

  for (const path of chartPaths) {
    const chart = await archive.xml(path);
    const plotArea = firstDescendant(chart, 'plotArea');
    const chartNodes = plotArea ? directChildren(plotArea).filter((element) => /Chart$/i.test(element.localName)) : [];
    const supportedCombination = isSupportedXlsxCombinationChartNodes(chartNodes);
    if (chartNodes.length > 1 && !supportedCombination) combinations += 1;
    if (
      ((chartNodes.length === 1 && SUPPORTED_CHART_ELEMENTS.has(chartNodes[0].localName)) || supportedCombination) &&
      anchoredPaths.has(path)
    ) {
      supported += 1;
    } else {
      unsupportedTypes += 1;
    }
    trendlines += unsupportedTrendlineCount(chartNodes);
    errorBars += unsupportedErrorBarCount(chartNodes);
    dataLabels += unsupportedDataLabelCount(chartNodes);
    if (hasAdvancedChartFormatting(chart, chartNodes)) advancedFormatting += 1;
    if (chartNodes.some((node) => !isSupportedXlsxChartPlotLayout(node))) advancedPlotLayout += 1;
    if (!isSupportedXlsxChartLegend(chart)) unsupportedLegends += 1;
    if (hasAdvancedAxes(chart, chartNodes)) advancedAxes += 1;
    axisTitles += unsupportedAxisTitleCount(chart, chartNodes);
    axisSettings += unsupportedValueAxisSettingsCount(chart);
    categoryAxisSettings += unsupportedCategoryAxisSettingsCount(chart);
    if (
      descendants(chart, 'multiLvlStrRef').length ||
      descendants(chart, 'f').some((formula) => !isSupportedChartReference(formula.textContent ?? ''))
    ) {
      unsupportedReferences += 1;
    }
    if (descendants(chart, 'externalData').length) externalData += 1;
  }

  const issues: WorkCompatibilityIssue[] = [];
  if (supported) {
    issues.push(
      issue(
        'xlsx.charts',
        `${supported} anchored column, bar, line, pie, doughnut, area, radar, scatter, bubble, or column/line/area combination chart(s), cached data, live cell references, simple series fills, transparency, lines, dashes, and markers, data labels, error bars, trendlines, chart and axis titles, axis order, label positions, major tick marks, category-label intervals, value-axis bounds, major units, major gridlines, number formats, legend positions and overlay, grouping, stacking, plot spacing, smoothing, names, alternative text, positions, sizes, and supported primary or secondary axes are preserved and editable.`,
        'info'
      )
    );
  }
  if (unsupportedTypes) {
    issues.push(
      issue(
        'xlsx.charts.unsupported-type',
        `${unsupportedTypes} unsupported, unanchored, 3D, stock, surface, or other chart part(s) cannot be edited and will not be included in a regenerated XLSX.`
      )
    );
  }
  if (combinations) {
    issues.push(
      issue(
        'xlsx.charts.combination',
        `${combinations} combination chart(s) include unsupported plot families or axis layouts; Work does not flatten them into a misleading editable chart.`
      )
    );
  }
  if (trendlines) {
    issues.push(
      issue(
        'xlsx.charts.trendline',
        `${trendlines} trendline(s) use an unsupported chart family, type, parameter, extension, or manual label setting and cannot be represented exactly by the editable chart model.`
      )
    );
  }
  if (errorBars) {
    issues.push(
      issue(
        'xlsx.charts.error-bars',
        `${errorBars} error-bar setting(s) use an unsupported chart family, direction, type, value, custom range, duplicate direction, formatting, or extension and cannot be represented exactly by the editable chart model.`
      )
    );
  }
  if (dataLabels) {
    issues.push(
      issue(
        'xlsx.charts.data-labels',
        `${dataLabels} data-label setting(s) use per-point overrides, legend keys, number formats, leader lines, extensions, invalid values, or chart-specific content that cannot be represented exactly by the editable chart model.`
      )
    );
  }
  if (advancedAxes) {
    issues.push(
      issue(
        'xlsx.charts.axes',
        `${advancedAxes} chart(s) use date, series, or additional axes beyond the supported category/value and one-primary/one-secondary combination layouts.`
      )
    );
  }
  if (axisTitles) {
    issues.push(
      issue(
        'xlsx.charts.axis-titles',
        `${axisTitles} axis-title setting(s) use an unsupported axis position, duplicate title, manual layout, overlay, rich-text structure, field, line break, or extension and cannot be represented exactly by the editable chart model.`
      )
    );
  }
  if (axisSettings) {
    issues.push(
      issue(
        'xlsx.charts.axis-settings',
        `${axisSettings} value-axis setting(s) use a logarithmic scale, invalid bounds or major unit, minor unit or gridlines, display units, unsupported crossing or minor ticks, malformed label or major-tick placement, malformed number format, or extension that cannot be represented exactly by the editable chart model.`
      )
    );
  }
  if (categoryAxisSettings) {
    issues.push(
      issue(
        'xlsx.charts.category-axis-settings',
        `${categoryAxisSettings} category-axis setting(s) use an invalid orientation, label position, major tick, label interval, tick interval, alignment, offset, crossing, number format, extension, or another setting that cannot be represented exactly by the editable chart model.`
      )
    );
  }
  if (unsupportedLegends) {
    issues.push(
      issue(
        'xlsx.charts.legend',
        `${unsupportedLegends} chart(s) use a manual legend layout, per-entry override, invalid position or overlay, formatting, extension, or duplicate legend setting that cannot be represented exactly by the editable chart model.`
      )
    );
  }
  if (advancedPlotLayout) {
    issues.push(
      issue(
        'xlsx.charts.layout',
        `${advancedPlotLayout} chart(s) use an invalid grouping, gap, overlap, mixed per-series smoothing, 3D bubble layout, explosion, slice rotation, unsupported combination layout, or another plot setting that cannot be represented exactly by the editable chart model.`
      )
    );
  }
  if (unsupportedReferences) {
    issues.push(
      issue(
        'xlsx.charts.references',
        `${unsupportedReferences} chart(s) use external, named, multi-level, union, or otherwise unsupported data references; only cached values can be previewed where available.`
      )
    );
  }
  if (externalData) {
    issues.push(
      issue(
        'xlsx.charts.external-data',
        `${externalData} chart(s) reference an embedded or external workbook; Work uses worksheet references and cached values without refreshing external data.`
      )
    );
  }
  if (advancedFormatting && supported) {
    issues.push(
      issue(
        'xlsx.charts.format',
        `${advancedFormatting} chart(s) contain theme, label, fill, line, effect, wall, or other advanced formatting that is normalized to the Work chart style on export.`
      )
    );
  }
  return issues;
}

async function anchoredChartPaths(archive: OoxmlPackage): Promise<Set<string>> {
  const paths = new Set<string>();
  const drawings = archive.paths('xl/drawings/').filter((path) => /^xl\/drawings\/drawing\d+\.xml$/i.test(path));
  for (const path of drawings) {
    const document = await archive.xml(path);
    const relationships = await archive.relationships(path);
    for (const frame of descendants(document, 'graphicFrame')) {
      const reference = firstDescendant(frame, 'chart');
      const relationshipId = reference ? (attribute(reference, 'r:id') ?? attribute(reference, 'id')) : null;
      const relationship = relationshipId ? relationships.get(relationshipId) : undefined;
      if (
        relationship &&
        relationship.targetMode !== 'External' &&
        relationship.type.endsWith('/chart') &&
        archive.has(relationship.target)
      ) {
        paths.add(relationship.target);
      }
    }
  }
  return paths;
}

function hasAdvancedChartFormatting(document: Document, chartNodes: Element[]): boolean {
  if (
    ['upDownBars', 'pictureOptions', 'view3D', 'floor', 'sideWall', 'backWall'].some(
      (name) => descendants(document, name).length > 0
    ) ||
    descendants(document, 'txPr').length > 0 ||
    descendants(document, 'style').length > 0
  ) {
    return true;
  }
  const supportedShapeProperties = new Set<Element>();
  for (const chart of chartNodes) {
    for (const series of directChildren(chart, 'ser')) {
      if (!isSupportedXlsxChartSeriesFormatting(series, chart.localName)) return true;
      for (const shape of xlsxChartSeriesFormattingShapeProperties(series)) supportedShapeProperties.add(shape);
    }
  }
  return descendants(document, 'spPr').some((shape) => !supportedShapeProperties.has(shape));
}

function isSupportedChartReference(value: string): boolean {
  const formula = value.trim().replace(/^=/, '');
  if (formula.includes('[') || formula.includes(']')) return false;
  return /^(?:(?:'(?:(?:[^']|'')+)'|[^!]+)!)?\$?[A-Z]{1,3}\$?[1-9]\d*(?::\$?[A-Z]{1,3}\$?[1-9]\d*)?$/i.test(formula);
}

function unsupportedTrendlineCount(chartNodes: Element[]): number {
  return chartNodes.reduce((count, chart) => {
    const supportsTrendlines =
      TRENDLINE_CHART_ELEMENTS.has(chart.localName) && !xlsxChartNodeUsesStackedGrouping(chart);
    const trendlines = directChildren(chart, 'ser').flatMap((series) => directChildren(series, 'trendline'));
    return count + trendlines.filter((trendline) => !supportsTrendlines || !isSupportedTrendline(trendline)).length;
  }, 0);
}

function unsupportedDataLabelCount(chartNodes: Element[]): number {
  return chartNodes.reduce((count, chart) => {
    const dataLabels = [
      ...directChildren(chart, 'dLbls'),
      ...directChildren(chart, 'ser').flatMap((series) => directChildren(series, 'dLbls')),
    ];
    return count + dataLabels.filter((labels) => !isSupportedDataLabels(labels, chart.localName)).length;
  }, 0);
}

function unsupportedErrorBarCount(chartNodes: Element[]): number {
  return chartNodes.reduce((count, chart) => {
    const supportsErrorBars = ERROR_BAR_CHART_ELEMENTS.has(chart.localName) && !xlsxChartNodeUsesStackedGrouping(chart);
    const seriesNodes = directChildren(chart, 'ser');
    const directErrorBars = seriesNodes.flatMap((series) => directChildren(series, 'errBars'));
    const misplacedErrorBars = Math.max(0, descendants(chart, 'errBars').length - directErrorBars.length);
    const unsupported = seriesNodes.reduce((seriesCount, series) => {
      const directions = new Set<string>();
      return (
        seriesCount +
        directChildren(series, 'errBars').filter((errorBars) => {
          const direction = errorBarDirection(errorBars);
          const duplicateDirection = Boolean(direction && directions.has(direction));
          if (direction) directions.add(direction);
          return duplicateDirection || !supportsErrorBars || !isSupportedErrorBars(errorBars, chart.localName);
        }).length
      );
    }, 0);
    return count + misplacedErrorBars + unsupported;
  }, 0);
}

function isSupportedErrorBars(errorBars: Element, chartLocalName: string): boolean {
  const children = directChildren(errorBars);
  if (children.some((child) => !ERROR_BAR_CHILDREN.has(child.localName))) return false;
  const direction = errorBarDirection(errorBars);
  if (!direction) return false;
  if (direction === 'x' && chartLocalName !== 'scatterChart' && chartLocalName !== 'bubbleChart') return false;

  const barTypeElements = directChildren(errorBars, 'errBarType');
  if (barTypeElements.length !== 1) return false;
  const barType = attribute(barTypeElements[0], 'val');
  if (barType !== 'both' && barType !== 'plus' && barType !== 'minus') return false;

  const valueTypeElements = directChildren(errorBars, 'errValType');
  if (valueTypeElements.length !== 1) return false;
  const valueType = attribute(valueTypeElements[0], 'val');
  if (!valueType || !ERROR_BAR_VALUE_TYPES.has(valueType)) return false;
  if (!validOptionalBooleanAttribute(errorBars, 'noEndCap')) return false;

  const values = directChildren(errorBars, 'val');
  if (values.length > 1) return false;
  const usesScalarValue = valueType === 'fixedVal' || valueType === 'percentage' || valueType === 'stdDev';
  if (values.length && (!usesScalarValue || !validNonNegativeNumberAttribute(values[0]))) return false;

  const plus = directChildren(errorBars, 'plus');
  const minus = directChildren(errorBars, 'minus');
  if (plus.length > 1 || minus.length > 1) return false;
  if (valueType !== 'cust') return plus.length === 0 && minus.length === 0;
  if (values.length) return false;
  if (barType !== 'minus' && (plus.length !== 1 || !isSupportedErrorBarSource(plus[0]))) return false;
  if (barType === 'minus' && plus.length) return false;
  if (barType !== 'plus' && (minus.length !== 1 || !isSupportedErrorBarSource(minus[0]))) return false;
  return barType !== 'plus' || minus.length === 0;
}

function errorBarDirection(errorBars: Element): string | null {
  const elements = directChildren(errorBars, 'errDir');
  if (elements.length !== 1) return null;
  const direction = attribute(elements[0], 'val');
  return direction === 'x' || direction === 'y' ? direction : null;
}

function isSupportedErrorBarSource(source: Element): boolean {
  const children = directChildren(source);
  if (children.length !== 1 || (children[0].localName !== 'numRef' && children[0].localName !== 'numLit')) {
    return false;
  }
  const data = children[0];
  if (data.localName === 'numRef') {
    const dataChildren = directChildren(data);
    if (dataChildren.some((child) => child.localName !== 'f' && child.localName !== 'numCache')) return false;
    const formulas = directChildren(data, 'f');
    const caches = directChildren(data, 'numCache');
    if (formulas.length !== 1 || caches.length > 1 || !isSupportedChartReference(formulas[0].textContent ?? '')) {
      return false;
    }
    return !caches.length || isSupportedErrorBarNumberCache(caches[0], false);
  }
  return isSupportedErrorBarNumberCache(data, true);
}

function isSupportedErrorBarNumberCache(cache: Element, requireValues: boolean): boolean {
  const children = directChildren(cache);
  if (children.some((child) => !['formatCode', 'ptCount', 'pt'].includes(child.localName))) return false;
  if (directChildren(cache, 'formatCode').length > 1 || directChildren(cache, 'ptCount').length > 1) return false;
  const points = directChildren(cache, 'pt');
  if (requireValues && !points.length) return false;
  const indexes = new Set<number>();
  for (const point of points) {
    const index = Number(attribute(point, 'idx'));
    const values = directChildren(point, 'v');
    if (!Number.isInteger(index) || index < 0 || indexes.has(index) || values.length !== 1) return false;
    indexes.add(index);
    const value = Number(values[0].textContent);
    if (!Number.isFinite(value) || value < 0) return false;
  }
  const pointCounts = directChildren(cache, 'ptCount');
  if (!pointCounts.length) return true;
  const pointCount = Number(attribute(pointCounts[0], 'val'));
  return Number.isInteger(pointCount) && pointCount >= points.length;
}

function validNonNegativeNumberAttribute(element: Element): boolean {
  const raw = attribute(element, 'val');
  const value = raw === null || raw.trim() === '' ? Number.NaN : Number(raw);
  return Number.isFinite(value) && value >= 0;
}

function isSupportedDataLabels(labels: Element, chartLocalName: string): boolean {
  const children = directChildren(labels);
  if (children.some((child) => !SUPPORTED_DATA_LABEL_CHILDREN.has(child.localName))) return false;
  if (['spPr', 'txPr', 'separator'].some((name) => directChildren(labels, name).length > 1)) return false;
  const positions = directChildren(labels, 'dLblPos');
  if (
    positions.length > 1 ||
    (positions.length === 1 && !SUPPORTED_DATA_LABEL_POSITIONS.has(attribute(positions[0], 'val') ?? ''))
  ) {
    return false;
  }
  const booleanNames = [
    'delete',
    'showLegendKey',
    'showVal',
    'showCatName',
    'showSerName',
    'showPercent',
    'showBubbleSize',
    'showLeaderLines',
  ];
  if (booleanNames.some((name) => !validOptionalBooleanAttribute(labels, name))) return false;
  if (booleanElementIsTrue(labels, 'showLegendKey') || booleanElementIsTrue(labels, 'showLeaderLines')) return false;
  if (
    booleanElementIsTrue(labels, 'showPercent') &&
    chartLocalName !== 'pieChart' &&
    chartLocalName !== 'doughnutChart'
  ) {
    return false;
  }
  return !booleanElementIsTrue(labels, 'showBubbleSize') || chartLocalName === 'bubbleChart';
}

function booleanElementIsTrue(parent: Element, localName: string): boolean {
  const element = directChildren(parent, localName)[0];
  if (!element) return false;
  const value = attribute(element, 'val');
  return value === null || value === '1' || value === 'true';
}

function isSupportedTrendline(trendline: Element): boolean {
  const children = directChildren(trendline);
  if (children.some((child) => !SUPPORTED_TRENDLINE_CHILDREN.has(child.localName))) return false;
  if (directChildren(trendline, 'name').length > 1 || directChildren(trendline, 'spPr').length > 1) return false;

  const typeElements = directChildren(trendline, 'trendlineType');
  if (typeElements.length !== 1) return false;
  const type = attribute(typeElements[0], 'val');
  if (!type || !SUPPORTED_TRENDLINE_TYPES.has(type)) return false;

  const order = directChildren(trendline, 'order');
  if (type === 'poly') {
    if (order.length !== 1 || !validIntegerAttribute(order[0], 2, 6)) return false;
  } else if (order.length) {
    return false;
  }

  const period = directChildren(trendline, 'period');
  if (type === 'movingAvg') {
    if (period.length !== 1 || !validIntegerAttribute(period[0], 2, 255)) return false;
  } else if (period.length) {
    return false;
  }

  if (!validOptionalNumberAttribute(trendline, 'forward', (value) => value >= 0)) return false;
  if (!validOptionalNumberAttribute(trendline, 'backward', (value) => value >= 0)) return false;
  if (!validOptionalNumberAttribute(trendline, 'intercept', () => true)) return false;
  if (!validOptionalBooleanAttribute(trendline, 'dispEq')) return false;
  if (!validOptionalBooleanAttribute(trendline, 'dispRSq')) return false;

  const labels = directChildren(trendline, 'trendlineLbl');
  if (labels.length > 1) return false;
  return !labels.some((label) => ['manualLayout', 'tx', 'numFmt'].some((name) => descendants(label, name).length > 0));
}

function validIntegerAttribute(element: Element, minimum: number, maximum: number): boolean {
  const value = Number(attribute(element, 'val'));
  return Number.isInteger(value) && value >= minimum && value <= maximum;
}

function validOptionalNumberAttribute(
  parent: Element,
  localName: string,
  predicate: (value: number) => boolean
): boolean {
  const elements = directChildren(parent, localName);
  if (elements.length > 1) return false;
  if (!elements.length) return true;
  const raw = attribute(elements[0], 'val');
  const value = raw === null || raw.trim() === '' ? Number.NaN : Number(raw);
  return Number.isFinite(value) && predicate(value);
}

function validOptionalBooleanAttribute(parent: Element, localName: string): boolean {
  const elements = directChildren(parent, localName);
  if (elements.length > 1) return false;
  if (!elements.length) return true;
  const value = attribute(elements[0], 'val');
  return value === null || ['0', '1', 'false', 'true'].includes(value);
}

function hasAdvancedAxes(document: Document, chartNodes: Element[]): boolean {
  if (descendants(document, 'serAx').length || descendants(document, 'dateAx').length) return true;
  const valueAxes = descendants(document, 'valAx').length;
  const categoryAxes = descendants(document, 'catAx').length;
  if (isSupportedXlsxCombinationChartNodes(chartNodes)) return valueAxes > 2 || categoryAxes > 2;
  const chartType = chartNodes.length === 1 ? chartNodes[0].localName : '';
  const expectedValueAxes =
    chartType === 'scatterChart' || chartType === 'bubbleChart'
      ? 2
      : chartType === 'pieChart' || chartType === 'doughnutChart'
        ? 0
        : 1;
  const expectedCategoryAxes =
    chartType === 'scatterChart' ||
    chartType === 'bubbleChart' ||
    chartType === 'pieChart' ||
    chartType === 'doughnutChart'
      ? 0
      : 1;
  return valueAxes > expectedValueAxes || categoryAxes > expectedCategoryAxes;
}

function unsupportedAxisTitleCount(document: Document, chartNodes: Element[]): number {
  const plotArea = firstDescendant(document, 'plotArea');
  if (!plotArea) return 0;
  const axes = directChildren(plotArea).filter((axis) => ['catAx', 'valAx', 'serAx'].includes(axis.localName));
  const supportedCombination = isSupportedXlsxCombinationChartNodes(chartNodes);
  const chartType = chartNodes.length === 1 ? chartNodes[0].localName : '';
  const supportsAxisTitles =
    supportedCombination ||
    (SUPPORTED_CHART_ELEMENTS.has(chartType) && chartType !== 'pieChart' && chartType !== 'doughnutChart');
  const supportsSecondaryTitles = supportedCombination;
  const positions = new Set<string>();
  return axes.reduce((count, axis) => {
    const titles = directChildren(axis, 'title');
    if (!titles.length) return count;
    const positionElements = directChildren(axis, 'axPos');
    const position = positionElements.length === 1 ? attribute(positionElements[0], 'val') : null;
    const positionSupported =
      position === 'b' || position === 'l' || (supportsSecondaryTitles && (position === 't' || position === 'r'));
    const duplicatePosition = Boolean(position && positions.has(position));
    if (position) positions.add(position);
    const supported =
      supportsAxisTitles &&
      axis.localName !== 'serAx' &&
      titles.length === 1 &&
      positionSupported &&
      !duplicatePosition &&
      isSupportedAxisTitle(titles[0]);
    return count + (supported ? 0 : titles.length);
  }, 0);
}

function unsupportedValueAxisSettingsCount(document: Document): number {
  const plotArea = firstDescendant(document, 'plotArea');
  if (!plotArea) return 0;
  const hasCategoryAxes = directChildren(plotArea, 'catAx').length > 0;
  return directChildren(plotArea, 'valAx').filter((axis) => !isSupportedValueAxisSettings(axis, hasCategoryAxes))
    .length;
}

function isSupportedValueAxisSettings(axis: Element, hasCategoryAxes: boolean): boolean {
  const scaling = directChildren(axis, 'scaling');
  if (scaling.length > 1) return false;
  if (scaling.length === 1) {
    const children = directChildren(scaling[0]);
    if (children.some((child) => !['orientation', 'max', 'min'].includes(child.localName))) return false;
    const orientation = directChildren(scaling[0], 'orientation');
    if (
      orientation.length > 1 ||
      orientation.some((item) => !['minMax', 'maxMin'].includes(attribute(item, 'val') ?? ''))
    ) {
      return false;
    }
    const minimum = axisSettingNumber(scaling[0], 'min');
    const maximum = axisSettingNumber(scaling[0], 'max');
    if (!minimum.valid || !maximum.valid) return false;
    if (minimum.value !== undefined && maximum.value !== undefined && minimum.value >= maximum.value) return false;
  }
  const majorUnit = axisSettingNumber(axis, 'majorUnit');
  if (!majorUnit.valid || (majorUnit.value !== undefined && majorUnit.value <= 0)) return false;
  if (directChildren(axis, 'majorGridlines').length > 1) return false;
  if (
    ['minorUnit', 'minorGridlines', 'dispUnits', 'crossAt', 'extLst'].some(
      (name) => directChildren(axis, name).length > 0
    )
  ) {
    return false;
  }
  const majorTicks = directChildren(axis, 'majorTickMark');
  if (
    majorTicks.length > 1 ||
    majorTicks.some((tick) => !['none', 'in', 'out', 'cross'].includes(attribute(tick, 'val') ?? ''))
  ) {
    return false;
  }
  const minorTicks = directChildren(axis, 'minorTickMark');
  if (minorTicks.length > 1 || minorTicks.some((tick) => attribute(tick, 'val') !== 'none')) return false;
  const tickLabelPositions = directChildren(axis, 'tickLblPos');
  if (
    tickLabelPositions.length > 1 ||
    tickLabelPositions.some((position) => !['nextTo', 'high', 'low', 'none'].includes(attribute(position, 'val') ?? ''))
  ) {
    return false;
  }
  const crosses = directChildren(axis, 'crosses');
  const axisPosition = attribute(directChildren(axis, 'axPos')[0] ?? axis, 'val');
  if (crosses.length > 1) return false;
  if (
    crosses.some((crossing) => {
      const value = attribute(crossing, 'val');
      return value !== 'autoZero' && !(value === 'max' && axisPosition === 'r');
    })
  )
    return false;
  const crossBetween = directChildren(axis, 'crossBetween');
  const expectedCrossBetween = hasCategoryAxes ? 'between' : 'midCat';
  if (crossBetween.length > 1 || crossBetween.some((crossing) => attribute(crossing, 'val') !== expectedCrossBetween)) {
    return false;
  }
  const deletion = directChildren(axis, 'delete');
  if (deletion.length > 1 || deletion.some(booleanAttributeIsTrue)) return false;
  const numberFormats = directChildren(axis, 'numFmt');
  if (numberFormats.length > 1) return false;
  if (numberFormats.length === 1) {
    const formatCode = attribute(numberFormats[0], 'formatCode');
    const sourceLinked = attribute(numberFormats[0], 'sourceLinked');
    if (
      !formatCode?.trim() ||
      formatCode.length > 255 ||
      !sourceLinked ||
      !['0', '1', 'false', 'true'].includes(sourceLinked)
    ) {
      return false;
    }
  }
  return true;
}

function unsupportedCategoryAxisSettingsCount(document: Document): number {
  const plotArea = firstDescendant(document, 'plotArea');
  if (!plotArea) return 0;
  return directChildren(plotArea, 'catAx').filter((axis) => !isSupportedCategoryAxisSettings(axis)).length;
}

function isSupportedCategoryAxisSettings(axis: Element): boolean {
  const scaling = directChildren(axis, 'scaling');
  if (scaling.length > 1) return false;
  if (scaling.length === 1) {
    const children = directChildren(scaling[0]);
    if (children.some((child) => child.localName !== 'orientation')) return false;
    const orientations = directChildren(scaling[0], 'orientation');
    if (
      orientations.length > 1 ||
      orientations.some((item) => !['minMax', 'maxMin'].includes(attribute(item, 'val') ?? ''))
    ) {
      return false;
    }
  }
  const majorTicks = directChildren(axis, 'majorTickMark');
  if (
    majorTicks.length > 1 ||
    majorTicks.some((tick) => !['none', 'in', 'out', 'cross'].includes(attribute(tick, 'val') ?? ''))
  ) {
    return false;
  }
  const minorTicks = directChildren(axis, 'minorTickMark');
  if (minorTicks.length > 1 || minorTicks.some((tick) => attribute(tick, 'val') !== 'none')) return false;
  const labelPositions = directChildren(axis, 'tickLblPos');
  if (
    labelPositions.length > 1 ||
    labelPositions.some((item) => !['nextTo', 'high', 'low', 'none'].includes(attribute(item, 'val') ?? ''))
  ) {
    return false;
  }
  const labelIntervals = directChildren(axis, 'tickLblSkip');
  if (labelIntervals.length > 1 || labelIntervals.some((item) => !validIntegerAttribute(item, 1, 31_999))) {
    return false;
  }
  if (
    ['tickMarkSkip', 'numFmt', 'majorGridlines', 'minorGridlines', 'crossAt', 'noMultiLvlLbl', 'extLst'].some(
      (name) => directChildren(axis, name).length > 0
    )
  ) {
    return false;
  }
  const crosses = directChildren(axis, 'crosses');
  const axisPosition = attribute(directChildren(axis, 'axPos')[0] ?? axis, 'val');
  if (crosses.length > 1) return false;
  if (
    crosses.some((item) => {
      const value = attribute(item, 'val');
      return value !== 'autoZero' && !(value === 'max' && axisPosition === 't');
    })
  )
    return false;
  const auto = directChildren(axis, 'auto');
  if (auto.length > 1 || auto.some((item) => !booleanAttributeIsTrue(item))) return false;
  const alignments = directChildren(axis, 'lblAlgn');
  if (alignments.length > 1 || alignments.some((item) => attribute(item, 'val') !== 'ctr')) return false;
  const offsets = directChildren(axis, 'lblOffset');
  if (offsets.length > 1 || offsets.some((item) => attribute(item, 'val') !== '100')) return false;
  const deletion = directChildren(axis, 'delete');
  if (deletion.length > 1 || !deletion.every((item) => validBooleanAttributeValue(item))) return false;
  return !deletion.some(booleanAttributeIsTrue) || axisPosition === 't';
}

function validBooleanAttributeValue(element: Element): boolean {
  const value = attribute(element, 'val');
  return value === null || ['0', '1', 'false', 'true'].includes(value);
}

function axisSettingNumber(parent: Element, name: string): { valid: boolean; value?: number } {
  const elements = directChildren(parent, name);
  if (!elements.length) return { valid: true };
  if (elements.length > 1) return { valid: false };
  const source = attribute(elements[0], 'val');
  if (!source?.trim()) return { valid: false };
  const value = Number(source);
  return Number.isFinite(value) ? { valid: true, value } : { valid: false };
}

function isSupportedAxisTitle(title: Element): boolean {
  const children = directChildren(title);
  if (children.some((child) => !['tx', 'layout', 'overlay'].includes(child.localName))) return false;
  const text = directChildren(title, 'tx');
  if (text.length !== 1) return false;
  const layouts = directChildren(title, 'layout');
  if (layouts.length > 1 || layouts.some((layout) => descendants(layout, 'manualLayout').length > 0)) return false;
  const overlays = directChildren(title, 'overlay');
  if (overlays.length > 1 || overlays.some((overlay) => booleanAttributeIsTrue(overlay))) return false;

  const sources = directChildren(text[0]);
  if (sources.length !== 1) return false;
  const source = sources[0];
  if (source.localName === 'strRef') {
    const sourceChildren = directChildren(source);
    if (sourceChildren.some((child) => child.localName !== 'f' && child.localName !== 'strCache')) return false;
    const formulas = directChildren(source, 'f');
    return (
      formulas.length === 1 &&
      directChildren(source, 'strCache').length <= 1 &&
      isSupportedChartReference(formulas[0].textContent ?? '')
    );
  }
  if (source.localName !== 'rich') return false;
  if (descendants(source, 'p').length !== 1 || !descendants(source, 't').length) return false;
  return !['br', 'fld', 'hlinkClick', 'hlinkMouseOver'].some((name) => descendants(source, name).length > 0);
}

function booleanAttributeIsTrue(element: Element): boolean {
  const value = attribute(element, 'val');
  return value === null || value === '1' || value === 'true';
}

function issue(
  code: string,
  message: string,
  severity: WorkCompatibilityIssue['severity'] = 'warning'
): WorkCompatibilityIssue {
  return {
    code,
    severity,
    feature: 'Workbook charts',
    message,
  };
}
