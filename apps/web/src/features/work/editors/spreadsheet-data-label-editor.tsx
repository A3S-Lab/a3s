import {
  normalizeWorkSpreadsheetDataLabelPosition,
  type WorkSpreadsheetChartType,
  type WorkSpreadsheetDataLabelPosition,
  type WorkSpreadsheetDataLabels,
} from '../work-types';

interface SpreadsheetDataLabelEditorProps {
  chartType: WorkSpreadsheetChartType;
  seriesNumber: number;
  value?: WorkSpreadsheetDataLabels;
  onChange: (value: WorkSpreadsheetDataLabels | undefined) => void;
}

export function SpreadsheetDataLabelEditor({
  chartType,
  seriesNumber,
  value,
  onChange,
}: SpreadsheetDataLabelEditorProps) {
  const labelPrefix = `系列 ${seriesNumber} 数据标签`;
  const change = (update: Partial<WorkSpreadsheetDataLabels>) => onChange({ ...value, ...update });

  return (
    <section className='work-spreadsheet-data-labels' aria-label={labelPrefix}>
      <label className='check enable-data-labels'>
        <input
          type='checkbox'
          aria-label={`系列 ${seriesNumber} 显示数据标签`}
          checked={Boolean(value)}
          onChange={(event) => onChange(event.target.checked ? { showValue: true } : undefined)}
        />
        <span>显示数据标签</span>
      </label>
      {value && (
        <div>
          <label className='check'>
            <input
              type='checkbox'
              aria-label={`${labelPrefix}显示数值`}
              checked={value.showValue === true}
              onChange={(event) => change({ showValue: event.target.checked })}
            />
            <span>数值</span>
          </label>
          <label className='check'>
            <input
              type='checkbox'
              aria-label={`${labelPrefix}显示分类名称`}
              checked={value.showCategoryName === true}
              onChange={(event) => change({ showCategoryName: event.target.checked })}
            />
            <span>分类名称</span>
          </label>
          <label className='check'>
            <input
              type='checkbox'
              aria-label={`${labelPrefix}显示系列名称`}
              checked={value.showSeriesName === true}
              onChange={(event) => change({ showSeriesName: event.target.checked })}
            />
            <span>系列名称</span>
          </label>
          {(chartType === 'pie' || chartType === 'doughnut') && (
            <label className='check'>
              <input
                type='checkbox'
                aria-label={`${labelPrefix}显示百分比`}
                checked={value.showPercentage === true}
                onChange={(event) => change({ showPercentage: event.target.checked })}
              />
              <span>百分比</span>
            </label>
          )}
          {chartType === 'bubble' && (
            <label className='check'>
              <input
                type='checkbox'
                aria-label={`${labelPrefix}显示气泡大小`}
                checked={value.showBubbleSize === true}
                onChange={(event) => change({ showBubbleSize: event.target.checked })}
              />
              <span>气泡大小</span>
            </label>
          )}
          <label className='data-label-position'>
            <span>位置</span>
            <select
              aria-label={`${labelPrefix}位置`}
              value={normalizeWorkSpreadsheetDataLabelPosition(value.position)}
              onChange={(event) => change({ position: event.target.value as WorkSpreadsheetDataLabelPosition })}
            >
              <option value='bestFit'>最佳匹配</option>
              <option value='center'>居中</option>
              <option value='insideBase'>内侧基部</option>
              <option value='insideEnd'>内侧末端</option>
              <option value='outsideEnd'>外侧末端</option>
              <option value='left'>左侧</option>
              <option value='right'>右侧</option>
              <option value='above'>上方</option>
              <option value='below'>下方</option>
            </select>
          </label>
          <label className='data-label-separator'>
            <span>分隔符</span>
            <input
              aria-label={`${labelPrefix}分隔符`}
              value={value.separator ?? ', '}
              maxLength={64}
              onChange={(event) => change({ separator: event.target.value })}
            />
          </label>
        </div>
      )}
    </section>
  );
}
