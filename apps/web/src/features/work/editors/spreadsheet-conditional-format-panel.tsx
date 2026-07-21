import type { Sheet } from '@fortune-sheet/core';
import { ArrowDown, ArrowUp, Plus, Trash2 } from 'lucide-react';
import { useEffect, useState } from 'react';
import {
  defaultSpreadsheetConditionalIconThresholds,
  SPREADSHEET_CONDITIONAL_ICON_SETS,
  type SpreadsheetConditionalIconSetName,
} from '../work-spreadsheet-conditional-icons';
import { defaultSpreadsheetColorScaleThresholds } from '../work-spreadsheet-conditional-values';
import { formatSpreadsheetCellRanges } from '../work-spreadsheet-ranges';
import type { FortuneConditionalFormatRule } from '../work-xlsx-conditional-format';
import type { WorkSpreadsheetContent } from '../work-types';
import { SpreadsheetConditionalComparisonFields } from './spreadsheet-conditional-comparison-fields';
import { SpreadsheetConditionalThresholdFields } from './spreadsheet-conditional-threshold-fields';
import {
  buildConditionalRule,
  conditionalRuleDraftForRule,
  conditionalRuleLabel,
  conditionalToolbarRuleSummary,
  conditionalThresholdDrafts,
  isManagedConditionalRule,
  managedConditionalRuleEntries,
  newConditionalRuleDraft,
  sheetConditionalRules,
  withConditionalRules,
  type ConditionalRuleDraft,
  type SpreadsheetConditionalThresholdDraft,
} from './spreadsheet-conditional-format-model';

export { managedConditionalFormatCount } from './spreadsheet-conditional-format-model';

interface SpreadsheetConditionalFormatPanelProps {
  content: WorkSpreadsheetContent;
  onChange: (content: WorkSpreadsheetContent) => void;
}

interface ConditionalRuleLocation {
  sheetId: string;
  index: number;
}

