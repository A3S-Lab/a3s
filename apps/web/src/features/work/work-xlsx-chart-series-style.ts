import { attribute, directChild, directChildren } from './work-ooxml-package';
import {
  normalizeWorkSpreadsheetChartColor,
  normalizeWorkSpreadsheetChartSeriesStyle,
} from './work-spreadsheet-chart-series-style';
import type {
  WorkSpreadsheetChartLineDash,
  WorkSpreadsheetChartMarkerStyle,
  WorkSpreadsheetChartMarkerSymbol,
  WorkSpreadsheetChartSeriesStyle,
} from './work-types';

const EMUS_PER_POINT = 12_700;
const DASH_VALUES = new Set<WorkSpreadsheetChartLineDash>(['solid', 'dash', 'dot', 'dashDot']);
const MARKER_VALUES = new Set<WorkSpreadsheetChartMarkerSymbol>([
  'none',
  'circle',
  'square',
  'diamond',
  'triangle',
  'plus',
  'x',
  'star',
]);
const MARKER_CHART_ELEMENTS = new Set(['lineChart', 'radarChart', 'scatterChart']);

interface XlsxSolidColor {
  color: string;
  transparency?: number;
}

export function parseXlsxChartSeriesStyle(series: Element): WorkSpreadsheetChartSeriesStyle | undefined {
  const shape = directChild(series, 'spPr');
  const base = shape ? parseShapeProperties(shape) : undefined;
  const markerNode = directChild(series, 'marker');
  const marker = markerNode ? parseMarkerStyle(markerNode) : undefined;
  return normalizeWorkSpreadsheetChartSeriesStyle({
    ...base,
    ...(marker ? { marker } : {}),
  });
}

export function xlsxChartSeriesShapePropertiesXml(style: WorkSpreadsheetChartSeriesStyle | undefined): string {
  const normalized = normalizeWorkSpreadsheetChartSeriesStyle(style);
  if (!normalized) return '';
  const fill = normalized.fillColor ? xlsxSolidFillXml(normalized.fillColor, normalized.fillTransparency) : '';
  const line = xlsxLineXml(normalized);
  return fill || line ? `<c:spPr>${fill}${line}</c:spPr>` : '';
}

export function xlsxChartSeriesMarkerXml(
  style: WorkSpreadsheetChartSeriesStyle | undefined,
  defaultVisible: boolean,
  defaultSize = 5
): string {
  const marker = normalizeWorkSpreadsheetChartSeriesStyle(style)?.marker;
  if (!marker) {
    return defaultVisible
      ? `<c:marker><c:symbol val="circle"/><c:size val="${defaultSize}"/></c:marker>`
      : '<c:marker><c:symbol val="none"/></c:marker>';
  }
  const symbol = marker.symbol ?? 'circle';
  if (symbol === 'none') return '<c:marker><c:symbol val="none"/></c:marker>';
  const size = marker.size ?? defaultSize;
  const shape = xlsxMarkerShapePropertiesXml(marker);
  return `<c:marker><c:symbol val="${symbol}"/><c:size val="${size}"/>${shape}</c:marker>`;
}

export function isSupportedXlsxChartSeriesFormatting(series: Element, chartLocalName: string): boolean {
  const shapeProperties = directChildren(series, 'spPr');
  if (shapeProperties.length > 1 || shapeProperties.some((shape) => !isSupportedSeriesShapeProperties(shape))) {
    return false;
  }
  const markers = directChildren(series, 'marker');
  if (markers.length > 1) return false;
  if (!markers.length) return true;
  if (!MARKER_CHART_ELEMENTS.has(chartLocalName)) return false;
  return isSupportedMarker(markers[0]);
}

export function xlsxChartSeriesFormattingShapeProperties(series: Element): Element[] {
  return [
    ...directChildren(series, 'spPr'),
    ...directChildren(series, 'marker').flatMap((marker) => directChildren(marker, 'spPr')),
  ];
}

function parseShapeProperties(shape: Element): WorkSpreadsheetChartSeriesStyle | undefined {
  const fill = parseSolidFill(directChild(shape, 'solidFill'));
  const line = directChild(shape, 'ln');
  const lineFill = line ? parseSolidFill(directChild(line, 'solidFill')) : undefined;
  const width = line ? xlsxLineWidth(line) : undefined;
  const dash = line ? xlsxLineDash(line) : undefined;
  return normalizeWorkSpreadsheetChartSeriesStyle({
    ...(fill
      ? {
          fillColor: fill.color,
          ...(fill.transparency !== undefined ? { fillTransparency: fill.transparency } : {}),
        }
      : {}),
    ...(lineFill ? { lineColor: lineFill.color } : {}),
    ...(width !== undefined ? { lineWidth: width } : {}),
    ...(dash ? { lineDash: dash } : {}),
  });
}

