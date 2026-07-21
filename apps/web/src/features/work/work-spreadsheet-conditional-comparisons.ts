export const SPREADSHEET_CONDITIONAL_COMPARISON_OPERATORS = [
  'greaterThan',
  'greaterThanOrEqual',
  'lessThan',
  'lessThanOrEqual',
  'equal',
  'notEqual',
  'between',
  'notBetween',
] as const;

export type SpreadsheetConditionalComparisonOperator = (typeof SPREADSHEET_CONDITIONAL_COMPARISON_OPERATORS)[number];

export function isSpreadsheetConditionalComparisonOperator(
  value: unknown
): value is SpreadsheetConditionalComparisonOperator {
  return SPREADSHEET_CONDITIONAL_COMPARISON_OPERATORS.some((operator) => operator === value);
}

export function spreadsheetConditionalComparisonNeedsUpperValue(
  operator: SpreadsheetConditionalComparisonOperator
): boolean {
  return operator === 'between' || operator === 'notBetween';
}
