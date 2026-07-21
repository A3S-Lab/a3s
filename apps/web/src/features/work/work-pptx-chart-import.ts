import { attribute, descendants, directChild, directChildren, firstDescendant } from './work-ooxml-package';
import { readPptxChartAxisDiagnostics } from './work-pptx-chart-axis-diagnostics';
import { readPptxChartDataLabels } from './work-pptx-chart-data-labels';
import { readPptxChartSeriesAnalysis } from './work-pptx-chart-series-analysis';
import {
  normalizeDoughnutHoleSize,
  normalizePresentationBubbleScale,
  normalizePresentationBubbleSizeRepresents,
  normalizePresentationChartLegendPosition,
  normalizePresentationScatterStyle,
  normalizeRadarStyle,
  presentationChartUsesNumericXAxis,
} from './work-presentation-charts';
import type { WorkSlideChart, WorkSlideChartLegendPosition } from './work-types';
import { parseXlsxChartAxes } from './work-xlsx-chart-axes';

export interface PptxChartImportDiagnostic {
  code: string;
  feature: string;
  message: string;
}

export interface PptxChartImportResult {
  chart: WorkSlideChart;
  diagnostics: PptxChartImportDiagnostic[];
}

export function readPptxChart(document: Document): PptxChartImportResult {
  const diagnostics: PptxChartImportDiagnostic[] = [];
  const chartTypes = [
    'barChart',
    'lineChart',
    'pieChart',
    'doughnutChart',
    'areaChart',
    'radarChart',
    'scatterChart',
    'bubbleChart',
  ];
  const chartNode = chartTypes.map((name) => firstDescendant(document, name)).find(Boolean);
  if (!chartNode) {
    diagnostics.push({
      code: 'pptx.chart.type',
      feature: 'Charts',
      message: 'This chart type is shown as a basic column chart and will be normalized on export.',
    });
  }
  const localName = chartNode?.localName;
  const barDirection = attribute(firstDescendant(chartNode, 'barDir') ?? document.documentElement, 'val');
  const type: WorkSlideChart['type'] =
    localName === 'scatterChart'
      ? 'scatter'
      : localName === 'bubbleChart'
        ? 'bubble'
        : localName === 'lineChart'
          ? 'line'
          : localName === 'pieChart'
            ? 'pie'
            : localName === 'doughnutChart'
              ? 'doughnut'
              : localName === 'radarChart'
                ? 'radar'
                : localName === 'areaChart'
                  ? 'area'
                  : barDirection === 'bar'
                    ? 'bar'
                    : 'column';
  const numericXAxis = presentationChartUsesNumericXAxis(type);
  const seriesNodes = chartNode ? directChildren(chartNode, 'ser') : descendants(document, 'ser');
  const xValues = seriesNodes.map((node) => cachedValues(directChild(node, 'xVal')).map(finiteNumber));
  const seriesAnalysis = seriesNodes.map((node) => readPptxChartSeriesAnalysis(node, type));
  const series = seriesNodes.map((node, index) => ({
    name: cachedValues(directChild(node, 'tx'))[0] ?? `Series ${index + 1}`,
    values: cachedValues(directChild(node, numericXAxis ? 'yVal' : 'val')).map(finiteNumber),
    ...(type === 'bubble' ? { bubbleSizes: cachedValues(directChild(node, 'bubbleSize')).map(finiteNumber) } : {}),
    ...(seriesAnalysis[index]?.trendlines ? { trendlines: seriesAnalysis[index].trendlines } : {}),
    ...(seriesAnalysis[index]?.errorBars ? { errorBars: seriesAnalysis[index].errorBars } : {}),
  }));
  diagnostics.push(
    ...seriesAnalysis.flatMap((result) =>
      result.diagnostics.map((diagnostic) => ({ ...diagnostic, feature: 'Chart series analysis' }))
    )
  );
  const categories = numericXAxis
    ? (xValues[0] ?? []).map((value) => String(value))
    : cachedValues(directChild(seriesNodes[0] ?? document.documentElement, 'cat'));
  if (numericXAxis && new Set(xValues.map((values) => values.join('|'))).size > 1) {
    diagnostics.push({
      code: 'pptx.chart.xy-values',
      feature: 'XY chart data',
      message: 'Different per-series X values were normalized to the first series shared X-value list.',
    });
  }
  const chartRoot = firstDescendant(document, 'chart');
  const legend = directChild(chartRoot ?? document.documentElement, 'legend');
  const title = chartText(directChild(chartRoot ?? document.documentElement, 'title'));
  const axes = parseXlsxChartAxes(document, type, false, true);
  diagnostics.push(
    ...readPptxChartAxisDiagnostics(document).map((diagnostic) => ({
      ...diagnostic,
      feature: 'Chart axes',
    }))
  );
  diagnostics.push({
    code: 'pptx.chart.format',
    feature: 'Chart formatting',
    message: 'Chart data and its basic type are preserved; advanced chart styling is normalized on export.',
  });
  const chart: WorkSlideChart = {
    type,
    title,
    categories,
    series,
    showLegend: Boolean(legend),
    ...(legend ? { legendPosition: pptxLegendPosition(legend) } : {}),
    ...(axes ? { axes } : {}),
  };
  const dataLabelResult = readPptxChartDataLabels(chartNode, type);
  if (dataLabelResult.dataLabels) chart.dataLabels = dataLabelResult.dataLabels;
  diagnostics.push(
    ...dataLabelResult.diagnostics.map((diagnostic) => ({
      ...diagnostic,
      feature: 'Chart data labels',
    }))
  );
  if (type === 'doughnut') readDoughnutSettings(chartNode, document, chart, diagnostics);
  if (type === 'radar') readRadarSettings(chartNode, document, chart, diagnostics);
  if (type === 'scatter') readScatterSettings(chartNode, chart, diagnostics);
  if (type === 'bubble') readBubbleSettings(chartNode, chart, diagnostics);
  return { chart, diagnostics };
}