function parseMarkerStyle(marker: Element): WorkSpreadsheetChartMarkerStyle | undefined {
  const symbol = attribute(directChild(marker, 'symbol') ?? marker, 'val');
  const sizeSource = attribute(directChild(marker, 'size') ?? marker, 'val');
  const size = sizeSource === null ? undefined : Number(sizeSource);
  const shape = directChild(marker, 'spPr');
  const fill = shape ? parseSolidFill(directChild(shape, 'solidFill')) : undefined;
  const line = shape ? directChild(shape, 'ln') : undefined;
  const lineFill = line ? parseSolidFill(directChild(line, 'solidFill')) : undefined;
  const parsed: WorkSpreadsheetChartMarkerStyle = {
    ...(MARKER_VALUES.has(symbol as WorkSpreadsheetChartMarkerSymbol)
      ? { symbol: symbol as WorkSpreadsheetChartMarkerSymbol }
      : {}),
    ...(Number.isFinite(size) ? { size } : {}),
    ...(fill ? { fillColor: fill.color } : {}),
    ...(lineFill ? { lineColor: lineFill.color } : {}),
  };
  return Object.keys(parsed).length ? parsed : undefined;
}

function parseSolidFill(fill: Element | undefined): XlsxSolidColor | undefined {
  if (!fill) return undefined;
  const color = directChild(fill, 'srgbClr');
  const normalized = normalizeWorkSpreadsheetChartColor(attribute(color ?? fill, 'val'));
  if (!color || !normalized) return undefined;
  const alphaElement = directChild(color, 'alpha');
  if (!alphaElement) return { color: normalized };
  const alphaSource = attribute(alphaElement, 'val');
  if (alphaSource === null) return { color: normalized };
  const alpha = Number(alphaSource);
  if (!Number.isFinite(alpha)) return { color: normalized };
  return { color: normalized, transparency: Math.round((100 - alpha / 1000) * 100) / 100 };
}

function xlsxLineWidth(line: Element): number | undefined {
  const source = attribute(line, 'w');
  if (source === null) return undefined;
  const width = Number(source);
  return Number.isFinite(width) ? Math.round((width / EMUS_PER_POINT) * 100) / 100 : undefined;
}

function xlsxLineDash(line: Element): WorkSpreadsheetChartLineDash | undefined {
  const value = attribute(directChild(line, 'prstDash') ?? line, 'val');
  return DASH_VALUES.has(value as WorkSpreadsheetChartLineDash) ? (value as WorkSpreadsheetChartLineDash) : undefined;
}

