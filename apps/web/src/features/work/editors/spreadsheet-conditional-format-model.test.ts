import { describe, expect, it } from 'vitest';
import type { FortuneConditionalFormatRule } from '../work-xlsx-conditional-format';
import {
  buildConditionalRule,
  conditionalRuleDraftForRule,
  conditionalRuleLabel,
  isManagedConditionalRule,
} from './spreadsheet-conditional-format-model';

const TOOLBAR_RULES: Array<[conditionName: string, conditionValue: string[], label: string]> = [
  ['textContains', ['Ready'], '包含文本'],
  ['duplicateValue', ['0'], '重复值'],
  ['duplicateValue', ['1'], '唯一值'],
  ['top10', ['5'], '前 5 项'],
  ['top10_percent', ['25'], '前 25%'],
  ['last10', ['4'], '后 4 项'],
  ['last10_percent', ['15'], '后 15%'],
  ['aboveAverage', [''], '高于平均值'],
  ['belowAverage', [''], '低于平均值'],
  ['formula', ['=MOD(ROW(),2)=0'], '公式'],
];

describe('spreadsheet conditional-format model', () => {
  it.each(TOOLBAR_RULES)('preserves and labels the %s toolbar rule', (conditionName, conditionValue, label) => {
    const rule: FortuneConditionalFormatRule = {
      type: 'default',
      cellrange: [{ row: [0, 2], column: [0, 0] }],
      format: { textColor: '#006100', cellColor: '#c6efce' },
      conditionName,
      conditionRange: [{ row: [4, 4], column: [3, 3] }],
      conditionValue,
    };

    expect(isManagedConditionalRule(rule)).toBe(true);
    expect(conditionalRuleLabel(rule)).toBe(label);

    const draft = conditionalRuleDraftForRule('sheet-1', rule);
    draft.reference = 'B2:B4';
    draft.stopIfTrue = true;
    const result = buildConditionalRule(draft);

    expect(result).toEqual({
      rule: {
        ...rule,
        cellrange: [{ row: [1, 3], column: [1, 1] }],
        stopIfTrue: true,
      },
    });
  });
});
