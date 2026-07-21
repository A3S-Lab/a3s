export interface SpreadsheetConditionalRgbColor {
  red: number;
  green: number;
  blue: number;
}

export function parseSpreadsheetConditionalColor(value: string): SpreadsheetConditionalRgbColor | null {
  const hexadecimal = /^#?([0-9a-f]{6})$/i.exec(value.trim());
  if (hexadecimal) {
    return {
      red: Number.parseInt(hexadecimal[1].slice(0, 2), 16),
      green: Number.parseInt(hexadecimal[1].slice(2, 4), 16),
      blue: Number.parseInt(hexadecimal[1].slice(4, 6), 16),
    };
  }
  const rgb = /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/i.exec(value);
  if (!rgb) return null;
  return {
    red: boundedColorChannel(rgb[1]),
    green: boundedColorChannel(rgb[2]),
    blue: boundedColorChannel(rgb[3]),
  };
}

export function interpolateSpreadsheetConditionalColor(
  start: SpreadsheetConditionalRgbColor,
  end: SpreadsheetConditionalRgbColor,
  amount: number
): SpreadsheetConditionalRgbColor {
  return {
    red: Math.round(start.red + (end.red - start.red) * amount),
    green: Math.round(start.green + (end.green - start.green) * amount),
    blue: Math.round(start.blue + (end.blue - start.blue) * amount),
  };
}

export function spreadsheetConditionalCssColor(color: SpreadsheetConditionalRgbColor): string {
  return `rgb(${color.red}, ${color.green}, ${color.blue})`;
}

export function spreadsheetConditionalRatio(value: number, minimum: number, maximum: number): number {
  if (maximum === minimum) return 0;
  return Math.max(0, Math.min(1, (value - minimum) / (maximum - minimum)));
}

function boundedColorChannel(value: string): number {
  return Math.max(0, Math.min(255, Number(value)));
}