export function SpreadsheetConditionalFormatPanel({ content, onChange }: SpreadsheetConditionalFormatPanelProps) {
  const sheets = content.sheets.filter((sheet): sheet is Sheet & { id: string } => Boolean(sheet.id));
  const activeSheetId = sheets.find((sheet) => sheet.status === 1)?.id ?? sheets[0]?.id ?? '';
  const entries = managedConditionalRuleEntries(sheets);
  const [selection, setSelection] = useState<ConditionalRuleLocation | null>(
    entries[0] ? { sheetId: entries[0].sheet.id, index: entries[0].index } : null
  );
  const [draft, setDraft] = useState<ConditionalRuleDraft>(() =>
    entries[0]
      ? conditionalRuleDraftForRule(entries[0].sheet.id, entries[0].rule)
      : newConditionalRuleDraft(activeSheetId)
  );
  const [error, setError] = useState('');

  useEffect(() => {
    if (!selection) return;
    const sheet = sheets.find((item) => item.id === selection.sheetId);
    const rule = sheet ? sheetConditionalRules(sheet)[selection.index] : undefined;
    if (isManagedConditionalRule(rule)) {
      setDraft(conditionalRuleDraftForRule(selection.sheetId, rule));
      return;
    }
    setSelection(null);
    setDraft(newConditionalRuleDraft(activeSheetId));
  }, [content.sheets, selection?.sheetId, selection?.index]);

  const startNew = () => {
    setSelection(null);
    setDraft(newConditionalRuleDraft(activeSheetId));
    setError('');
  };
  const selectRule = (location: ConditionalRuleLocation, rule: FortuneConditionalFormatRule) => {
    setSelection(location);
    setDraft(conditionalRuleDraftForRule(location.sheetId, rule));
    setError('');
  };
  const setIconSet = (iconSet: SpreadsheetConditionalIconSetName) => {
    setDraft({
      ...draft,
      iconSet,
      iconThresholds: conditionalThresholdDrafts(defaultSpreadsheetConditionalIconThresholds(iconSet)),
    });
  };
  const updateThreshold = (
    field: 'scaleThresholds' | 'barThresholds' | 'iconThresholds',
    index: number,
    patch: Partial<SpreadsheetConditionalThresholdDraft>
  ) => {
    const thresholds = draft[field].map((threshold, thresholdIndex) =>
      thresholdIndex === index ? { ...threshold, ...patch } : threshold
    );
    setDraft({ ...draft, [field]: thresholds });
  };
  const saveRule = () => {
    const result = buildConditionalRule(draft);
    if ('error' in result) {
      setError(result.error);
      return;
    }
    const rule = result.rule;
    let savedIndex = -1;
    const selectedRule = selection;
    const nextSheets = content.sheets.map((sheet) => {
      let rules = sheetConditionalRules(sheet);
      if (selectedRule && selectedRule.sheetId === sheet.id) {
        rules = [...rules];
        if (selectedRule.sheetId === draft.sheetId) {
          rules[selectedRule.index] = rule;
          savedIndex = selectedRule.index;
        } else {
          rules.splice(selectedRule.index, 1);
        }
      }
      if (sheet.id === draft.sheetId && selectedRule?.sheetId !== draft.sheetId) {
        rules = [...rules, rule];
        savedIndex = rules.length - 1;
      }
      return withConditionalRules(sheet, rules);
    });
    if (savedIndex < 0) {
      setError('请选择有效的目标工作表。');
      return;
    }
    onChange({ ...content, sheets: nextSheets });
    setSelection({ sheetId: draft.sheetId, index: savedIndex });
    setError('');
  };
  const deleteRule = () => {
    if (!selection) return;
    const nextSheets = content.sheets.map((sheet) => {
      if (sheet.id !== selection.sheetId) return sheet;
      const rules = sheetConditionalRules(sheet).filter((_, index) => index !== selection.index);
      return withConditionalRules(sheet, rules);
    });
    onChange({ ...content, sheets: nextSheets });
    setSelection(null);
    setDraft(newConditionalRuleDraft(activeSheetId));
    setError('');
  };
  const moveRule = (offset: -1 | 1) => {
    if (!selection) return;
    let movedIndex = selection.index;
    const nextSheets = content.sheets.map((sheet) => {
      if (sheet.id !== selection.sheetId) return sheet;
      const rules = [...sheetConditionalRules(sheet)];
      const target = selection.index + offset;
      if (target < 0 || target >= rules.length) return sheet;
      [rules[selection.index], rules[target]] = [rules[target], rules[selection.index]];
      movedIndex = target;
      return withConditionalRules(sheet, rules);
    });
    if (movedIndex === selection.index) return;
    onChange({ ...content, sheets: nextSheets });
    setSelection({ ...selection, index: movedIndex });
    setError('');
  };

  const selectedSheet = selection ? sheets.find((sheet) => sheet.id === selection.sheetId) : undefined;
  const selectedRuleCount = selectedSheet ? sheetConditionalRules(selectedSheet).length : 0;

  if (!sheets.length) return <p className='work-spreadsheet-conditional-empty'>当前工作簿没有可编辑的工作表。</p>;
  return (
    <div className='work-spreadsheet-conditional-manager'>
      <aside aria-label='Work 条件格式规则'>
        <button type='button' className='create' onClick={startNew}>
          <Plus size={13} />
          新建规则
        </button>
        <div className='work-spreadsheet-conditional-list'>
          {entries.map(({ sheet, rule, index }) => {
            const selected = selection?.sheetId === sheet.id && selection.index === index;
            return (
              <button
                type='button'
                className={selected ? 'active' : ''}
                key={`${sheet.id}-${index}`}
                onClick={() => selectRule({ sheetId: sheet.id, index }, rule)}
              >
                <strong>{conditionalRuleLabel(rule)}</strong>
                <span>{sheet.name}</span>
                <small>{formatSpreadsheetCellRanges(rule.cellrange)}</small>
              </button>
            );
          })}
          {!entries.length && <p>还没有 Work 可管理的条件格式规则。</p>}
        </div>
      </aside>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          saveRule();
        }}
      >
        <label>
          <span>工作表</span>
          <select
            aria-label='条件格式工作表'
            value={draft.sheetId}
            onChange={(event) => setDraft({ ...draft, sheetId: event.target.value })}
          >
            {sheets.map((sheet) => (
              <option value={sheet.id} key={sheet.id}>
                {sheet.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>规则类型</span>
          <select
            aria-label='条件格式规则类型'
            value={draft.type}
            disabled={draft.type === 'toolbarRule'}
            onChange={(event) => setDraft({ ...draft, type: event.target.value as ConditionalRuleDraft['type'] })}
          >
            <option value='cellComparison'>单元格比较</option>
            <option value='colorGradation'>色阶</option>
            <option value='dataBar'>数据条</option>
            <option value='icons'>图标集</option>
            {draft.type === 'toolbarRule' && <option value='toolbarRule'>工具栏规则</option>}
          </select>
        </label>
        <label className='reference'>
          <span>应用范围</span>
          <input
            aria-label='条件格式范围'
            value={draft.reference}
            placeholder='A2:A20'
            onChange={(event) => setDraft({ ...draft, reference: event.target.value })}
          />
        </label>
        <label className='toggle'>
          <input
            type='checkbox'
            aria-label='匹配后停止后续规则'
            checked={draft.stopIfTrue}
            onChange={(event) => setDraft({ ...draft, stopIfTrue: event.target.checked })}
          />
          <span>匹配后停止后续规则</span>
        </label>
        {draft.type === 'toolbarRule' ? (
          <label className='reference'>
            <span>规则摘要</span>
            <input aria-label='条件格式规则摘要' readOnly value={conditionalToolbarRuleSummary(draft.preservedRule)} />
          </label>
        ) : draft.type === 'cellComparison' ? (
          <SpreadsheetConditionalComparisonFields
            draft={draft}
            onChange={(patch) => setDraft({ ...draft, ...patch })}
          />
        ) : draft.type === 'colorGradation' ? (
          <>
            <label>
              <span>色阶级数</span>
              <select
                aria-label='色阶级数'
                value={draft.scaleSize}
                onChange={(event) =>
                  setDraft({
                    ...draft,
                    scaleSize: event.target.value as ConditionalRuleDraft['scaleSize'],
                    scaleThresholds: conditionalThresholdDrafts(
                      defaultSpreadsheetColorScaleThresholds(Number(event.target.value))
                    ),
                  })
                }
              >
                <option value='2'>双色阶</option>
                <option value='3'>三色阶</option>
              </select>
            </label>
            <ColorField
              label='最小值颜色'
              value={draft.minimumColor}
              onChange={(minimumColor) => setDraft({ ...draft, minimumColor })}
            />
            {draft.scaleSize === '3' && (
              <ColorField
                label='中间值颜色'
                value={draft.midpointColor}
                onChange={(midpointColor) => setDraft({ ...draft, midpointColor })}
              />
            )}
            <ColorField
              label='最大值颜色'
              value={draft.maximumColor}
              onChange={(maximumColor) => setDraft({ ...draft, maximumColor })}
            />
            <SpreadsheetConditionalThresholdFields
              label='色阶'
              thresholds={draft.scaleThresholds}
              onChange={(index, patch) => updateThreshold('scaleThresholds', index, patch)}
            />
          </>
        ) : draft.type === 'dataBar' ? (
          <>
            <ColorField
              label='数据条颜色'
              value={draft.barColor}
              onChange={(barColor) => setDraft({ ...draft, barColor })}
            />
            <label className='toggle'>
              <input
                type='checkbox'
                checked={draft.barShowValue}
                onChange={(event) => setDraft({ ...draft, barShowValue: event.target.checked })}
              />
              <span>显示数据条数值</span>
            </label>
            <label>
              <span>最短长度（%）</span>
              <input
                type='number'
                min='0'
                max='100'
                aria-label='数据条最短长度'
                value={draft.barMinLength}
                onChange={(event) => setDraft({ ...draft, barMinLength: event.target.value })}
              />
            </label>
            <label>
              <span>最长长度（%）</span>
              <input
                type='number'
                min='0'
                max='100'
                aria-label='数据条最长长度'
                value={draft.barMaxLength}
                onChange={(event) => setDraft({ ...draft, barMaxLength: event.target.value })}
              />
            </label>
            <SpreadsheetConditionalThresholdFields
              label='数据条'
              thresholds={draft.barThresholds}
              onChange={(index, patch) => updateThreshold('barThresholds', index, patch)}
            />
          </>
        ) : (
          <>
            <label>
              <span>图标集</span>
              <select
                aria-label='图标集'
                value={draft.iconSet}
                onChange={(event) => setIconSet(event.target.value as SpreadsheetConditionalIconSetName)}
              >
                {SPREADSHEET_CONDITIONAL_ICON_SETS.map((iconSet) => (
                  <option value={iconSet.name} key={iconSet.name}>
                    {iconSet.label}
                  </option>
                ))}
              </select>
            </label>
            <label className='toggle'>
              <input
                type='checkbox'
                checked={draft.iconReverse}
                onChange={(event) => setDraft({ ...draft, iconReverse: event.target.checked })}
              />
              <span>反转图标顺序</span>
            </label>
            <label className='toggle'>
              <input
                type='checkbox'
                checked={draft.iconShowValue}
                onChange={(event) => setDraft({ ...draft, iconShowValue: event.target.checked })}
              />
              <span>显示单元格值</span>
            </label>
            <SpreadsheetConditionalThresholdFields
              label='图标'
              thresholds={draft.iconThresholds}
              startIndex={1}
              showEquality
              onChange={(index, patch) => updateThreshold('iconThresholds', index, patch)}
            />
          </>
        )}
        <p>文本、重复值、排名、平均值和公式规则的条件与样式由表格工具栏维护；此处可统一修改范围、停止行为和优先级。</p>
        <div className='actions'>
          {error && <output className='error'>{error}</output>}
          <button
            type='button'
            disabled={!selection || selection.index <= 0}
            aria-label='提高优先级'
            onClick={() => moveRule(-1)}
          >
            <ArrowUp size={13} />
            提高优先级
          </button>
          <button
            type='button'
            disabled={!selection || selection.index >= selectedRuleCount - 1}
            aria-label='降低优先级'
            onClick={() => moveRule(1)}
          >
            <ArrowDown size={13} />
            降低优先级
          </button>
          <button type='button' className='danger' disabled={!selection} onClick={deleteRule}>
            <Trash2 size={13} />
            删除规则
          </button>
          <button type='submit' className='primary'>
            保存规则
          </button>
        </div>
      </form>
    </div>
  );
}

function ColorField({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  return (
    <label className='color'>
      <span>{label}</span>
      <input type='color' aria-label={label} value={value} onChange={(event) => onChange(event.target.value)} />
    </label>
  );
}
