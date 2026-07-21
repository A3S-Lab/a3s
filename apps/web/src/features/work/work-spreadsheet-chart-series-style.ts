import {
  type WorkSpreadsheetChartLineDash,
  type WorkSpreadsheetChartMarkerStyle,
  type WorkSpreadsheetChartMarkerSymbol,
  type WorkSpreadsheetChartSeries,
  type WorkSpreadsheetChartSeriesStyle,
} from './work-types';
import { escapeChartXml, roundChartNumber, WORK_SPREADSHEET_CHART_COLORS } from './work-spreadsheet-chart-svg-utils';

const LINE_DASHES = new Set<WorkSpreadsheetChartLineDash>(['solid', 'dash', 'dot', 'dashDot']);
const MARKER_SYMBOLS = new Set<WorkSpreadsheetChartMarkerSymbol>([
  'none',
  'circle',
  'square',
  'diamond',
  'triangle',
  'plus',
  'x',
  'star',
]);

export interface SpreadsheetChartSeriesFillStyle {
  color: string;
  opacity: number;
  attributes: string;
}

export interface SpreadsheetChartSeriesLineStyle {
  color: string;
  width: number;
  dash: WorkSpreadsheetChartLineDash;
  attributes: string;
}

interface SpreadsheetChartMarkerOptions {
  visible: boolean;
  defaultSize?: number;
  attributes?: string;
}

export function normalizeWorkSpreadsheetChartSeriesStyle(
  source: WorkSpreadsheetChartSeriesStyle | null | undefined | Record<string, unknown>
): WorkSpreadsheetChartSeriesStyle | undefined {
  if (!source || typeof source !== 'object') return undefined;
  const fillColor = normalizeWorkSpreadsheetChartColor(source.fillColor);
  const fillTransparency = optionalBoundedNumber(source.fillTransparency, 0, 100, 0);
  const lineColor = normalizeWorkSpreadsheetChartColor(source.lineColor);
  const lineWidth = optionalBoundedNumber(source.lineWidth, 0.25, 20, 2.25);
  const lineDash = LINE_DASHES.has(source.lineDash as WorkSpreadsheetChartLineDash)
    ? (source.lineDash as WorkSpreadsheetChartLineDash)
    : undefined;
  const marker = normalizeWorkSpreadsheetChartMarkerStyle(source.marker);
  const normalized: WorkSpreadsheetChartSeriesStyle = {
    ...(fillColor ? { fillColor } : {}),
    ...(fillTransparency !== undefined ? { fillTransparency } : {}),
    ...(lineColor ? { lineColor } : {}),
    ...(lineWidth !== undefined ? { lineWidth } : {}),
    ...(lineDash ? { lineDash } : {}),
    ...(marker ? { marker } : {}),
  };
  return Object.keys(normalized).length ? normalized : undefined;
}

