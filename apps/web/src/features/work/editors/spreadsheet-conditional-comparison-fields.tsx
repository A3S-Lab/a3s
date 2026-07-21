import { SPREADSHEET_CONDITIONAL_COMPARISONS, type ConditionalRuleDraft } from './spreadsheet-conditional-format-model';
import {
  spreadsheetConditionalComparisonNeedsUpperValue,
  type SpreadsheetConditionalComparisonOperator,
} from '../work-spreadsheet-conditional-comparisons';

interface SpreadsheetConditionalComparisonFieldsProps {
  draft: ConditionalRuleDraft;
  onChange: (patch: Partial<ConditionalRuleDraft>) => void;
}

export function SpreadsheetConditionalComparisonFields({
  draft,
  onChange,
}: SpreadsheetConditionalComparisonFieldsProps) {
  const needsUpperValue = spreadsheetConditionalComparisonNeedsUpperValue(draft.comparisonOperator);
  return (
    <>
      <label>
        <span>比较方式</span>
        <select
          aria-label='条件比较运算符'
          value={draft.comparisonOperator}
          onChange={(event) =>
            onChange({ comparisonOperator: event.target.value as SpreadsheetConditionalComparisonOperator })
          }
        >
          {SPREADSHEET_CONDITIONAL_COMPARISONS.map((comparison) => (
            <option value={comparison.name} key={comparison.name}>
              {comparison.label}
            </option>
          ))}
        </select>
      </label>
      <label>
        <span>{needsUpperValue ? '下限' : '比较值'}</span>
        <input
          aria-label={needsUpperValue ? '条件比较下限' : '条件比较值'}
          value={draft.comparisonValue}
          onChange={(event) => onChange({ comparisonValue: event.target.value })}
        />
      </label>
      {needsUpperValue && (
        <label>
          <span>上限</span>
          <input
            aria-label='条件比较上限'
            value={draft.comparisonUpperValue}
            onChange={(event) => onChange({ comparisonUpperValue: event.target.value })}
          />
        </label>
      )}
      <label className='toggle'>
        <input
          type='checkbox'
          aria-label='设置文字颜色'
          checked={draft.comparisonUseTextColor}
          onChange={(event) => onChange({ comparisonUseTextColor: event.target.checked })}
        />
        <span>设置文字颜色</span>
      </label>
      <label className='color'>
        <span>文字颜色</span>
        <input
          type='color'
          aria-label='条件文字颜色'
          value={draft.comparisonTextColor}
          disabled={!draft.comparisonUseTextColor}
          onChange={(event) => onChange({ comparisonTextColor: event.target.value })}
        />
      </label>
      <label className='toggle'>
        <input
          type='checkbox'
          aria-label='设置填充颜色'
          checked={draft.comparisonUseCellColor}
          onChange={(event) => onChange({ comparisonUseCellColor: event.target.checked })}
        />
        <span>设置填充颜色</span>
      </label>
      <label className='color'>
        <span>填充颜色</span>
        <input
          type='color'
          aria-label='条件填充颜色'
          value={draft.comparisonCellColor}
          disabled={!draft.comparisonUseCellColor}
          onChange={(event) => onChange({ comparisonCellColor: event.target.value })}
        />
      </label>
    </>
  );
}
