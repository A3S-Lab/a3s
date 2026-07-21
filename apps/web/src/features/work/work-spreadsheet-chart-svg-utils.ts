export const WORK_SPREADSHEET_CHART_COLORS = [
  '#4f6bed',
  '#e66c37',
  '#8f95a3',
  '#f4b41a',
  '#36a2ae',
  '#6a9f4e',
] as const;

export function finiteChartNumber(value: number): number {
  return Number.isFinite(value) ? value : 0;
}

export function compactChartNumber(value: number): string {
  const absolute = Math.abs(value);
  if (absolute >= 1_000_000) return `${roundChartNumber(value / 1_000_000)}M`;
  if (absolute >= 1_000) return `${roundChartNumber(value / 1_000)}K`;
  return String(roundChartNumber(value));
}

export function truncateChartText(value: string, length: number): string {
  const text = String(value);
  return text.length > length ? `${text.slice(0, length - 1)}…` : text;
}

export function roundChartNumber(value: number): number {
  return Math.round(value * 100) / 100;
}

export function clampChartNumber(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, Number.isFinite(value) ? value : minimum));
}

export function escapeChartXml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&apos;');
}
