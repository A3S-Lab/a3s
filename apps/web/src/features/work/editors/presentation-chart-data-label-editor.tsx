import {
  normalizePresentationChartDataLabelPosition,
  presentationChartDataLabelPositionLabel,
  presentationChartDataLabelPositions,
} from '../work-presentation-charts';
import type { WorkSlideChartDataLabelPosition, WorkSlideChartDataLabels, WorkSlideChartType } from '../work-types';

export function PresentationChartDataLabelEditor({
  chartType,
  value,
  onChange,
}: {
  chartType: WorkSlideChartType;
  value?: WorkSlideChartDataLabels;
  onChange: (value: WorkSlideChartDataLabels | undefined) => void;
}) {
  const change = (patch: Partial<WorkSlideChartDataLabels>) => onChange({ ...value, ...patch });
  return (
    <section className='work-presentation-chart-data-labels' aria-label='演示图表数据标签'>
      <label className='check enable-data-labels'>
        <input
          type='checkbox'
          aria-label='显示演示图表数据标签'
          checked={value !== undefined}
          onChange={(event) => onChange(event.target.checked ? { showValue: true } : undefined)}
        />
        <span>数据标签</span>
      </label>
      {value && (
        <div>
          <label className='check'>
            <input
              type='checkbox'
              aria-label='演示图表数据标签显示数值'
              checked={value.showValue === true}
              onChange={(event) => change({ showValue: event.target.checked })}
            />
            <span>数值</span>
          </label>
          <label className='check'>
            <input
              type='checkbox'
              aria-label='演示图表数据标签显示分类名称'
              checked={value.showCategoryName === true}
              onChange={(event) => change({ showCategoryName: event.target.checked })}
            />
            <span>分类</span>
          </label>
          <label className='check'>
            <input
              type='checkbox'
              aria-label='演示图表数据标签显示系列名称'
              checked={value.showSeriesName === true}
              onChange={(event) => change({ showSeriesName: event.target.checked })}
            />
            <span>系列</span>
          </label>
          {(chartType === 'pie' || chartType === 'doughnut') && (
            <label className='check'>
              <input
                type='checkbox'
                aria-label='演示图表数据标签显示百分比'
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
                aria-label='演示图表数据标签显示气泡大小'
                checked={value.showBubbleSize === true}
                onChange={(event) => change({ showBubbleSize: event.target.checked })}
              />
              <span>气泡大小</span>
            </label>
          )}
          <label>
            <span>位置</span>
            <select
              aria-label='演示图表数据标签位置'
              value={normalizePresentationChartDataLabelPosition(value.position, chartType)}
              onChange={(event) => change({ position: event.target.value as WorkSlideChartDataLabelPosition })}
            >
              {presentationChartDataLabelPositions(chartType).map((position) => (
                <option value={position} key={position}>
                  {presentationChartDataLabelPositionLabel(position)}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>分隔符</span>
            <input
              aria-label='演示图表数据标签分隔符'
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
