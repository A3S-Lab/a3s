import type { Image, Sheet } from '@fortune-sheet/core';
import type { WorkSpreadsheetChartLayout } from './work-spreadsheet-chart-layout';

export type WorkArtifactKind = 'document' | 'spreadsheet' | 'presentation' | 'pdf';
export type WorkLibraryView = 'home' | 'recent' | 'favorites' | 'folder' | 'trash';
export type WorkSaveState = 'saved' | 'dirty' | 'saving' | 'error';
export type WorkStorageMode = 'server' | 'local';

export interface WorkDocumentMargins {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export interface WorkDocumentColumnDefinition {
  widthPercent: number;
  spacing: number;
}

export interface WorkDocumentColumns {
  count: number;
  spacing: number;
  separator: boolean;
  custom?: WorkDocumentColumnDefinition[];
}

export type WorkDocumentPageChromeVariant = 'default' | 'first' | 'even';

export interface WorkDocumentPageChromeContent {
  headerHtml: string;
  footerHtml: string;
  showPageNumber: boolean;
}

export interface WorkDocumentPageChrome {
  differentFirstPage: boolean;
  differentOddEvenPages: boolean;
  default: WorkDocumentPageChromeContent;
  first: WorkDocumentPageChromeContent;
  even: WorkDocumentPageChromeContent;
}

export type WorkDocumentSectionBreakType = 'nextPage' | 'continuous' | 'evenPage' | 'oddPage' | 'nextColumn';

export interface WorkDocumentSectionLayout {
  pageSize: 'a4' | 'letter';
  orientation: 'portrait' | 'landscape';
  margins: WorkDocumentMargins;
  columns: WorkDocumentColumns;
  breakAfter: WorkDocumentSectionBreakType;
  headerText?: string;
  footerText?: string;
  showPageNumbers?: boolean;
  pageNumberStart?: number;
  pageChrome?: WorkDocumentPageChrome;
}

export interface WorkDocumentContent {
  type: 'document';
  html: string;
  pageSize: 'a4' | 'letter';
  orientation?: 'portrait' | 'landscape';
  margins?: WorkDocumentMargins;
  columns?: WorkDocumentColumns;
  headerText?: string;
  footerText?: string;
  showPageNumbers?: boolean;
  pageNumberStart?: number;
  pageChrome?: WorkDocumentPageChrome;
  trackChanges?: boolean;
  comments?: WorkDocumentComment[];
  bibliography?: WorkDocumentBibliography;
}

export type WorkDocumentCitationStyle = 'apa' | 'mla' | 'chicago' | 'ieee';

export interface WorkDocumentCitationPerson {
  first: string;
  middle?: string;
  last: string;
  suffix?: string;
}

export interface WorkDocumentCitationContributor {
  people?: WorkDocumentCitationPerson[];
  corporate?: string;
}

export interface WorkDocumentCitationSource {
  id: string;
  tag: string;
  sourceType: string;
  guid?: string;
  title: string;
  year?: string;
  contributors?: Record<string, WorkDocumentCitationContributor>;
  publisher?: string;
  city?: string;
  journalName?: string;
  volume?: string;
  issue?: string;
  pages?: string;
  url?: string;
  standardNumber?: string;
  conferenceName?: string;
  institution?: string;
  additionalFields?: Record<string, string>;
}

export interface WorkDocumentBibliography {
  style: WorkDocumentCitationStyle;
  styleName?: string;
  selectedStyle?: string;
  sources: WorkDocumentCitationSource[];
}

export interface WorkDocumentCommentReply {
  id: string;
  author: string;
  date: string;
  text: string;
}

export interface WorkDocumentComment {
  id: string;
  author: string;
  date: string;
  text: string;
  resolved: boolean;
  replies?: WorkDocumentCommentReply[];
}

export interface WorkSpreadsheetContent {
  type: 'spreadsheet';
  sheets: WorkSpreadsheetSheet[];
  calculation?: WorkSpreadsheetCalculationSettings;
  namedRanges?: WorkSpreadsheetNamedRange[];
  printAreas?: WorkSpreadsheetPrintArea[];
  printTitles?: WorkSpreadsheetPrintTitles[];
  pageBreaks?: WorkSpreadsheetPageBreaks[];
  pageSetups?: WorkSpreadsheetPageSetup[];
}

export interface WorkSpreadsheetImage extends Image {
  name?: string;
  altText?: string;
  contentType?: string;
}

export type WorkSpreadsheetChartType =
  | 'bar'
  | 'column'
  | 'line'
  | 'pie'
  | 'doughnut'
  | 'area'
  | 'radar'
  | 'scatter'
  | 'bubble'
  | 'combination';
export type WorkSpreadsheetRadarStyle = 'standard' | 'marker' | 'filled';
export type WorkSpreadsheetScatterStyle = 'marker' | 'line' | 'lineMarker' | 'smooth' | 'smoothMarker';
export type WorkSpreadsheetBubbleSizeRepresents = 'area' | 'width';
export type WorkSpreadsheetCombinationSeriesType = 'column' | 'line' | 'area';
export type WorkSpreadsheetChartAxisGroup = 'primary' | 'secondary';
export type WorkSpreadsheetChartAxisPosition = 'bottom' | 'left' | 'top' | 'right';
export type WorkSpreadsheetChartLineDash = 'solid' | 'dash' | 'dot' | 'dashDot';
export type WorkSpreadsheetChartMarkerSymbol =
  | 'none'
  | 'circle'
  | 'square'
  | 'diamond'
  | 'triangle'
  | 'plus'
  | 'x'
  | 'star';

export interface WorkSpreadsheetChartMarkerStyle {
  symbol?: WorkSpreadsheetChartMarkerSymbol;
  size?: number;
  fillColor?: string;
  lineColor?: string;
}

export interface WorkSpreadsheetChartSeriesStyle {
  fillColor?: string;
  fillTransparency?: number;
  lineColor?: string;
  lineWidth?: number;
  lineDash?: WorkSpreadsheetChartLineDash;
  marker?: WorkSpreadsheetChartMarkerStyle;
}

export interface WorkSpreadsheetChartAxis {
  title?: string;
  titleReference?: string;
  reverseOrder?: boolean;
  labelPosition?: 'nextTo' | 'high' | 'low' | 'none';
  majorTickMark?: 'none' | 'inside' | 'outside' | 'cross';
  labelInterval?: number;
  minimum?: number;
  maximum?: number;
  majorUnit?: number;
  showMajorGridlines?: boolean;
  numberFormat?: string;
  numberFormatSourceLinked?: boolean;
}

export interface WorkSpreadsheetChartAxes {
  bottom?: WorkSpreadsheetChartAxis;
  left?: WorkSpreadsheetChartAxis;
  top?: WorkSpreadsheetChartAxis;
  right?: WorkSpreadsheetChartAxis;
}

export type WorkSpreadsheetDataLabelPosition =
  | 'bestFit'
  | 'center'
  | 'insideBase'
  | 'insideEnd'
  | 'outsideEnd'
  | 'left'
  | 'right'
  | 'above'
  | 'below';

export interface WorkSpreadsheetDataLabels {
  showValue?: boolean;
  showCategoryName?: boolean;
  showSeriesName?: boolean;
  showPercentage?: boolean;
  showBubbleSize?: boolean;
  separator?: string;
  position?: WorkSpreadsheetDataLabelPosition;
}

export type WorkSpreadsheetErrorBarDirection = 'x' | 'y';
export type WorkSpreadsheetErrorBarType = 'both' | 'plus' | 'minus';
export type WorkSpreadsheetErrorBarValueType =
  | 'fixedValue'
  | 'percentage'
  | 'standardDeviation'
  | 'standardError'
  | 'custom';

export interface WorkSpreadsheetErrorBars {
  direction: WorkSpreadsheetErrorBarDirection;
  barType: WorkSpreadsheetErrorBarType;
  valueType: WorkSpreadsheetErrorBarValueType;
  value?: number;
  showEndCaps?: boolean;
  plusValues?: number[];
  plusReference?: string;
  minusValues?: number[];
  minusReference?: string;
}

export type WorkSpreadsheetTrendlineType =
  | 'linear'
  | 'exponential'
  | 'logarithmic'
  | 'polynomial'
  | 'power'
  | 'movingAverage';

export interface WorkSpreadsheetTrendline {
  type: WorkSpreadsheetTrendlineType;
  name?: string;
  order?: number;
  period?: number;
  forward?: number;
  backward?: number;
  intercept?: number;
  displayEquation?: boolean;
  displayRSquared?: boolean;
}

export function normalizeWorkSpreadsheetDoughnutHoleSize(value: unknown): number {
  const size = Number(value);
  if (!Number.isFinite(size)) return 50;
  return Math.min(90, Math.max(10, Math.round(size)));
}

export function normalizeWorkSpreadsheetRadarStyle(value: unknown): WorkSpreadsheetRadarStyle {
  return value === 'standard' || value === 'filled' || value === 'marker' ? value : 'marker';
}

export function normalizeWorkSpreadsheetScatterStyle(value: unknown): WorkSpreadsheetScatterStyle {
  return value === 'line' ||
    value === 'lineMarker' ||
    value === 'smooth' ||
    value === 'smoothMarker' ||
    value === 'marker'
    ? value
    : 'marker';
}

export function normalizeWorkSpreadsheetBubbleScale(value: unknown): number {
  const scale = Number(value);
  if (!Number.isFinite(scale)) return 100;
  return Math.min(300, Math.max(0, Math.round(scale)));
}

export function normalizeWorkSpreadsheetBubbleSizeRepresents(value: unknown): WorkSpreadsheetBubbleSizeRepresents {
  return value === 'width' || value === 'w' ? 'width' : 'area';
}

export function workSpreadsheetChartUsesNumericXAxis(type: WorkSpreadsheetChartType): boolean {
  return type === 'scatter' || type === 'bubble';
}

export function workSpreadsheetChartSupportsAxes(type: WorkSpreadsheetChartType): boolean {
  return type !== 'pie' && type !== 'doughnut';
}

export function normalizeWorkSpreadsheetCombinationSeriesType(value: unknown): WorkSpreadsheetCombinationSeriesType {
  return value === 'line' || value === 'area' || value === 'column' ? value : 'column';
}

export function normalizeWorkSpreadsheetChartAxisGroup(value: unknown): WorkSpreadsheetChartAxisGroup {
  return value === 'secondary' ? 'secondary' : 'primary';
}

export function workSpreadsheetCombinationSeriesTypeLabel(type: WorkSpreadsheetCombinationSeriesType): string {
  if (type === 'line') return '折线图';
  if (type === 'area') return '面积图';
  return '柱形图';
}

export function workSpreadsheetChartSupportsTrendlines(type: WorkSpreadsheetChartType): boolean {
  return type !== 'pie' && type !== 'doughnut' && type !== 'radar';
}

export function workSpreadsheetChartSupportsErrorBars(type: WorkSpreadsheetChartType): boolean {
  return type !== 'pie' && type !== 'doughnut' && type !== 'radar';
}

export function normalizeWorkSpreadsheetErrorBars(
  source: WorkSpreadsheetErrorBars,
  chartType: WorkSpreadsheetChartType
): WorkSpreadsheetErrorBars {
  const direction = workSpreadsheetChartUsesNumericXAxis(chartType) && source.direction === 'x' ? 'x' : 'y';
  const barType = source.barType === 'plus' || source.barType === 'minus' ? source.barType : 'both';
  const valueType =
    source.valueType === 'percentage' ||
    source.valueType === 'standardDeviation' ||
    source.valueType === 'standardError' ||
    source.valueType === 'custom' ||
    source.valueType === 'fixedValue'
      ? source.valueType
      : 'fixedValue';
  const fallbackValue = valueType === 'percentage' ? 5 : 1;
  const numericValue = Number(source.value);
  const value = Number.isFinite(numericValue) ? Math.max(0, numericValue) : fallbackValue;
  const plusValues = source.plusValues?.map(normalizedErrorBarAmount);
  const minusValues = source.minusValues?.map(normalizedErrorBarAmount);
  return {
    direction,
    barType,
    valueType,
    ...(valueType === 'fixedValue' || valueType === 'percentage' || valueType === 'standardDeviation' ? { value } : {}),
    ...(source.showEndCaps === false ? { showEndCaps: false } : {}),
    ...(valueType === 'custom' && barType !== 'minus' && plusValues?.length ? { plusValues } : {}),
    ...(valueType === 'custom' && barType !== 'minus' && source.plusReference?.trim()
      ? { plusReference: source.plusReference.trim().replace(/^=/, '') }
      : {}),
    ...(valueType === 'custom' && barType !== 'plus' && minusValues?.length ? { minusValues } : {}),
    ...(valueType === 'custom' && barType !== 'plus' && source.minusReference?.trim()
      ? { minusReference: source.minusReference.trim().replace(/^=/, '') }
      : {}),
  };
}

export function workSpreadsheetErrorBarValueTypeLabel(type: WorkSpreadsheetErrorBarValueType): string {
  if (type === 'percentage') return '百分比';
  if (type === 'standardDeviation') return '标准差';
  if (type === 'standardError') return '标准误差';
  if (type === 'custom') return '自定义';
  return '固定值';
}

export function workSpreadsheetErrorBarTypeLabel(type: WorkSpreadsheetErrorBarType): string {
  if (type === 'plus') return '正向';
  if (type === 'minus') return '负向';
  return '双向';
}

function normalizedErrorBarAmount(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}

export function normalizeWorkSpreadsheetDataLabelPosition(value: unknown): WorkSpreadsheetDataLabelPosition {
  return value === 'center' ||
    value === 'insideBase' ||
    value === 'insideEnd' ||
    value === 'outsideEnd' ||
    value === 'left' ||
    value === 'right' ||
    value === 'above' ||
    value === 'below' ||
    value === 'bestFit'
    ? value
    : 'bestFit';
}

export function normalizeWorkSpreadsheetDataLabels(
  source: WorkSpreadsheetDataLabels,
  chartType: WorkSpreadsheetChartType
): WorkSpreadsheetDataLabels {
  const position = source.position ? normalizeWorkSpreadsheetDataLabelPosition(source.position) : undefined;
  const separator = typeof source.separator === 'string' ? source.separator.slice(0, 64) : undefined;
  return {
    ...(source.showValue === true ? { showValue: true } : {}),
    ...(source.showCategoryName === true ? { showCategoryName: true } : {}),
    ...(source.showSeriesName === true ? { showSeriesName: true } : {}),
    ...(source.showPercentage === true && (chartType === 'pie' || chartType === 'doughnut')
      ? { showPercentage: true }
      : {}),
    ...(source.showBubbleSize === true && chartType === 'bubble' ? { showBubbleSize: true } : {}),
    ...(separator !== undefined ? { separator } : {}),
    ...(position ? { position } : {}),
  };
}

export function workSpreadsheetDataLabelPositionLabel(position: WorkSpreadsheetDataLabelPosition): string {
  if (position === 'center') return '居中';
  if (position === 'insideBase') return '内侧基部';
  if (position === 'insideEnd') return '内侧末端';
  if (position === 'outsideEnd') return '外侧末端';
  if (position === 'left') return '左侧';
  if (position === 'right') return '右侧';
  if (position === 'above') return '上方';
  if (position === 'below') return '下方';
  return '最佳匹配';
}

export function normalizeWorkSpreadsheetTrendlineType(value: unknown): WorkSpreadsheetTrendlineType {
  return value === 'exponential' ||
    value === 'logarithmic' ||
    value === 'polynomial' ||
    value === 'power' ||
    value === 'movingAverage' ||
    value === 'linear'
    ? value
    : 'linear';
}

export function normalizeWorkSpreadsheetTrendline(trendline: WorkSpreadsheetTrendline): WorkSpreadsheetTrendline {
  const type = normalizeWorkSpreadsheetTrendlineType(trendline.type);
  const name = trendline.name?.trim().slice(0, 255);
  const order = normalizedInteger(trendline.order, 2, 6, 2);
  const period = normalizedInteger(trendline.period, 2, 255, 2);
  const forward = normalizedNonNegativeNumber(trendline.forward);
  const backward = normalizedNonNegativeNumber(trendline.backward);
  const intercept = Number.isFinite(trendline.intercept) ? Number(trendline.intercept) : undefined;
  return {
    type,
    ...(name ? { name } : {}),
    ...(type === 'polynomial' ? { order } : {}),
    ...(type === 'movingAverage' ? { period } : {}),
    ...(forward > 0 ? { forward } : {}),
    ...(backward > 0 ? { backward } : {}),
    ...(intercept !== undefined ? { intercept } : {}),
    ...(trendline.displayEquation === true ? { displayEquation: true } : {}),
    ...(trendline.displayRSquared === true ? { displayRSquared: true } : {}),
  };
}

export function workSpreadsheetTrendlineTypeLabel(type: WorkSpreadsheetTrendlineType): string {
  if (type === 'exponential') return '指数';
  if (type === 'logarithmic') return '对数';
  if (type === 'polynomial') return '多项式';
  if (type === 'power') return '幂';
  if (type === 'movingAverage') return '移动平均';
  return '线性';
}

function normalizedInteger(value: unknown, minimum: number, maximum: number, fallback: number): number {
  const number = Number(value);
  if (!Number.isFinite(number)) return fallback;
  return Math.min(maximum, Math.max(minimum, Math.round(number)));
}

function normalizedNonNegativeNumber(value: unknown): number {
  const number = Number(value);
  return Number.isFinite(number) ? Math.max(0, number) : 0;
}

export function workSpreadsheetChartTypeLabel(type: WorkSpreadsheetChartType): string {
  if (type === 'bar') return '条形图';
  if (type === 'line') return '折线图';
  if (type === 'pie') return '饼图';
  if (type === 'doughnut') return '圆环图';
  if (type === 'area') return '面积图';
  if (type === 'radar') return '雷达图';
  if (type === 'scatter') return '散点图';
  if (type === 'bubble') return '气泡图';
  if (type === 'combination') return '组合图';
  return '柱形图';
}

export interface WorkSpreadsheetChartSeries {
  name: string;
  values: number[];
  nameReference?: string;
  valuesReference?: string;
  xValues?: number[];
  xValuesReference?: string;
  bubbleSizes?: number[];
  bubbleSizesReference?: string;
  chartType?: WorkSpreadsheetCombinationSeriesType;
  axisGroup?: WorkSpreadsheetChartAxisGroup;
  dataLabels?: WorkSpreadsheetDataLabels;
  errorBars?: WorkSpreadsheetErrorBars[];
  trendlines?: WorkSpreadsheetTrendline[];
  style?: WorkSpreadsheetChartSeriesStyle;
}

export interface WorkSpreadsheetChart extends WorkSpreadsheetChartLayout {
  id: string;
  name: string;
  altText?: string;
  type: WorkSpreadsheetChartType;
  title?: string;
  titleReference?: string;
  axes?: WorkSpreadsheetChartAxes;
  categories: string[];
  categoryReference?: string;
  series: WorkSpreadsheetChartSeries[];
  showLegend: boolean;
  doughnutHoleSize?: number;
  radarStyle?: WorkSpreadsheetRadarStyle;
  scatterStyle?: WorkSpreadsheetScatterStyle;
  bubbleScale?: number;
  showNegativeBubbles?: boolean;
  bubbleSizeRepresents?: WorkSpreadsheetBubbleSizeRepresents;
  left: number;
  top: number;
  width: number;
  height: number;
}

export type WorkSpreadsheetSheet = Omit<Sheet, 'images'> & {
  images?: WorkSpreadsheetImage[];
  charts?: WorkSpreadsheetChart[];
  pivotTables?: WorkSpreadsheetPivotTable[];
  formulaMetadata?: WorkSpreadsheetFormulaMetadata;
};

export type WorkSpreadsheetPivotAggregation =
  | 'sum'
  | 'count'
  | 'counta'
  | 'average'
  | 'max'
  | 'min'
  | 'product'
  | 'stdDev'
  | 'stdDevP'
  | 'var'
  | 'varP';

export interface WorkSpreadsheetPivotValue {
  fieldIndex: number;
  aggregation: WorkSpreadsheetPivotAggregation;
  caption?: string;
}

export type WorkSpreadsheetPivotFilterValue = string | number | boolean | null;

export interface WorkSpreadsheetPivotReportFilter {
  fieldIndex: number;
  selectedItem?: WorkSpreadsheetPivotFilterValue;
}

export interface WorkSpreadsheetPivotTable {
  id: string;
  name: string;
  sourceSheetId: string;
  sourceReference: string;
  anchor: string;
  rowFields: number[];
  columnFields: number[];
  reportFilters?: WorkSpreadsheetPivotReportFilter[];
  values: WorkSpreadsheetPivotValue[];
  rowGrandTotals: boolean;
  columnGrandTotals: boolean;
  styleName: string;
  refreshOnLoad: boolean;
  outputReference?: string;
}

export type WorkSpreadsheetCalculationMode = 'automatic' | 'automatic-except-data-tables' | 'manual';

export interface WorkSpreadsheetCalculationSettings {
  mode: WorkSpreadsheetCalculationMode;
  fullCalculationOnLoad: boolean;
  forceFullCalculation: boolean;
  iterativeCalculation: boolean;
  maximumIterations: number;
  maximumChange: number;
  fullPrecision: boolean;
}

export type WorkSpreadsheetFormulaRangeType = 'array' | 'dynamic-array' | 'data-table';

export interface WorkSpreadsheetDataTableOptions {
  input1Reference?: string;
  input2Reference?: string;
  twoDimensional?: boolean;
  rowOriented?: boolean;
  input1Deleted?: boolean;
  input2Deleted?: boolean;
  calculateOnLoad?: boolean;
}

export interface WorkSpreadsheetFormulaRange {
  type: WorkSpreadsheetFormulaRangeType;
  anchor: string;
  reference: string;
  formula?: string;
  dataTable?: WorkSpreadsheetDataTableOptions;
}

export interface WorkSpreadsheetFormulaMetadata {
  ranges?: WorkSpreadsheetFormulaRange[];
  sourceFormulas?: Record<string, string>;
  normalizedSharedFormulaGroups?: number;
  normalizedSharedFormulaCells?: number;
}

export interface WorkSpreadsheetNamedRange {
  id: string;
  name: string;
  reference: string;
  scopeSheetId?: string;
  comment?: string;
}

export interface WorkSpreadsheetPrintArea {
  sheetId: string;
  reference: string;
}

export interface WorkSpreadsheetPrintTitles {
  sheetId: string;
  rows?: string;
  columns?: string;
}

export interface WorkSpreadsheetPageBreaks {
  sheetId: string;
  rows?: number[];
  columns?: number[];
}

export interface WorkSpreadsheetPageMargins {
  top: number;
  right: number;
  bottom: number;
  left: number;
  header: number;
  footer: number;
}

export interface WorkSpreadsheetHeaderFooterSections {
  left?: string;
  center?: string;
  right?: string;
}

export type WorkSpreadsheetPaperSize = 'a3' | 'a4' | 'a5' | 'letter' | 'legal' | 'tabloid';

export interface WorkSpreadsheetPageSetup {
  sheetId: string;
  paperSize?: WorkSpreadsheetPaperSize;
  orientation?: 'portrait' | 'landscape';
  scale?: number;
  fitToPage?: boolean;
  fitToWidth?: number;
  fitToHeight?: number;
  horizontalCentered?: boolean;
  verticalCentered?: boolean;
  header?: WorkSpreadsheetHeaderFooterSections;
  footer?: WorkSpreadsheetHeaderFooterSections;
  pageNumberStart?: number;
  pageOrder?: 'downThenOver' | 'overThenDown';
  scaleWithDocument?: boolean;
  alignWithMargins?: boolean;
  margins?: WorkSpreadsheetPageMargins;
}

export type WorkCompatibilitySeverity = 'info' | 'warning' | 'error';

export interface WorkCompatibilityIssue {
  code: string;
  severity: WorkCompatibilitySeverity;
  feature: string;
  message: string;
  location?: string;
}

export interface WorkCompatibilityReport {
  sourceFormat: string;
  sourceName: string;
  assessedAt: number;
  issues: WorkCompatibilityIssue[];
}

export type WorkSlideElementType = 'text' | 'shape' | 'image' | 'table' | 'chart' | 'line';
export type WorkSlideTextAlign = 'left' | 'center' | 'right';
export type WorkSlideVerticalAlign = 'top' | 'middle' | 'bottom';
export type WorkSlideShapeType = 'rect' | 'roundRect' | 'ellipse' | 'triangle' | 'diamond' | 'line';

export interface WorkSlideTextRun {
  text: string;
  fontSize?: number;
  color?: string;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  fontFamily?: string;
  href?: string;
}

export interface WorkSlideImage {
  dataUrl: string;
  contentType: string;
  name: string;
}

export interface WorkSlideTable {
  rows: string[][];
  headerRows?: number;
}

export type WorkSlideChartErrorBars = WorkSpreadsheetErrorBars;
export type WorkSlideChartTrendline = WorkSpreadsheetTrendline;
export type WorkSlideChartSeriesStyle = WorkSpreadsheetChartSeriesStyle;

export interface WorkSlideChartSeries {
  name: string;
  values: number[];
  bubbleSizes?: number[];
  errorBars?: WorkSlideChartErrorBars[];
  trendlines?: WorkSlideChartTrendline[];
  style?: WorkSlideChartSeriesStyle;
}

export type WorkSlideChartType =
  | 'bar'
  | 'column'
  | 'line'
  | 'pie'
  | 'doughnut'
  | 'area'
  | 'radar'
  | 'scatter'
  | 'bubble';
export type WorkSlideRadarStyle = 'standard' | 'marker' | 'filled';
export type WorkSlideScatterStyle = 'marker' | 'line' | 'lineMarker' | 'smooth' | 'smoothMarker';
export type WorkSlideBubbleSizeRepresents = 'area' | 'width';
export type WorkSlideChartAxis = WorkSpreadsheetChartAxis;
export type WorkSlideChartAxes = WorkSpreadsheetChartAxes;
export type WorkSlideChartLegendPosition = 'right' | 'left' | 'top' | 'bottom' | 'topRight';
export type WorkSlideChartDataLabelPosition =
  | 'bestFit'
  | 'center'
  | 'insideBase'
  | 'insideEnd'
  | 'outsideEnd'
  | 'left'
  | 'right'
  | 'above'
  | 'below';

export interface WorkSlideChartDataLabels {
  showValue?: boolean;
  showCategoryName?: boolean;
  showSeriesName?: boolean;
  showPercentage?: boolean;
  showBubbleSize?: boolean;
  separator?: string;
  position?: WorkSlideChartDataLabelPosition;
}

export interface WorkSlideChart extends WorkSpreadsheetChartLayout {
  type: WorkSlideChartType;
  title?: string;
  categories: string[];
  series: WorkSlideChartSeries[];
  showLegend?: boolean;
  legendPosition?: WorkSlideChartLegendPosition;
  axes?: WorkSlideChartAxes;
  // Retained for persisted artifacts created before editable axis settings.
  categoryAxisTitle?: string;
  valueAxisTitle?: string;
  dataLabels?: WorkSlideChartDataLabels;
  doughnutHoleSize?: number;
  radarStyle?: WorkSlideRadarStyle;
  scatterStyle?: WorkSlideScatterStyle;
  bubbleScale?: number;
  showNegativeBubbles?: boolean;
  bubbleSizeRepresents?: WorkSlideBubbleSizeRepresents;
}

export interface WorkSlidePlaceholder {
  key: string;
  type: string;
  prompt?: string;
  inheritsGeometry?: boolean;
  inheritsStyle?: boolean;
}

export type WorkSlideTransitionType = 'fade' | 'push' | 'wipe' | 'split' | 'cut';
export type WorkSlideTransitionSpeed = 'fast' | 'medium' | 'slow';
export type WorkSlideTransitionDirection = 'left' | 'right' | 'up' | 'down' | 'in' | 'out';

export interface WorkSlideTransition {
  type: WorkSlideTransitionType;
  speed: WorkSlideTransitionSpeed;
  direction?: WorkSlideTransitionDirection;
  orientation?: 'horizontal' | 'vertical';
  advanceOnClick: boolean;
  advanceAfterMs?: number;
}

export interface WorkSlideElement {
  id: string;
  type: WorkSlideElementType;
  x: number;
  y: number;
  width: number;
  height: number;
  text: string;
  fontSize: number;
  color: string;
  fill: string;
  bold: boolean;
  align: WorkSlideTextAlign;
  radius?: number;
  shapeType?: WorkSlideShapeType;
  rotation?: number;
  opacity?: number;
  borderColor?: string;
  borderWidth?: number;
  fontFamily?: string;
  italic?: boolean;
  underline?: boolean;
  verticalAlign?: WorkSlideVerticalAlign;
  textRuns?: WorkSlideTextRun[];
  image?: WorkSlideImage;
  table?: WorkSlideTable;
  chart?: WorkSlideChart;
  href?: string;
  altText?: string;
  placeholder?: WorkSlidePlaceholder;
}

export interface WorkSlide {
  id: string;
  name: string;
  background: string;
  layoutId?: string;
  useLayoutBackground?: boolean;
  showMasterElements?: boolean;
  elements: WorkSlideElement[];
  notes?: string;
  comments?: WorkSlideComment[];
  transition?: WorkSlideTransition;
}

export interface WorkSlideComment {
  id: string;
  author: string;
  initials?: string;
  date: string;
  text: string;
  x: number;
  y: number;
}

export interface WorkPresentationMaster {
  id: string;
  name: string;
  background: string;
  elements: WorkSlideElement[];
}

export interface WorkPresentationLayout {
  id: string;
  name: string;
  masterId: string;
  background?: string;
  elements: WorkSlideElement[];
  showMasterElements?: boolean;
  sourceType?: string;
}

export interface WorkPresentationContent {
  type: 'presentation';
  slides: WorkSlide[];
  width?: number;
  height?: number;
  masters?: WorkPresentationMaster[];
  layouts?: WorkPresentationLayout[];
}

export type WorkPresentationPrintLayout = 'slides' | 'notes' | 'handout-2' | 'handout-3' | 'handout-6';

export interface WorkPdfContent {
  type: 'pdf';
  pageCount?: number;
}

export type WorkArtifactContent =
  | WorkDocumentContent
  | WorkSpreadsheetContent
  | WorkPresentationContent
  | WorkPdfContent;

export interface WorkArtifact {
  id: string;
  kind: WorkArtifactKind;
  title: string;
  favorite: boolean;
  createdAt: number;
  updatedAt: number;
  lastOpenedAt: number;
  revision: number;
  content: WorkArtifactContent;
  folderId?: string | null;
  trashedAt?: number | null;
  source?: WorkSourceFile | null;
  compatibility?: WorkCompatibilityReport | null;
}

export interface WorkSourceFile {
  name: string;
  contentType: string;
  size: number;
  updatedAt: number;
}

export interface WorkFolder {
  id: string;
  name: string;
  parentId?: string | null;
  createdAt: number;
  updatedAt: number;
  revision: number;
  trashedAt?: number | null;
}

export interface WorkStorageLimits {
  artifactBytes: number;
  sourceBytes: number;
  historyEntries: number;
}

export interface WorkLibrarySnapshot {
  artifacts: WorkArtifact[];
  folders: WorkFolder[];
  limits: WorkStorageLimits | null;
  storage: WorkStorageMode;
}

export interface WorkArtifactVersion {
  revision: number;
  updatedAt: number;
  current: boolean;
  artifact: WorkArtifact;
}

export interface WorkTemplate {
  id: string;
  kind: WorkArtifactKind;
  name: string;
  description: string;
  accent: string;
}

export function workArtifactExtension(kind: WorkArtifactKind): string {
  if (kind === 'document') return 'docx';
  if (kind === 'spreadsheet') return 'xlsx';
  if (kind === 'presentation') return 'pptx';
  return 'pdf';
}

export function workArtifactKindLabel(kind: WorkArtifactKind): string {
  if (kind === 'document') return '文字';
  if (kind === 'spreadsheet') return '表格';
  if (kind === 'presentation') return '演示';
  return 'PDF';
}
