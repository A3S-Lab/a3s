import { Plus, Trash2 } from 'lucide-react';
import {
  type WorkSpreadsheetChartType,
  type WorkSpreadsheetErrorBarDirection,
  type WorkSpreadsheetErrorBars,
  type WorkSpreadsheetErrorBarType,
  type WorkSpreadsheetErrorBarValueType,
  workSpreadsheetChartUsesNumericXAxis,
} from '../work-types';

interface SpreadsheetErrorBarEditorProps {
  chartType: WorkSpreadsheetChartType;
  seriesNumber: number;
  errorBars: WorkSpreadsheetErrorBars[];
  onChange: (errorBars: WorkSpreadsheetErrorBars[]) => void;
  customInput?: 'references' | 'values';
}

export function SpreadsheetErrorBarEditor({
  chartType,
  seriesNumber,
  errorBars,
  onChange,
  customInput = 'references',
}: SpreadsheetErrorBarEditorProps) {
  const replaceErrorBars = (index: number, change: Partial<WorkSpreadsheetErrorBars>) => {
    onChange(errorBars.map((item, candidate) => (candidate === index ? { ...item, ...change } : item)));
  };
  const hasDirection = (direction: WorkSpreadsheetErrorBarDirection) =>
    errorBars.some((item) => item.direction === direction);
  const addErrorBars = (direction: WorkSpreadsheetErrorBarDirection) =>
    onChange([...errorBars, { direction, barType: 'both', valueType: 'standardError' }]);

  return (
    <section className='work-spreadsheet-error-bars' aria-label={`系列 ${seriesNumber} 误差线`}>
      <header>
        <strong>误差线</strong>
        <div>
          {workSpreadsheetChartUsesNumericXAxis(chartType) && (
            <button
              type='button'
              aria-label={`添加系列 ${seriesNumber} X 误差线`}
              disabled={hasDirection('x')}
              onClick={() => addErrorBars('x')}
            >
              <Plus size={11} />X
            </button>
          )}
          <button
            type='button'
            aria-label={`添加系列 ${seriesNumber} Y 误差线`}
            disabled={hasDirection('y')}
            onClick={() => addErrorBars('y')}
          >
            <Plus size={11} />Y
          </button>
        </div>
      </header>
      {!errorBars.length && <p>添加固定值、百分比、统计或自定义范围误差线。</p>}
      {errorBars.map((item, index) => {
        const errorBarNumber = index + 1;
        const labelPrefix = `系列 ${seriesNumber} 误差线 ${errorBarNumber}`;
        return (
          <fieldset key={`${seriesNumber}-${errorBarNumber}`}>
            <legend>
              {item.direction.toUpperCase()} 误差线 {errorBarNumber}
            </legend>
            <label>
              <span>方向</span>
              <select
                aria-label={`${labelPrefix} 方向`}
                value={item.direction}
                onChange={(event) =>
                  replaceErrorBars(index, { direction: event.target.value as WorkSpreadsheetErrorBarDirection })
                }
              >
                {workSpreadsheetChartUsesNumericXAxis(chartType) && <option value='x'>X</option>}
                <option value='y'>Y</option>
              </select>
            </label>
            <label>
              <span>误差类型</span>
              <select
                aria-label={`${labelPrefix} 误差类型`}
                value={item.barType}
                onChange={(event) =>
                  replaceErrorBars(index, { barType: event.target.value as WorkSpreadsheetErrorBarType })
                }
              >
                <option value='both'>双向</option>
                <option value='plus'>正向</option>
                <option value='minus'>负向</option>
              </select>
            </label>
            <label>
              <span>计算方式</span>
              <select
                aria-label={`${labelPrefix} 计算方式`}
                value={item.valueType}
                onChange={(event) =>
                  replaceErrorBars(
                    index,
                    errorBarsWithValueType(item, event.target.value as WorkSpreadsheetErrorBarValueType)
                  )
                }
              >
                <option value='fixedValue'>固定值</option>
                <option value='percentage'>百分比</option>
                <option value='standardDeviation'>标准差</option>
                <option value='standardError'>标准误差</option>
                <option value='custom'>自定义范围</option>
              </select>
            </label>
            {(item.valueType === 'fixedValue' ||
              item.valueType === 'percentage' ||
              item.valueType === 'standardDeviation') && (
              <label>
                <span>{item.valueType === 'percentage' ? '百分比（%）' : '数值'}</span>
                <input
                  type='number'
                  aria-label={`${labelPrefix} 数值`}
                  min={0}
                  step='any'
                  value={item.value ?? (item.valueType === 'percentage' ? 5 : 1)}
                  onChange={(event) => replaceErrorBars(index, { value: optionalNumber(event.target.value) })}
                />
              </label>
            )}
            {item.valueType === 'custom' && item.barType !== 'minus' && customInput === 'references' && (
              <label className='error-reference'>
                <span>正误差引用</span>
                <input
                  aria-label={`${labelPrefix} 正误差引用`}
                  value={item.plusReference ?? ''}
                  placeholder="'报告'!$C$2:$C$8"
                  onChange={(event) => replaceErrorBars(index, { plusReference: event.target.value })}
                />
              </label>
            )}
            {item.valueType === 'custom' && item.barType !== 'plus' && customInput === 'references' && (
              <label className='error-reference'>
                <span>负误差引用</span>
                <input
                  aria-label={`${labelPrefix} 负误差引用`}
                  value={item.minusReference ?? ''}
                  placeholder="'报告'!$D$2:$D$8"
                  onChange={(event) => replaceErrorBars(index, { minusReference: event.target.value })}
                />
              </label>
            )}
            {item.valueType === 'custom' && item.barType !== 'minus' && customInput === 'values' && (
              <label className='error-reference'>
                <span>正误差值</span>
                <input
                  aria-label={`${labelPrefix} 正误差值`}
                  value={item.plusValues?.join(', ') ?? ''}
                  placeholder={item.plusReference ? '已保留导入引用；输入数值可替换' : '1, 2, 1.5'}
                  onChange={(event) =>
                    replaceErrorBars(index, {
                      plusValues: parseCustomValues(event.target.value),
                      plusReference: undefined,
                    })
                  }
                />
                {item.plusReference && <small title={item.plusReference}>已保留导入引用：{item.plusReference}</small>}
              </label>
            )}
            {item.valueType === 'custom' && item.barType !== 'plus' && customInput === 'values' && (
              <label className='error-reference'>
                <span>负误差值</span>
                <input
                  aria-label={`${labelPrefix} 负误差值`}
                  value={item.minusValues?.join(', ') ?? ''}
                  placeholder={item.minusReference ? '已保留导入引用；输入数值可替换' : '1, 2, 1.5'}
                  onChange={(event) =>
                    replaceErrorBars(index, {
                      minusValues: parseCustomValues(event.target.value),
                      minusReference: undefined,
                    })
                  }
                />
                {item.minusReference && (
                  <small title={item.minusReference}>已保留导入引用：{item.minusReference}</small>
                )}
              </label>
            )}
            <label className='check'>
              <input
                type='checkbox'
                aria-label={`${labelPrefix} 显示端帽`}
                checked={item.showEndCaps !== false}
                onChange={(event) => replaceErrorBars(index, { showEndCaps: event.target.checked })}
              />
              <span>显示端帽</span>
            </label>
            <button
              type='button'
              className='remove-error-bars'
              aria-label={`删除${labelPrefix}`}
              onClick={() => onChange(errorBars.filter((_, candidate) => candidate !== index))}
            >
              <Trash2 size={12} />
            </button>
          </fieldset>
        );
      })}
    </section>
  );
}

function errorBarsWithValueType(
  errorBars: WorkSpreadsheetErrorBars,
  valueType: WorkSpreadsheetErrorBarValueType
): Partial<WorkSpreadsheetErrorBars> {
  if (valueType === 'standardError' || valueType === 'custom') return { valueType, value: undefined };
  return { valueType, value: errorBars.value ?? (valueType === 'percentage' ? 5 : 1) };
}

function optionalNumber(value: string): number | undefined {
  return value === '' ? undefined : Number(value);
}

function parseCustomValues(value: string): number[] | undefined {
  const values = value
    .split(/[\s,;，；]+/)
    .map((item) => item.trim())
    .filter(Boolean)
    .slice(0, 256)
    .map(Number)
    .filter(Number.isFinite)
    .map((item) => Math.max(0, item));
  return values.length ? values : undefined;
}
