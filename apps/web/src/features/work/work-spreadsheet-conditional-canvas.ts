import type { Cell } from '@fortune-sheet/core';
import type { SpreadsheetConditionalDataBar } from './work-spreadsheet-conditional-data-bar';
import type { SpreadsheetConditionalIconBounds } from './work-spreadsheet-conditional-icons';

export function drawSpreadsheetConditionalDataBar(
  context: CanvasRenderingContext2D,
  bounds: SpreadsheetConditionalIconBounds,
  dataBar: SpreadsheetConditionalDataBar,
  cell: Cell | null,
  background: string,
  textColor?: string
): void {
  const width = bounds.endX - bounds.startX;
  const height = bounds.endY - bounds.startY;
  if (width < 6 || height < 6) return;
  context.save();
  context.beginPath();
  context.rect(bounds.startX + 1, bounds.startY + 1, Math.max(0, width - 3), Math.max(0, height - 3));
  context.clip();
  context.fillStyle = background;
  context.fillRect(bounds.startX + 1, bounds.startY + 1, Math.max(0, width - 3), Math.max(0, height - 3));

  const barLeft = bounds.startX + (width * dataBar.startPercent) / 100;
  const barWidth = (width * dataBar.widthPercent) / 100;
  if (barWidth > 0) {
    context.globalAlpha = 0.52;
    context.fillStyle = dataBar.color;
    context.fillRect(barLeft, bounds.startY + 2, barWidth, Math.max(0, height - 5));
    context.globalAlpha = 1;
  }
  if (dataBar.axisPercent !== undefined && dataBar.axisPercent > 0 && dataBar.axisPercent < 100) {
    const axis = bounds.startX + (width * dataBar.axisPercent) / 100;
    context.beginPath();
    context.moveTo(axis, bounds.startY + 1);
    context.lineTo(axis, bounds.endY - 2);
    context.lineWidth = 1;
    context.strokeStyle = 'rgba(77, 86, 104, 0.58)';
    context.stroke();
  }
  if (dataBar.showValue) drawCellValue(context, bounds, cell, textColor);
  context.restore();
}

export function drawSpreadsheetCommentMarker(
  context: CanvasRenderingContext2D,
  bounds: SpreadsheetConditionalIconBounds
): void {
  const size = Math.min(8, Math.max(4, (bounds.endY - bounds.startY) / 3));
  context.save();
  context.beginPath();
  context.moveTo(bounds.endX - size - 1, bounds.startY + 1);
  context.lineTo(bounds.endX - 1, bounds.startY + 1);
  context.lineTo(bounds.endX - 1, bounds.startY + size + 1);
  context.fillStyle = '#fc6666';
  context.fill();
  context.restore();
}

function drawCellValue(
  context: CanvasRenderingContext2D,
  bounds: SpreadsheetConditionalIconBounds,
  cell: Cell | null,
  conditionalTextColor?: string
): void {
  const value = cell?.m ?? cell?.v ?? (cell?.f ? cell.f : '');
  if (value === null || value === undefined || String(value).length === 0) return;
  const height = bounds.endY - bounds.startY;
  const fontSize = typeof cell?.fs === 'number' ? cell.fs : Math.max(9, Math.min(12, height * 0.48));
  const family = typeof cell?.ff === 'string' ? `"${cell.ff.replaceAll('"', '\\"')}"` : 'Arial, sans-serif';
  context.font = `${cell?.it ? 'italic ' : ''}${cell?.bl ? '700 ' : '400 '}${fontSize}px ${family}`;
  context.fillStyle = conditionalTextColor ?? cell?.fc ?? '#222222';
  context.textBaseline = 'middle';
  const horizontalAlignment = cell?.ht;
  if (horizontalAlignment === 0) {
    context.textAlign = 'center';
    context.fillText(
      String(value),
      (bounds.startX + bounds.endX) / 2,
      bounds.startY + height / 2,
      bounds.endX - bounds.startX - 8
    );
  } else if (horizontalAlignment === 2 || (horizontalAlignment === undefined && typeof cell?.v === 'number')) {
    context.textAlign = 'right';
    context.fillText(String(value), bounds.endX - 4, bounds.startY + height / 2, bounds.endX - bounds.startX - 8);
  } else {
    context.textAlign = 'left';
    context.fillText(String(value), bounds.startX + 4, bounds.startY + height / 2, bounds.endX - bounds.startX - 8);
  }
}
