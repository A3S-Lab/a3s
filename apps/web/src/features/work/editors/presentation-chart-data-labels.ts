import {
  normalizePresentationChartDataLabelPosition,
  presentationChartDataLabelText,
} from '../work-presentation-charts';
import type { WorkSlideChart, WorkSlideChartDataLabelPosition } from '../work-types';

export type PresentationChartDataLabelGeometry =
  | { kind: 'point'; x: number; y: number }
  | { kind: 'verticalBar'; x: number; valueY: number; baselineY: number; value: number }
  | { kind: 'horizontalBar'; y: number; valueX: number; baselineX: number; value: number }
  | {
      kind: 'circular';
      centerX: number;
      centerY: number;
      angle: number;
      innerRadius: number;
      outerRadius: number;
    };

export function drawPresentationChartDataLabel(
  context: CanvasRenderingContext2D,
  chart: WorkSlideChart,
  seriesIndex: number,
  pointIndex: number,
  geometry: PresentationChartDataLabelGeometry
): void {
  if (!chart.dataLabels) return;
  const text = presentationChartDataLabelText(chart, seriesIndex, pointIndex);
  if (!text) return;
  const position = normalizePresentationChartDataLabelPosition(chart.dataLabels.position, chart.type);
  const placed = placeDataLabel(geometry, position);
  context.save();
  context.fillStyle = '#27344d';
  context.font = '500 9px -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';
  context.textAlign = placed.align;
  context.textBaseline = 'middle';
  context.fillText(truncateDataLabel(text), placed.x, placed.y);
  context.restore();
}

function placeDataLabel(
  geometry: PresentationChartDataLabelGeometry,
  position: WorkSlideChartDataLabelPosition
): { x: number; y: number; align: CanvasTextAlign } {
  if (geometry.kind === 'verticalBar') return placeVerticalBarLabel(geometry, position);
  if (geometry.kind === 'horizontalBar') return placeHorizontalBarLabel(geometry, position);
  if (geometry.kind === 'circular') return placeCircularLabel(geometry, position);
  if (position === 'left') return { x: geometry.x - 5, y: geometry.y, align: 'right' };
  if (position === 'right') return { x: geometry.x + 5, y: geometry.y, align: 'left' };
  if (position === 'below') return { x: geometry.x, y: geometry.y + 9, align: 'center' };
  if (position === 'center' || position === 'insideBase' || position === 'insideEnd') {
    return { x: geometry.x, y: geometry.y, align: 'center' };
  }
  return { x: geometry.x, y: geometry.y - 7, align: 'center' };
}

function placeVerticalBarLabel(
  geometry: Extract<PresentationChartDataLabelGeometry, { kind: 'verticalBar' }>,
  position: WorkSlideChartDataLabelPosition
): { x: number; y: number; align: CanvasTextAlign } {
  const positive = geometry.value >= 0;
  if (position === 'center') {
    return { x: geometry.x, y: (geometry.valueY + geometry.baselineY) / 2, align: 'center' };
  }
  if (position === 'insideBase') {
    return { x: geometry.x, y: geometry.baselineY + (positive ? -7 : 7), align: 'center' };
  }
  if (position === 'insideEnd') {
    return { x: geometry.x, y: geometry.valueY + (positive ? 7 : -7), align: 'center' };
  }
  return { x: geometry.x, y: geometry.valueY + (positive ? -7 : 7), align: 'center' };
}

function placeHorizontalBarLabel(
  geometry: Extract<PresentationChartDataLabelGeometry, { kind: 'horizontalBar' }>,
  position: WorkSlideChartDataLabelPosition
): { x: number; y: number; align: CanvasTextAlign } {
  const positive = geometry.value >= 0;
  if (position === 'center') {
    return { x: (geometry.valueX + geometry.baselineX) / 2, y: geometry.y, align: 'center' };
  }
  if (position === 'insideBase') {
    return { x: geometry.baselineX + (positive ? 5 : -5), y: geometry.y, align: positive ? 'left' : 'right' };
  }
  if (position === 'insideEnd') {
    return { x: geometry.valueX + (positive ? -5 : 5), y: geometry.y, align: positive ? 'right' : 'left' };
  }
  return { x: geometry.valueX + (positive ? 5 : -5), y: geometry.y, align: positive ? 'left' : 'right' };
}

function placeCircularLabel(
  geometry: Extract<PresentationChartDataLabelGeometry, { kind: 'circular' }>,
  position: WorkSlideChartDataLabelPosition
): { x: number; y: number; align: CanvasTextAlign } {
  const middleRadius = (geometry.innerRadius + geometry.outerRadius) / 2;
  const radius =
    position === 'outsideEnd'
      ? geometry.outerRadius * 1.12
      : position === 'insideEnd'
        ? geometry.outerRadius * 0.78
        : middleRadius;
  const x = geometry.centerX + Math.cos(geometry.angle) * radius;
  const y = geometry.centerY + Math.sin(geometry.angle) * radius;
  const align = position === 'outsideEnd' ? (Math.cos(geometry.angle) < 0 ? 'right' : 'left') : 'center';
  return { x, y, align };
}

function truncateDataLabel(value: string): string {
  const singleLine = value.replace(/\r?\n/g, ' ');
  return singleLine.length > 48 ? `${singleLine.slice(0, 47)}…` : singleLine;
}