export function normalizeWorkSpreadsheetChartColor(value: unknown): string | undefined {
  if (typeof value !== 'string') return undefined;
  const text = value.trim().replace(/^#/, '');
  if (/^[0-9a-f]{3}$/i.test(text)) {
    return `#${text
      .split('')
      .map((character) => character.repeat(2))
      .join('')}`.toUpperCase();
  }
  return /^[0-9a-f]{6}$/i.test(text) ? `#${text.toUpperCase()}` : undefined;
}

export function defaultWorkSpreadsheetChartSeriesStyle(seriesIndex: number): WorkSpreadsheetChartSeriesStyle {
  const color = workSpreadsheetChartSeriesDefaultColor(seriesIndex);
  return {
    fillColor: color,
    fillTransparency: 0,
    lineColor: color,
    lineWidth: 2.5,
    lineDash: 'solid',
    marker: {
      symbol: 'circle',
      size: 5,
      fillColor: '#FFFFFF',
      lineColor: color,
    },
  };
}

export function workSpreadsheetChartSeriesDefaultColor(seriesIndex: number): string {
  return WORK_SPREADSHEET_CHART_COLORS[seriesIndex % WORK_SPREADSHEET_CHART_COLORS.length].toUpperCase();
}

export function spreadsheetChartSeriesFillStyle(
  series: WorkSpreadsheetChartSeries,
  seriesIndex: number,
  defaultOpacity = 1,
  defaultColor = workSpreadsheetChartSeriesDefaultColor(seriesIndex)
): SpreadsheetChartSeriesFillStyle {
  const style = normalizeWorkSpreadsheetChartSeriesStyle(series.style);
  const color = style?.fillColor ?? defaultColor;
  const opacity =
    style?.fillTransparency === undefined ? boundedNumber(defaultOpacity, 0, 1, 1) : 1 - style.fillTransparency / 100;
  return {
    color,
    opacity,
    attributes: `fill="${escapeChartXml(color)}"${opacity < 1 ? ` fill-opacity="${compactDecimal(opacity)}"` : ''}`,
  };
}

export function spreadsheetChartSeriesLineStyle(
  series: WorkSpreadsheetChartSeries,
  seriesIndex: number,
  defaultWidth: number,
  defaultColor = workSpreadsheetChartSeriesDefaultColor(seriesIndex)
): SpreadsheetChartSeriesLineStyle {
  const style = normalizeWorkSpreadsheetChartSeriesStyle(series.style);
  const hasCustomLine = Boolean(style?.lineColor || style?.lineWidth !== undefined || style?.lineDash);
  const color = style?.lineColor ?? defaultColor;
  const width = style?.lineWidth ?? (hasCustomLine && defaultWidth <= 0 ? 1 : defaultWidth);
  const dash = style?.lineDash ?? 'solid';
  const dashArray = spreadsheetChartLineDashArray(dash);
  return {
    color,
    width,
    dash,
    attributes:
      !hasCustomLine && defaultWidth <= 0
        ? 'stroke="none"'
        : `stroke="${escapeChartXml(color)}" stroke-width="${compactDecimal(width)}"${
            dashArray ? ` stroke-dasharray="${dashArray}"` : ''
          }`,
  };
}

export function spreadsheetChartSeriesLegendColor(series: WorkSpreadsheetChartSeries, seriesIndex: number): string {
  const style = normalizeWorkSpreadsheetChartSeriesStyle(series.style);
  return style?.fillColor ?? style?.lineColor ?? workSpreadsheetChartSeriesDefaultColor(seriesIndex);
}

export function spreadsheetChartSeriesMarkerSvg(
  series: WorkSpreadsheetChartSeries,
  seriesIndex: number,
  x: number,
  y: number,
  options: SpreadsheetChartMarkerOptions
): string {
  const style = normalizeWorkSpreadsheetChartSeriesStyle(series.style);
  const marker = style?.marker;
  const visible = marker ? marker.symbol !== 'none' : options.visible;
  if (!visible) return '';
  const symbol = marker?.symbol && marker.symbol !== 'none' ? marker.symbol : 'circle';
  const size = marker?.size ?? options.defaultSize ?? 5;
  const radius = Math.max(1, size * 0.6);
  const line = spreadsheetChartSeriesLineStyle(series, seriesIndex, 2);
  const fillColor = marker?.fillColor ?? '#FFFFFF';
  const lineColor = marker?.lineColor ?? line.color;
  const common = `data-marker-symbol="${symbol}" data-marker-size="${compactDecimal(size)}"${
    options.attributes ? ` ${options.attributes}` : ''
  }`;
  const paint = `fill="${escapeChartXml(fillColor)}" stroke="${escapeChartXml(lineColor)}" stroke-width="2"`;
  const pointX = roundChartNumber(x);
  const pointY = roundChartNumber(y);
  if (symbol === 'square') {
    const side = radius * 1.7;
    return `<rect ${common} x="${roundChartNumber(x - side / 2)}" y="${roundChartNumber(
      y - side / 2
    )}" width="${roundChartNumber(side)}" height="${roundChartNumber(side)}" rx="0.6" ${paint}/>`;
  }
  if (symbol === 'diamond') {
    return `<path ${common} d="M ${pointX} ${roundChartNumber(y - radius)} L ${roundChartNumber(
      x + radius
    )} ${pointY} L ${pointX} ${roundChartNumber(y + radius)} L ${roundChartNumber(x - radius)} ${pointY} Z" ${paint}/>`;
  }
  if (symbol === 'triangle') {
    return `<path ${common} d="M ${pointX} ${roundChartNumber(y - radius)} L ${roundChartNumber(
      x + radius * 0.9
    )} ${roundChartNumber(y + radius * 0.75)} L ${roundChartNumber(x - radius * 0.9)} ${roundChartNumber(
      y + radius * 0.75
    )} Z" ${paint}/>`;
  }
  if (symbol === 'plus' || symbol === 'x') {
    const paths =
      symbol === 'plus'
        ? `M ${roundChartNumber(x - radius)} ${pointY} L ${roundChartNumber(x + radius)} ${pointY} M ${pointX} ${roundChartNumber(
            y - radius
          )} L ${pointX} ${roundChartNumber(y + radius)}`
        : `M ${roundChartNumber(x - radius)} ${roundChartNumber(y - radius)} L ${roundChartNumber(
            x + radius
          )} ${roundChartNumber(y + radius)} M ${roundChartNumber(x + radius)} ${roundChartNumber(
            y - radius
          )} L ${roundChartNumber(x - radius)} ${roundChartNumber(y + radius)}`;
    return `<path ${common} d="${paths}" fill="none" stroke="${escapeChartXml(
      lineColor
    )}" stroke-width="2" stroke-linecap="round"/>`;
  }
  if (symbol === 'star') {
    const points = Array.from({ length: 10 }, (_, index) => {
      const angle = -Math.PI / 2 + (index * Math.PI) / 5;
      const pointRadius = index % 2 ? radius * 0.42 : radius;
      return `${roundChartNumber(x + Math.cos(angle) * pointRadius)},${roundChartNumber(
        y + Math.sin(angle) * pointRadius
      )}`;
    }).join(' ');
    return `<polygon ${common} points="${points}" ${paint}/>`;
  }
  return `<circle ${common} cx="${pointX}" cy="${pointY}" r="${roundChartNumber(radius)}" ${paint}/>`;
}

export function spreadsheetChartSeriesStyleContext(style: WorkSpreadsheetChartSeriesStyle | undefined): string {
  const normalized = normalizeWorkSpreadsheetChartSeriesStyle(style);
  if (!normalized) return '';
  const descriptions: string[] = [];
  if (normalized.fillColor || normalized.fillTransparency !== undefined) {
    descriptions.push(
      `填充 ${normalized.fillColor ?? '默认颜色'}${
        normalized.fillTransparency === undefined ? '' : `（透明度 ${compactDecimal(normalized.fillTransparency)}%）`
      }`
    );
  }
  if (normalized.lineColor || normalized.lineWidth !== undefined || normalized.lineDash) {
    descriptions.push(
      [
        `线条 ${normalized.lineColor ?? '默认颜色'}`,
        normalized.lineWidth === undefined ? '' : `${compactDecimal(normalized.lineWidth)} 磅`,
        normalized.lineDash ? workSpreadsheetChartLineDashLabel(normalized.lineDash) : '',
      ]
        .filter(Boolean)
        .join('、')
    );
  }
  if (normalized.marker) {
    const marker = normalized.marker;
    descriptions.push(
      [
        `数据标记 ${workSpreadsheetChartMarkerSymbolLabel(marker.symbol ?? 'circle')}`,
        marker.size === undefined ? '' : `${compactDecimal(marker.size)} 磅`,
        marker.fillColor ? `填充 ${marker.fillColor}` : '',
        marker.lineColor ? `轮廓 ${marker.lineColor}` : '',
      ]
        .filter(Boolean)
        .join('、')
    );
  }
  return descriptions.length ? `；系列外观：${descriptions.join('，')}` : '';
}

export function workSpreadsheetChartLineDashLabel(dash: WorkSpreadsheetChartLineDash): string {
  if (dash === 'dash') return '虚线';
  if (dash === 'dot') return '点线';
  if (dash === 'dashDot') return '点划线';
  return '实线';
}

export function workSpreadsheetChartMarkerSymbolLabel(symbol: WorkSpreadsheetChartMarkerSymbol): string {
  if (symbol === 'none') return '无';
  if (symbol === 'square') return '方形';
  if (symbol === 'diamond') return '菱形';
  if (symbol === 'triangle') return '三角形';
  if (symbol === 'plus') return '加号';
  if (symbol === 'x') return '叉号';
  if (symbol === 'star') return '星形';
  return '圆形';
}

function normalizeWorkSpreadsheetChartMarkerStyle(value: unknown): WorkSpreadsheetChartMarkerStyle | undefined {
  if (!value || typeof value !== 'object') return undefined;
  const source = value as Record<string, unknown>;
  const symbol = MARKER_SYMBOLS.has(source.symbol as WorkSpreadsheetChartMarkerSymbol)
    ? (source.symbol as WorkSpreadsheetChartMarkerSymbol)
    : undefined;
  const size = optionalBoundedNumber(source.size, 2, 72, 5);
  const fillColor = normalizeWorkSpreadsheetChartColor(source.fillColor);
  const lineColor = normalizeWorkSpreadsheetChartColor(source.lineColor);
  const marker: WorkSpreadsheetChartMarkerStyle = {
    ...(symbol ? { symbol } : {}),
    ...(size !== undefined ? { size } : {}),
    ...(fillColor ? { fillColor } : {}),
    ...(lineColor ? { lineColor } : {}),
  };
  return Object.keys(marker).length ? marker : undefined;
}

function spreadsheetChartLineDashArray(dash: WorkSpreadsheetChartLineDash): string {
  if (dash === 'dash') return '8 4';
  if (dash === 'dot') return '2 3';
  if (dash === 'dashDot') return '8 4 2 4';
  return '';
}

function optionalBoundedNumber(value: unknown, minimum: number, maximum: number, fallback: number): number | undefined {
  if (value === undefined || value === null || value === '') return undefined;
  const number = Number(value);
  return boundedNumber(number, minimum, maximum, fallback);
}

function boundedNumber(value: number, minimum: number, maximum: number, fallback: number): number {
  return Math.min(maximum, Math.max(minimum, Number.isFinite(value) ? value : fallback));
}

function compactDecimal(value: number): string {
  return String(Math.round(value * 100) / 100);
}
