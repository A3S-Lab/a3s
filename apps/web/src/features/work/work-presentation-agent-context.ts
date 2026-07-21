import type { WorkAgentProposalTarget } from './work-agent-proposal';
import { chartSeriesErrorBarContext, chartSeriesTrendlineContext } from './work-chart-series-analysis-context';
import { presentationChartAxes } from './work-presentation-chart-axes';
import {
  normalizePresentationBubbleScale,
  normalizePresentationBubbleSizeRepresents,
  normalizePresentationChartDataLabels,
  normalizePresentationChartLegendPosition,
  normalizePresentationScatterStyle,
  presentationChartDataLabelPositionLabel,
  presentationChartLegendPositionLabel,
  presentationChartShowsLegend,
  presentationChartTypeLabel,
  presentationChartUsesNumericXAxis,
} from './work-presentation-charts';
import {
  workSpreadsheetChartAxisDefaultLabelPosition,
  workSpreadsheetChartAxisLabelPositionLabel,
  workSpreadsheetChartAxisTickMarkLabel,
} from './work-spreadsheet-chart-axis';
import {
  normalizeWorkSpreadsheetChartGapWidth,
  normalizeWorkSpreadsheetChartGrouping,
  normalizeWorkSpreadsheetChartOverlap,
  normalizeWorkSpreadsheetChartSmoothLines,
  workSpreadsheetChartGroupingLabel,
  workSpreadsheetChartSupportsBarSpacing,
  workSpreadsheetChartSupportsGrouping,
  workSpreadsheetChartSupportsSeriesAnalysis,
  workSpreadsheetChartSupportsSmoothLines,
} from './work-spreadsheet-chart-layout';
import { spreadsheetChartSeriesStyleContext } from './work-spreadsheet-chart-series-style';
import {
  type WorkSlide,
  type WorkSlideChart,
  type WorkSlideElement,
  type WorkSlideRadarStyle,
  workSpreadsheetChartSupportsErrorBars,
  workSpreadsheetChartSupportsTrendlines,
} from './work-types';

export function presentationAgentSelection(
  slide: WorkSlide,
  slideIndex: number,
  slideCount: number,
  element?: WorkSlideElement | null
): string {
  const heading = [`幻灯片 ${slideIndex + 1} / ${slideCount}`, `名称：${slide.name}`, `背景：${slide.background}`];
  if (element) {
    return [
      ...heading,
      '',
      `当前选择：${slideElementLabel(element)}`,
      `位置：x ${formatPercent(element.x)}，y ${formatPercent(element.y)}，宽 ${formatPercent(
        element.width
      )}，高 ${formatPercent(element.height)}`,
      ...slideElementDetails(element),
      ...(slide.notes?.trim() ? ['', `演讲者备注：${slide.notes.trim()}`] : []),
    ].join('\n');
  }

  const elements = slide.elements.flatMap((candidate, index) => [
    `元素 ${index + 1}（${slideElementLabel(candidate)}）`,
    ...slideElementDetails(candidate).map((line) => `  ${line}`),
  ]);
  return [
    ...heading,
    '',
    '整页内容：',
    ...(elements.length ? elements : ['（空白幻灯片）']),
    ...(slide.notes?.trim() ? ['', `演讲者备注：${slide.notes.trim()}`] : []),
  ].join('\n');
}

export function presentationAgentProposalTargets(
  slide: WorkSlide,
  element?: WorkSlideElement | null
): WorkAgentProposalTarget[] {
  const candidates = element ? [element] : slide.elements;
  return candidates.flatMap((candidate, elementIndex) => {
    const label = element ? slideElementLabel(candidate) : `元素 ${elementIndex + 1} · ${slideElementLabel(candidate)}`;
    if (candidate.table) {
      return candidate.table.rows.flatMap((row, rowIndex) =>
        row.map((cell, columnIndex) => ({
          id: `table:${candidate.id}:${rowIndex}:${columnIndex}`,
          label: `${label} · 单元格 ${rowIndex + 1},${columnIndex + 1}`,
          before: cell,
        }))
      );
    }
    const text = presentationElementText(candidate);
    return text
      ? [
          {
            id: `text:${candidate.id}`,
            label,
            before: text,
          },
        ]
      : [];
  });
}

export function presentationNotesProposalTarget(slide: WorkSlide): WorkAgentProposalTarget {
  return {
    id: 'notes',
    label: `${slide.name} · 演讲者备注`,
    before: slide.notes ?? '',
  };
}

