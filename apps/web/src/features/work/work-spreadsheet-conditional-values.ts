export type SpreadsheetConditionalThresholdType = 'min' | 'max' | 'num' | 'percent' | 'percentile';

export interface SpreadsheetConditionalThreshold {
  type: SpreadsheetConditionalThresholdType;
  value?: number;
}

export interface SpreadsheetConditionalVisualOptions {
  thresholds: SpreadsheetConditionalThreshold[];
  showValue?: boolean;
  minLength?: number;
  maxLength?: number;
}

export const DEFAULT_DATA_BAR_MIN_LENGTH = 10;
export const DEFAULT_DATA_BAR_MAX_LENGTH = 90;

export function defaultSpreadsheetColorScaleThresholds(colorCount: number): SpreadsheetConditionalThreshold[] {
  return colorCount === 3
    ? [{ type: 'min' }, { type: 'percentile', value: 50 }, { type: 'max' }]
    : [{ type: 'min' }, { type: 'max' }];
}

export function defaultSpreadsheetDataBarOptions(): Required<SpreadsheetConditionalVisualOptions> {
  return {
    thresholds: [{ type: 'min' }, { type: 'max' }],
    showValue: true,
    minLength: DEFAULT_DATA_BAR_MIN_LENGTH,
    maxLength: DEFAULT_DATA_BAR_MAX_LENGTH,
  };
}

export function normalizeSpreadsheetConditionalThreshold(value: unknown): SpreadsheetConditionalThreshold | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const source = value as Partial<SpreadsheetConditionalThreshold>;
  if (!['min', 'max', 'num', 'percent', 'percentile'].includes(String(source.type))) return null;
  const type = source.type as SpreadsheetConditionalThresholdType;
  if (type === 'min' || type === 'max') return { type };
  if (typeof source.value !== 'number' || !Number.isFinite(source.value)) return null;
  return { type, value: source.value };
}

export function normalizeSpreadsheetConditionalVisualOptions(
  value: unknown,
  expectedThresholdCount: number,
  kind: 'colorScale' | 'dataBar'
): SpreadsheetConditionalVisualOptions | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const source = value as Partial<SpreadsheetConditionalVisualOptions>;
  if (!Array.isArray(source.thresholds) || source.thresholds.length !== expectedThresholdCount) return null;
  const thresholds = source.thresholds.map(normalizeSpreadsheetConditionalThreshold);
  if (thresholds.some((threshold) => !threshold)) return null;
  if (kind === 'colorScale') return { thresholds: thresholds as SpreadsheetConditionalThreshold[] };
  const minLength = source.minLength ?? DEFAULT_DATA_BAR_MIN_LENGTH;
  const maxLength = source.maxLength ?? DEFAULT_DATA_BAR_MAX_LENGTH;
  if (
    typeof minLength !== 'number' ||
    !Number.isFinite(minLength) ||
    minLength < 0 ||
    minLength > 100 ||
    typeof maxLength !== 'number' ||
    !Number.isFinite(maxLength) ||
    maxLength < 0 ||
    maxLength > 100 ||
    minLength > maxLength
  ) {
    return null;
  }
  return {
    thresholds: thresholds as SpreadsheetConditionalThreshold[],
    showValue: source.showValue !== false,
    minLength,
    maxLength,
  };
}

export function spreadsheetConditionalThresholdValue(
  threshold: SpreadsheetConditionalThreshold,
  sourceValues: number[]
): number | null {
  const values = sourceValues.filter(Number.isFinite).sort((left, right) => left - right);
  if (!values.length) return null;
  const minimum = values[0];
  const maximum = values.at(-1)!;
  if (threshold.type === 'min') return minimum;
  if (threshold.type === 'max') return maximum;
  if (threshold.type === 'num') return threshold.value ?? null;
  const percentage = threshold.value;
  if (percentage === undefined || !Number.isFinite(percentage)) return null;
  if (threshold.type === 'percent') return minimum + ((maximum - minimum) * percentage) / 100;
  const position = (Math.max(0, Math.min(100, percentage)) / 100) * (values.length - 1);
  const lower = Math.floor(position);
  const upper = Math.ceil(position);
  if (lower === upper) return values[lower];
  return values[lower] + (values[upper] - values[lower]) * (position - lower);
}

export function spreadsheetConditionalThresholdsEqual(
  left: SpreadsheetConditionalThreshold[],
  right: SpreadsheetConditionalThreshold[]
): boolean {
  return (
    left.length === right.length &&
    left.every((threshold, index) => threshold.type === right[index]?.type && threshold.value === right[index]?.value)
  );
}
