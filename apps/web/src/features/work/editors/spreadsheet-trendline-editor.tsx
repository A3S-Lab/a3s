import { Plus, Trash2 } from 'lucide-react';
import type { WorkSpreadsheetTrendline, WorkSpreadsheetTrendlineType } from '../work-types';

interface SpreadsheetTrendlineEditorProps {
  seriesNumber: number;
  trendlines: WorkSpreadsheetTrendline[];
  onChange: (trendlines: WorkSpreadsheetTrendline[]) => void;
}

export function SpreadsheetTrendlineEditor({ seriesNumber, trendlines, onChange }: SpreadsheetTrendlineEditorProps) {
  const replaceTrendline = (index: number, change: Partial<WorkSpreadsheetTrendline>) => {
    onChange(trendlines.map((trendline, candidate) => (candidate === index ? { ...trendline, ...change } : trendline)));
  };

  return (
    <section className='work-spreadsheet-trendlines' aria-label={`系列 ${seriesNumber} 趋势线`}>
      <header>
        <strong>趋势线</strong>
        <button
          type='button'
          aria-label={`添加系列 ${seriesNumber} 趋势线`}
          onClick={() => onChange([...trendlines, { type: 'linear' }])}
        >
          <Plus size={11} />
          添加趋势线
        </button>
      </header>
      {!trendlines.length && <p>可为同一系列叠加线性、多项式或移动平均等趋势分析。</p>}
      {trendlines.map((trendline, index) => {
        const trendlineNumber = index + 1;
        const labelPrefix = `系列 ${seriesNumber} 趋势线 ${trendlineNumber}`;
        const hasIntercept = trendline.intercept !== undefined;
        return (
          <fieldset key={`${seriesNumber}-${trendlineNumber}`}>
            <legend>趋势线 {trendlineNumber}</legend>
            <label>
              <span>类型</span>
              <select
                aria-label={`${labelPrefix} 类型`}
                value={trendline.type}
                onChange={(event) =>
                  onChange(
                    trendlines.map((item, candidate) =>
                      candidate === index
                        ? trendlineWithType(item, event.target.value as WorkSpreadsheetTrendlineType)
                        : item
                    )
                  )
                }
              >
                <option value='linear'>线性</option>
                <option value='exponential'>指数</option>
                <option value='logarithmic'>对数</option>
                <option value='polynomial'>多项式</option>
                <option value='power'>幂</option>
                <option value='movingAverage'>移动平均</option>
              </select>
            </label>
            <label>
              <span>名称</span>
              <input
                aria-label={`${labelPrefix} 名称`}
                value={trendline.name ?? ''}
                maxLength={255}
                placeholder={`趋势线 ${trendlineNumber}`}
                onChange={(event) => replaceTrendline(index, { name: event.target.value })}
              />
            </label>
            {trendline.type === 'polynomial' && (
              <label>
                <span>阶数</span>
                <input
                  type='number'
                  aria-label={`${labelPrefix} 阶数`}
                  min={2}
                  max={6}
                  step={1}
                  value={trendline.order ?? 2}
                  onChange={(event) => replaceTrendline(index, { order: optionalNumber(event.target.value) })}
                />
              </label>
            )}
            {trendline.type === 'movingAverage' && (
              <label>
                <span>周期</span>
                <input
                  type='number'
                  aria-label={`${labelPrefix} 周期`}
                  min={2}
                  max={255}
                  step={1}
                  value={trendline.period ?? 2}
                  onChange={(event) => replaceTrendline(index, { period: optionalNumber(event.target.value) })}
                />
              </label>
            )}
            <label>
              <span>前推</span>
              <input
                type='number'
                aria-label={`${labelPrefix} 前推`}
                min={0}
                step='any'
                value={trendline.forward ?? 0}
                onChange={(event) => replaceTrendline(index, { forward: optionalNumber(event.target.value) })}
              />
            </label>
            <label>
              <span>后推</span>
              <input
                type='number'
                aria-label={`${labelPrefix} 后推`}
                min={0}
                step='any'
                value={trendline.backward ?? 0}
                onChange={(event) => replaceTrendline(index, { backward: optionalNumber(event.target.value) })}
              />
            </label>
            <label className='check intercept-toggle'>
              <input
                type='checkbox'
                aria-label={`${labelPrefix} 固定截距`}
                checked={hasIntercept}
                onChange={(event) => replaceTrendline(index, { intercept: event.target.checked ? 0 : undefined })}
              />
              <span>固定截距</span>
            </label>
            <label>
              <span>截距</span>
              <input
                type='number'
                aria-label={`${labelPrefix} 截距`}
                step='any'
                disabled={!hasIntercept}
                value={trendline.intercept ?? 0}
                onChange={(event) => replaceTrendline(index, { intercept: optionalNumber(event.target.value) })}
              />
            </label>
            <label className='check'>
              <input
                type='checkbox'
                aria-label={`${labelPrefix} 显示公式`}
                checked={trendline.displayEquation === true}
                onChange={(event) => replaceTrendline(index, { displayEquation: event.target.checked })}
              />
              <span>显示公式</span>
            </label>
            <label className='check'>
              <input
                type='checkbox'
                aria-label={`${labelPrefix} 显示 R 方`}
                checked={trendline.displayRSquared === true}
                onChange={(event) => replaceTrendline(index, { displayRSquared: event.target.checked })}
              />
              <span>显示 R²</span>
            </label>
            <button
              type='button'
              className='remove-trendline'
              aria-label={`删除${labelPrefix}`}
              onClick={() => onChange(trendlines.filter((_, candidate) => candidate !== index))}
            >
              <Trash2 size={12} />
            </button>
          </fieldset>
        );
      })}
    </section>
  );
}

function trendlineWithType(
  trendline: WorkSpreadsheetTrendline,
  type: WorkSpreadsheetTrendlineType
): WorkSpreadsheetTrendline {
  const next = { ...trendline, type, order: undefined, period: undefined };
  if (type === 'polynomial') return { ...next, order: trendline.order ?? 2 };
  if (type === 'movingAverage') return { ...next, period: trendline.period ?? 2, intercept: undefined };
  return next;
}

function optionalNumber(value: string): number | undefined {
  return value === '' ? undefined : Number(value);
}