export function presentationElementText(element: WorkSlideElement): string {
  return element.textRuns?.map((run) => run.text).join('') || element.text;
}

function slideElementDetails(element: WorkSlideElement): string[] {
  const lines: string[] = [];
  const text = presentationElementText(element);
  if (text.trim()) lines.push(`文本：${text.trim()}`);
  if (element.table?.rows.length) {
    lines.push('表格：', ...element.table.rows.map((row) => row.map(normalizeValue).join('\t')));
  }
  if (element.chart) {
    lines.push(
      `图表：${element.chart.title || '无标题'}（${presentationChartTypeLabel(element.chart.type)}）`,
      `${presentationChartUsesNumericXAxis(element.chart.type) ? 'X 值' : '分类'}：${element.chart.categories
        .map(normalizeValue)
        .join('，')}`,
      ...presentationChartSeriesLines(element.chart),
      `图例：${
        presentationChartShowsLegend(element.chart)
          ? `显示（${presentationChartLegendPositionLabel(
              normalizePresentationChartLegendPosition(element.chart.legendPosition)
            )}）${element.chart.legendOverlay ? '；叠加绘图区' : ''}`
          : '隐藏'
      }`,
      ...presentationChartPlotLayoutLines(element.chart),
      ...presentationChartAxisLines(element.chart),
      ...presentationChartDataLabelLines(element.chart),
      ...(element.chart.type === 'doughnut' ? [`圆环孔径：${element.chart.doughnutHoleSize ?? 50}%`] : []),
      ...(element.chart.type === 'radar' ? [`雷达样式：${radarStyleLabel(element.chart.radarStyle)}`] : []),
      ...(element.chart.type === 'scatter' ? [`散点样式：${scatterStyleLabel(element.chart.scatterStyle)}`] : []),
      ...(element.chart.type === 'bubble' ? [presentationBubbleSettingsLine(element.chart)] : [])
    );
  }
  if (element.image) lines.push(`图片：${element.image.name}`);
  if (element.altText?.trim()) lines.push(`替代文本：${element.altText.trim()}`);
  if (element.href?.trim()) lines.push(`链接：${element.href.trim()}`);
  if (!lines.length) lines.push('（无文本内容）');
  return lines;
}

function presentationChartDataLabelLines(chart: WorkSlideChart): string[] {
  if (!chart.dataLabels) return [];
  const labels = normalizePresentationChartDataLabels(chart.dataLabels, chart.type);
  const content = [
    labels.showValue ? '数值' : '',
    labels.showCategoryName ? '分类名称' : '',
    labels.showSeriesName ? '系列名称' : '',
    labels.showPercentage ? '百分比' : '',
    labels.showBubbleSize ? '气泡大小' : '',
  ].filter(Boolean);
  const position = presentationChartDataLabelPositionLabel(labels.position!);
  return [
    `数据标签：${content.length ? content.join('、') : '无内容'}；位置：${position}；分隔符：“${labels.separator ?? ', '}”`,
  ];
}

function presentationChartSeriesLines(chart: WorkSlideChart): string[] {
  const analysisContext = (series: WorkSlideChart['series'][number]) =>
    `${workSpreadsheetChartSupportsSeriesAnalysis(chart) && workSpreadsheetChartSupportsTrendlines(chart.type) ? chartSeriesTrendlineContext(series) : ''}${
      workSpreadsheetChartSupportsSeriesAnalysis(chart) && workSpreadsheetChartSupportsErrorBars(chart.type)
        ? chartSeriesErrorBarContext(series, chart.type)
        : ''
    }${spreadsheetChartSeriesStyleContext(series.style)}`;
  if (!presentationChartUsesNumericXAxis(chart.type)) {
    return chart.series.map(
      (series) =>
        `系列 ${series.name}：${series.values.map((value) => normalizeValue(String(value))).join('，')}${analysisContext(
          series
        )}`
    );
  }
  const xValues = chart.categories.map(normalizeValue).join('，');
  return chart.series.map(
    (series) =>
      `系列 ${series.name}：X=${xValues}；Y=${series.values.map((value) => normalizeValue(String(value))).join('，')}${
        chart.type === 'bubble'
          ? `；大小=${series.bubbleSizes?.map((value) => normalizeValue(String(value))).join('，') || '无'}`
          : ''
      }${analysisContext(series)}`
  );
}