function pptxLegendPosition(legend: Element): WorkSlideChartLegendPosition {
  const value = attribute(directChild(legend, 'legendPos') ?? legend, 'val');
  const position =
    value === 'l' ? 'left' : value === 't' ? 'top' : value === 'b' ? 'bottom' : value === 'tr' ? 'topRight' : 'right';
  return normalizePresentationChartLegendPosition(position);
}

function chartText(node: Element | undefined): string | undefined {
  if (!node) return undefined;
  const richText = descendants(node, 't')
    .map((part) => part.textContent ?? '')
    .join('')
    .trim();
  if (richText) return richText;
  const cachedText = descendants(node, 'v')
    .map((part) => part.textContent ?? '')
    .join('')
    .trim();
  return cachedText || undefined;
}

function readDoughnutSettings(
  chartNode: Element | undefined,
  document: Document,
  chart: WorkSlideChart,
  diagnostics: PptxChartImportDiagnostic[]
): void {
  const holeSize = firstDescendant(chartNode ?? document.documentElement, 'holeSize');
  const holeSizeValue = attribute(holeSize ?? document.documentElement, 'val');
  const rawHoleSize = holeSizeValue === null ? undefined : Number(holeSizeValue);
  chart.doughnutHoleSize = normalizeDoughnutHoleSize(rawHoleSize);
  if (
    holeSize &&
    (rawHoleSize === undefined || !Number.isFinite(rawHoleSize) || rawHoleSize < 10 || rawHoleSize > 90)
  ) {
    diagnostics.push({
      code: 'pptx.chart.doughnut-hole',
      feature: 'Chart formatting',
      message: 'The doughnut hole size was outside the editable 10–90% range and was normalized.',
    });
  }
}

function readRadarSettings(
  chartNode: Element | undefined,
  document: Document,
  chart: WorkSlideChart,
  diagnostics: PptxChartImportDiagnostic[]
): void {
  const radarStyle = attribute(
    firstDescendant(chartNode ?? document.documentElement, 'radarStyle') ?? chartNode ?? document.documentElement,
    'val'
  );
  chart.radarStyle = normalizeRadarStyle(
    radarStyle === 'standard' || radarStyle === 'marker' || radarStyle === 'filled' ? radarStyle : undefined
  );
  if (radarStyle && !['standard', 'marker', 'filled'].includes(radarStyle)) {
    diagnostics.push({
      code: 'pptx.chart.radar-style',
      feature: 'Chart formatting',
      message: `The radar style “${radarStyle}” was normalized to the standard editable style.`,
    });
  }
}

