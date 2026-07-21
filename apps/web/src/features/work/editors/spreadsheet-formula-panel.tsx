import { Calculator, RefreshCw, Save } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { spreadsheetFormulaAnalysis } from '../work-spreadsheet-formula-analysis';
import { effectiveSpreadsheetCalculationSettings } from '../work-spreadsheet-formulas';
import type { WorkSpreadsheetCalculationSettings, WorkSpreadsheetContent } from '../work-types';

interface SpreadsheetFormulaPanelProps {
  content: WorkSpreadsheetContent;
  canRecalculateSelection: boolean;
  onChange: (content: WorkSpreadsheetContent) => void;
  onRecalculate: (scope: 'workbook' | 'selection') => boolean;
}

export function SpreadsheetFormulaPanel({
  content,
  canRecalculateSelection,
  onChange,
  onRecalculate,
}: SpreadsheetFormulaPanelProps) {
  const [settings, setSettings] = useState(() => effectiveSpreadsheetCalculationSettings(content.calculation));
  const [status, setStatus] = useState('');
  const { summary, diagnostics } = useMemo(() => spreadsheetFormulaAnalysis(content), [content]);

  useEffect(() => {
    setSettings(effectiveSpreadsheetCalculationSettings(content.calculation));
  }, [content.calculation]);

  const update = <Key extends keyof WorkSpreadsheetCalculationSettings>(
    key: Key,
    value: WorkSpreadsheetCalculationSettings[Key]
  ) => {
    setSettings((current) => ({ ...current, [key]: value }));
    setStatus('');
  };
  const save = () => {
    onChange({
      ...content,
      calculation: effectiveSpreadsheetCalculationSettings(settings),
    });
    setStatus('计算设置已保存。');
  };
  const recalculate = (scope: 'workbook' | 'selection') => {
    const started = onRecalculate(scope);
    setStatus(
      started ? (scope === 'workbook' ? '已重新计算工作簿。' : '已重新计算当前选区。') : '表格尚未准备好，请稍后重试。'
    );
  };

  return (
    <div className='work-spreadsheet-formula-manager'>
      <aside aria-label='公式统计'>
        <FormulaStat label='公式单元格' value={summary.formulaCells} />
        <FormulaStat
          label='缓存错误'
          value={summary.cachedErrorCells}
          tone={summary.cachedErrorCells ? 'error' : undefined}
        />
        <FormulaStat label='传统数组' value={summary.arrayRanges} />
        <FormulaStat label='动态数组' value={summary.dynamicArrayRanges} />
        <FormulaStat label='模拟运算表' value={summary.dataTableRanges} />
        <FormulaStat
          label='兼容性问题'
          value={diagnostics.filter((item) => item.severity !== 'info').length}
          tone={diagnostics.some((item) => item.severity === 'error') ? 'error' : undefined}
        />
      </aside>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          save();
        }}
      >
        <div className='work-spreadsheet-calculation-fields'>
          <label>
            <span>计算模式</span>
            <select
              aria-label='计算模式'
              value={settings.mode}
              onChange={(event) => update('mode', event.target.value as WorkSpreadsheetCalculationSettings['mode'])}
            >
              <option value='automatic'>自动</option>
              <option value='automatic-except-data-tables'>自动（模拟运算表除外）</option>
              <option value='manual'>手动</option>
            </select>
          </label>
          <label>
            <span>最大迭代次数</span>
            <input
              aria-label='最大迭代次数'
              type='number'
              min={1}
              max={10_000}
              step={1}
              disabled={!settings.iterativeCalculation}
              value={settings.maximumIterations}
              onChange={(event) => update('maximumIterations', Number(event.target.value))}
            />
          </label>
          <label>
            <span>最大更改值</span>
            <input
              aria-label='最大更改值'
              type='number'
              min='0.000000000001'
              step='any'
              disabled={!settings.iterativeCalculation}
              value={settings.maximumChange}
              onChange={(event) => update('maximumChange', Number(event.target.value))}
            />
          </label>
          <label className='toggle'>
            <input
              aria-label='打开工作簿时完整重算'
              type='checkbox'
              checked={settings.fullCalculationOnLoad}
              onChange={(event) => update('fullCalculationOnLoad', event.target.checked)}
            />
            打开工作簿时完整重算
          </label>
          <label className='toggle'>
            <input
              aria-label='强制完整计算'
              type='checkbox'
              checked={settings.forceFullCalculation}
              onChange={(event) => update('forceFullCalculation', event.target.checked)}
            />
            强制完整计算
          </label>
          <label className='toggle'>
            <input
              aria-label='使用迭代计算'
              type='checkbox'
              checked={settings.iterativeCalculation}
              onChange={(event) => update('iterativeCalculation', event.target.checked)}
            />
            使用迭代计算
          </label>
          <label className='toggle'>
            <input
              aria-label='使用完整精度'
              type='checkbox'
              checked={settings.fullPrecision}
              onChange={(event) => update('fullPrecision', event.target.checked)}
            />
            使用完整精度
          </label>
          <div className='actions'>
            {status && <span className='status'>{status}</span>}
            <button type='button' disabled={!canRecalculateSelection} onClick={() => recalculate('selection')}>
              <RefreshCw size={12} />
              重新计算当前选区
            </button>
            <button type='button' onClick={() => recalculate('workbook')}>
              <Calculator size={12} />
              重新计算工作簿
            </button>
            <button type='submit' className='primary'>
              <Save size={12} />
              保存计算设置
            </button>
          </div>
        </div>
        <section className='work-spreadsheet-formula-diagnostics' aria-label='公式兼容性诊断'>
          <header>
            <strong>公式兼容性诊断</strong>
            <span>{diagnostics.length ? `${diagnostics.length} 项` : '未发现问题'}</span>
          </header>
          <div>
            {diagnostics.map((diagnostic) => (
              <article className={diagnostic.severity} key={diagnostic.code}>
                <strong>{diagnostic.title}</strong>
                <p>{diagnostic.message}</p>
                {diagnostic.locations.length > 0 && (
                  <small>
                    {diagnostic.locations.slice(0, 4).join('、')}
                    {diagnostic.locations.length > 4 ? ` 等 ${diagnostic.locations.length} 处` : ''}
                  </small>
                )}
              </article>
            ))}
            {!diagnostics.length && <p className='empty'>当前公式可由 Work 直接计算，也没有缓存错误或分组冲突。</p>}
          </div>
        </section>
      </form>
    </div>
  );
}

function FormulaStat({ label, value, tone }: { label: string; value: number; tone?: 'error' }) {
  return (
    <div className={tone}>
      <strong>{value}</strong>
      <span>{label}</span>
    </div>
  );
}
