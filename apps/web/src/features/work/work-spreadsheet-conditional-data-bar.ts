export interface SpreadsheetConditionalDataBar {
  color: string;
  startPercent: number;
  widthPercent: number;
  axisPercent?: number;
  showValue: boolean;
}

export function spreadsheetConditionalDataBar(
  color: string,
  value: number,
  minimum: number,
  maximum: number,
  minLength: number,
  maxLength: number,
  showValue: boolean
): SpreadsheetConditionalDataBar {
  if (minimum < 0 && maximum > 0) {
    const axis = (-minimum / (maximum - minimum)) * 100;
    const ratio = value < 0 ? Math.abs(value / minimum) : value > 0 ? value / maximum : 0;
    const width =
      value === 0 ? 0 : displayLength(ratio, minLength, maxLength) * (value < 0 ? axis / 100 : (100 - axis) / 100);
    return {
      color,
      startPercent: value < 0 ? axis - width : axis,
      widthPercent: width,
      axisPercent: axis,
      showValue,
    };
  }
  if (maximum <= 0) {
    const ratio =
      minimum === maximum ? (value === 0 ? 0 : 1) : Math.max(0, Math.min(1, (maximum - value) / (maximum - minimum)));
    const width = value === 0 ? 0 : displayLength(ratio, minLength, maxLength);
    return { color, startPercent: 100 - width, widthPercent: width, axisPercent: 100, showValue };
  }
  const ratio =
    minimum === maximum ? (value === 0 ? 0 : 1) : Math.max(0, Math.min(1, (value - minimum) / (maximum - minimum)));
  const width = value === 0 ? 0 : displayLength(ratio, minLength, maxLength);
  return { color, startPercent: 0, widthPercent: width, axisPercent: 0, showValue };
}

function displayLength(valueRatio: number, minLength: number, maxLength: number): number {
  const normalized = Math.max(0, Math.min(1, valueRatio));
  return minLength + (maxLength - minLength) * normalized;
}