function presentationChartPlotLayoutLines(chart: WorkSlideChart): string[] {
  if (!workSpreadsheetChartSupportsGrouping(chart.type)) return [];
  const grouping = normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type);
  const settings = [workSpreadsheetChartGroupingLabel(grouping)];
  if (workSpreadsheetChartSupportsBarSpacing(chart.type)) {
    settings.push(`分类间距 ${normalizeWorkSpreadsheetChartGapWidth(chart.gapWidth)}%`);
    settings.push(`系列重叠 ${normalizeWorkSpreadsheetChartOverlap(chart.overlap, grouping)}%`);
  }
  if (workSpreadsheetChartSupportsSmoothLines(chart.type)) {
    settings.push(normalizeWorkSpreadsheetChartSmoothLines(chart.smoothLines) ? '平滑线' : '直线');
  }
  return [`绘图区：${settings.join('，')}`];
}

function presentationBubbleSettingsLine(chart: WorkSlideChart): string {
  return `气泡设置：缩放 ${normalizePresentationBubbleScale(chart.bubbleScale)}%，${
    chart.showNegativeBubbles ? '显示负气泡' : '隐藏负气泡'
  }，大小表示${normalizePresentationBubbleSizeRepresents(chart.bubbleSizeRepresents) === 'width' ? '宽度' : '面积'}`;
}

function scatterStyleLabel(style: WorkSlideChart['scatterStyle']): string {
  const normalized = normalizePresentationScatterStyle(style);
  if (normalized === 'marker') return '仅数据标记';
  if (normalized === 'line') return '直线';
  if (normalized === 'smooth') return '平滑线';
  if (normalized === 'smoothMarker') return '平滑线和数据标记';
  return '直线和数据标记';
}

function presentationChartAxisLines(chart: WorkSlideChart): string[] {
  const axes = presentationChartAxes(chart);
  if (!axes) return [];
  const descriptions = [
    presentationChartAxisDescription(chart, 'bottom', '横轴', axes.bottom),
    presentationChartAxisDescription(chart, 'left', '纵轴', axes.left),
  ].filter(Boolean);
  return descriptions.length ? [`坐标轴：${descriptions.join('；')}`] : [];
}

function presentationChartAxisDescription(
  chart: WorkSlideChart,
  position: 'bottom' | 'left',
  label: string,
  axis: NonNullable<ReturnType<typeof presentationChartAxes>>['bottom']
): string {
  if (!axis) return '';
  const parts = [
    axis.title?.trim() ? `标题“${axis.title.trim()}”` : '',
    !axis.title?.trim() && axis.titleReference?.trim() ? `标题引用“${axis.titleReference.trim()}”` : '',
    axis.reverseOrder ? '逆序' : '',
    axis.labelPosition && axis.labelPosition !== workSpreadsheetChartAxisDefaultLabelPosition(chart.type, position)
      ? `标签${workSpreadsheetChartAxisLabelPositionLabel(axis.labelPosition)}`
      : '',
    axis.majorTickMark && axis.majorTickMark !== 'none'
      ? `主要刻度${workSpreadsheetChartAxisTickMarkLabel(axis.majorTickMark)}`
      : '',
    axis.labelInterval !== undefined ? `标签间隔 ${axis.labelInterval}` : '',
    axis.minimum !== undefined && axis.maximum !== undefined
      ? `范围 ${axis.minimum}–${axis.maximum}`
      : axis.minimum !== undefined
        ? `最小值 ${axis.minimum}`
        : axis.maximum !== undefined
          ? `最大值 ${axis.maximum}`
          : '',
    axis.majorUnit !== undefined ? `主单位 ${axis.majorUnit}` : '',
    axis.numberFormat?.trim() ? `数字格式 ${axis.numberFormat.trim()}` : '',
    axis.showMajorGridlines === true ? '显示主要网格线' : axis.showMajorGridlines === false ? '不显示主要网格线' : '',
  ].filter(Boolean);
  return parts.length ? `${label}（${parts.join('，')}）` : '';
}

function slideElementLabel(element: WorkSlideElement): string {
  if (element.type === 'text') return '文本框';
  if (element.type === 'shape') return '形状';
  if (element.type === 'image') return '图片';
  if (element.type === 'table') return '表格';
  if (element.type === 'chart') return '图表';
  return '线条';
}

function formatPercent(value: number): string {
  return `${Number.isFinite(value) ? value.toFixed(1) : '0.0'}%`;
}

function normalizeValue(value: string): string {
  return value.replace(/\r?\n/g, ' ').replace(/\t/g, ' ').trim();
}

function radarStyleLabel(style: WorkSlideRadarStyle | undefined): string {
  if (style === 'marker') return '带数据标记';
  if (style === 'filled') return '填充';
  return '标准';
}