function xlsxSolidFillXml(color: string, transparency?: number): string {
  const value = color.replace(/^#/, '');
  if (transparency === undefined) return `<a:solidFill><a:srgbClr val="${value}"/></a:solidFill>`;
  return `<a:solidFill><a:srgbClr val="${value}"><a:alpha val="${Math.round(
    (100 - transparency) * 1000
  )}"/></a:srgbClr></a:solidFill>`;
}

function xlsxLineXml(style: WorkSpreadsheetChartSeriesStyle): string {
  if (!style.lineColor && style.lineWidth === undefined && !style.lineDash) return '';
  const width = style.lineWidth === undefined ? '' : ` w="${Math.round(style.lineWidth * EMUS_PER_POINT)}"`;
  const fill = style.lineColor ? xlsxSolidFillXml(style.lineColor) : '';
  const dash = style.lineDash ? `<a:prstDash val="${style.lineDash}"/>` : '';
  return `<a:ln${width}>${fill}${dash}</a:ln>`;
}

function xlsxMarkerShapePropertiesXml(marker: WorkSpreadsheetChartMarkerStyle): string {
  const fill = marker.fillColor ? xlsxSolidFillXml(marker.fillColor) : '';
  const line = marker.lineColor ? `<a:ln>${xlsxSolidFillXml(marker.lineColor)}</a:ln>` : '';
  return fill || line ? `<c:spPr>${fill}${line}</c:spPr>` : '';
}

function isSupportedSeriesShapeProperties(shape: Element): boolean {
  if (!hasOnlyAttributes(shape, [])) return false;
  const children = directChildren(shape);
  if (children.some((child) => child.localName !== 'solidFill' && child.localName !== 'ln')) return false;
  const fills = directChildren(shape, 'solidFill');
  const lines = directChildren(shape, 'ln');
  if (fills.length > 1 || lines.length > 1) return false;
  if (fills.some((fill) => !isSupportedSolidFill(fill, true))) return false;
  return !lines.length || isSupportedSeriesLine(lines[0]);
}

function isSupportedSeriesLine(line: Element): boolean {
  if (!hasOnlyAttributes(line, ['w'])) return false;
  const children = directChildren(line);
  if (children.some((child) => child.localName !== 'solidFill' && child.localName !== 'prstDash')) return false;
  const fills = directChildren(line, 'solidFill');
  const dashes = directChildren(line, 'prstDash');
  if (fills.length > 1 || dashes.length > 1 || fills.some((fill) => !isSupportedSolidFill(fill, false))) {
    return false;
  }
  const widthSource = attribute(line, 'w');
  if (widthSource !== null) {
    const width = Number(widthSource);
    if (!Number.isInteger(width) || width < 0.25 * EMUS_PER_POINT || width > 20 * EMUS_PER_POINT) return false;
  }
  return !dashes.length || DASH_VALUES.has(attribute(dashes[0], 'val') as WorkSpreadsheetChartLineDash);
}

function isSupportedMarker(marker: Element): boolean {
  if (!hasOnlyAttributes(marker, [])) return false;
  const children = directChildren(marker);
  if (children.some((child) => !['symbol', 'size', 'spPr'].includes(child.localName))) return false;
  const symbols = directChildren(marker, 'symbol');
  const sizes = directChildren(marker, 'size');
  const shapes = directChildren(marker, 'spPr');
  if (symbols.length !== 1 || sizes.length > 1 || shapes.length > 1) return false;
  const symbol = attribute(symbols[0], 'val');
  if (!hasOnlyAttributes(symbols[0], ['val'])) return false;
  if (!MARKER_VALUES.has(symbol as WorkSpreadsheetChartMarkerSymbol)) return false;
  if (sizes.length) {
    if (!hasOnlyAttributes(sizes[0], ['val'])) return false;
    const size = Number(attribute(sizes[0], 'val'));
    if (!Number.isInteger(size) || size < 2 || size > 72 || symbol === 'none') return false;
  }
  return !shapes.length || isSupportedMarkerShapeProperties(shapes[0]);
}

function isSupportedMarkerShapeProperties(shape: Element): boolean {
  if (!hasOnlyAttributes(shape, [])) return false;
  const children = directChildren(shape);
  if (children.some((child) => child.localName !== 'solidFill' && child.localName !== 'ln')) return false;
  const fills = directChildren(shape, 'solidFill');
  const lines = directChildren(shape, 'ln');
  if (fills.length > 1 || lines.length > 1 || fills.some((fill) => !isSupportedSolidFill(fill, false))) return false;
  if (!lines.length) return true;
  const line = lines[0];
  const lineChildren = directChildren(line);
  return (
    hasOnlyAttributes(line, []) &&
    lineChildren.length <= 1 &&
    lineChildren.every((child) => child.localName === 'solidFill' && isSupportedSolidFill(child, false))
  );
}

function isSupportedSolidFill(fill: Element, allowAlpha: boolean): boolean {
  if (!hasOnlyAttributes(fill, [])) return false;
  const children = directChildren(fill);
  if (children.length !== 1 || children[0].localName !== 'srgbClr') return false;
  const color = children[0];
  if (!hasOnlyAttributes(color, ['val'])) return false;
  if (!/^[0-9a-f]{6}$/i.test(attribute(color, 'val') ?? '')) return false;
  const transforms = directChildren(color);
  if (transforms.some((transform) => transform.localName !== 'alpha') || transforms.length > (allowAlpha ? 1 : 0)) {
    return false;
  }
  if (!transforms.length) return true;
  if (!hasOnlyAttributes(transforms[0], ['val'])) return false;
  const alpha = Number(attribute(transforms[0], 'val'));
  return Number.isInteger(alpha) && alpha >= 0 && alpha <= 100_000;
}

function hasOnlyAttributes(element: Element, allowed: string[]): boolean {
  return Array.from(element.attributes).every(
    (item) => item.name === 'xmlns' || item.prefix === 'xmlns' || allowed.includes(item.localName)
  );
}