function readScatterSettings(
  chartNode: Element | undefined,
  chart: WorkSlideChart,
  diagnostics: PptxChartImportDiagnostic[]
): void {
  if (!chartNode) return;
  const rawStyle = attribute(directChild(chartNode, 'scatterStyle') ?? chartNode, 'val');
  chart.scatterStyle = normalizePresentationScatterStyle(rawStyle);
  if (rawStyle && !['marker', 'line', 'lineMarker', 'smooth', 'smoothMarker'].includes(rawStyle)) {
    diagnostics.push({
      code: 'pptx.chart.scatter-style',
      feature: 'Chart formatting',
      message: `The scatter style “${rawStyle}” was normalized to the editable line-and-marker style.`,
    });
  }
}

function readBubbleSettings(
  chartNode: Element | undefined,
  chart: WorkSlideChart,
  diagnostics: PptxChartImportDiagnostic[]
): void {
  if (!chartNode) return;
  const source = chartNode;
  const scaleElement = directChild(source, 'bubbleScale');
  const rawScale = scaleElement ? Number(attribute(scaleElement, 'val')) : undefined;
  chart.bubbleScale = normalizePresentationBubbleScale(rawScale);
  if (scaleElement && (rawScale === undefined || !Number.isFinite(rawScale) || rawScale < 5 || rawScale > 300)) {
    diagnostics.push({
      code: 'pptx.chart.bubble-scale',
      feature: 'Chart formatting',
      message: 'The bubble scale was outside the editable 5–300% range and was normalized.',
    });
  }
  chart.showNegativeBubbles = pptxBooleanElement(source, 'showNegBubbles');
  const rawSizeRepresents = attribute(directChild(source, 'sizeRepresents') ?? source, 'val');
  chart.bubbleSizeRepresents = normalizePresentationBubbleSizeRepresents(
    rawSizeRepresents === 'w' ? 'width' : rawSizeRepresents
  );
  if (rawSizeRepresents && rawSizeRepresents !== 'area' && rawSizeRepresents !== 'w') {
    diagnostics.push({
      code: 'pptx.chart.bubble-size',
      feature: 'Chart formatting',
      message: `The bubble-size representation “${rawSizeRepresents}” was normalized to area.`,
    });
  }
  if (descendants(source, 'bubble3D').some((element) => pptxBooleanValue(element))) {
    diagnostics.push({
      code: 'pptx.chart.bubble-3d',
      feature: 'Chart formatting',
      message: '3D bubble effects were normalized to editable 2D bubbles.',
    });
  }
}

function cachedValues(parent: Element | undefined): string[] {
  if (!parent) return [];
  const multiLevelCache = firstDescendant(parent, 'multiLvlStrCache');
  if (multiLevelCache) {
    const levels = directChildren(multiLevelCache, 'lvl').map((level) => cachedPoints(level));
    const length = Math.max(0, ...levels.map((level) => level.length));
    return Array.from({ length }, (_, index) =>
      levels
        .map((level) => level[index]?.trim())
        .filter(Boolean)
        .join(' / ')
    );
  }
  const cache =
    firstDescendant(parent, 'strCache') ??
    firstDescendant(parent, 'numCache') ??
    firstDescendant(parent, 'strLit') ??
    firstDescendant(parent, 'numLit');
  if (!cache) {
    const value = firstDescendant(parent, 'v')?.textContent;
    return value === null || value === undefined ? [] : [value];
  }
  return cachedPoints(cache);
}

function cachedPoints(cache: Element): string[] {
  return descendants(cache, 'pt')
    .sort((left, right) => numberAttribute(left, 'idx', 0) - numberAttribute(right, 'idx', 0))
    .map((point) => firstDescendant(point, 'v')?.textContent ?? '');
}

function numberAttribute(element: Element | undefined, name: string, fallback: number): number {
  if (!element) return fallback;
  const value = Number(attribute(element, name));
  return Number.isFinite(value) ? value : fallback;
}

function finiteNumber(value: string): number {
  const number = Number(value);
  return Number.isFinite(number) ? number : 0;
}

function pptxBooleanElement(parent: Element, localName: string): boolean {
  const element = directChild(parent, localName);
  return element ? pptxBooleanValue(element) : false;
}

function pptxBooleanValue(element: Element): boolean {
  const value = attribute(element, 'val');
  return value === null || value === '1' || value.toLocaleLowerCase() === 'true';
}
