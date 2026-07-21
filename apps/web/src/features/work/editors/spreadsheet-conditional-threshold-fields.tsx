import type { SpreadsheetConditionalThresholdType } from '../work-spreadsheet-conditional-values';
import type { SpreadsheetConditionalThresholdDraft } from './spreadsheet-conditional-format-model';

interface SpreadsheetConditionalThresholdFieldsProps {
  label: string;
  thresholds: SpreadsheetConditionalThresholdDraft[];
  startIndex?: number;
  showEquality?: boolean;
  onChange: (index: number, patch: Partial<SpreadsheetConditionalThresholdDraft>) => void;
}

export function SpreadsheetConditionalThresholdFields({
  label,
  thresholds,
  startIndex = 0,
  showEquality = false,
  onChange,
}: SpreadsheetConditionalThresholdFieldsProps) {
  return (
    <fieldset className='work-spreadsheet-conditional-thresholds'>
      <legend>{label}阈值</legend>
      {thresholds.slice(startIndex).map((threshold, offset) => {
        const index = startIndex + offset;
        const valueRequired = threshold.type !== 'min' && threshold.type !== 'max';
        return (
          <div key={index}>
            <span>第 {index + 1} 级起点</span>
            <select
              aria-label={`${label}阈值 ${index + 1} 类型`}
              value={threshold.type}
              onChange={(event) =>
                onChange(index, {
                  type: event.target.value as SpreadsheetConditionalThresholdType,
                })
              }
            >
              <option value='percent'>百分比</option>
              <option value='percentile'>百分位</option>
              <option value='num'>数值</option>
              <option value='min'>最小值</option>
              <option value='max'>最大值</option>
            </select>
            <input
              type='number'
              aria-label={`${label}阈值 ${index + 1}`}
              value={valueRequired ? threshold.value : ''}
              disabled={!valueRequired}
              onChange={(event) => onChange(index, { value: event.target.value })}
            />
            {showEquality ? (
              <label className='threshold-gte'>
                <input
                  type='checkbox'
                  aria-label={`${label}阈值 ${index + 1} 包含等于`}
                  checked={threshold.gte}
                  onChange={(event) => onChange(index, { gte: event.target.checked })}
                />
                <span>包含等于</span>
              </label>
            ) : (
              <span />
            )}
          </div>
        );
      })}
    </fieldset>
  );
}
