import { presentationChartAxes, withPresentationChartAxes } from '../work-presentation-chart-axes';
import {
  workSpreadsheetChartAxisDefaultLabelPosition,
  workSpreadsheetChartAxisIsCategoryAxis,
  workSpreadsheetChartAxisIsValueAxis,
  workSpreadsheetChartAxisShowsMajorGridlinesByDefault,
} from '../work-spreadsheet-chart-axis';
import type { WorkSlideChart, WorkSlideChartAxis, WorkSpreadsheetChartAxisPosition } from '../work-types';

const PRIMARY_AXES: Array<{ position: WorkSpreadsheetChartAxisPosition; label: string }> = [
  { position: 'bottom', label: '横轴' },
  { position: 'left', label: '纵轴' },
];

export function PresentationChartAxisEditor({
  chart,
  onChange,
}: {
  chart: WorkSlideChart;
  onChange: (chart: WorkSlideChart) => void;
}) {
  const axes = presentationChartAxes(chart);
  const updateAxis = (position: WorkSpreadsheetChartAxisPosition, changes: Partial<WorkSlideChartAxis>) => {
    onChange(
      withPresentationChartAxes(chart, {
        ...axes,
        [position]: { ...axes?.[position], ...changes },
      })
    );
  };

  return (
    <section className='work-spreadsheet-chart-axes work-presentation-chart-axes' aria-label='演示图表坐标轴设置'>
      <header>
        <strong>坐标轴</strong>
        <span>标题、范围、刻度与标签</span>
      </header>
      <div>
        {PRIMARY_AXES.map(({ position, label }) => {
          const axis = axes?.[position];
          const valueAxis = workSpreadsheetChartAxisIsValueAxis(chart.type, position);
          const categoryAxis = workSpreadsheetChartAxisIsCategoryAxis(chart.type, position);
          const labelPosition =
            axis?.labelPosition ?? workSpreadsheetChartAxisDefaultLabelPosition(chart.type, position);
          const showMajorGridlines =
            axis?.showMajorGridlines ?? workSpreadsheetChartAxisShowsMajorGridlinesByDefault(chart.type, position);
          const sourceLinked = axis?.numberFormatSourceLinked ?? !axis?.numberFormat;
          const ariaName = `演示图表${label}`;
          return (
            <fieldset key={position}>
              <legend>{label}</legend>
              <label>
                <span>标题</span>
                <input
                  aria-label={`${ariaName}标题`}
                  value={axis?.title ?? ''}
                  maxLength={255}
                  onChange={(event) => updateAxis(position, { title: event.target.value })}
                />
              </label>
              <label className='axis-check'>
                <input
                  type='checkbox'
                  aria-label={`${ariaName}逆序`}
                  checked={axis?.reverseOrder === true}
                  onChange={(event) => updateAxis(position, { reverseOrder: event.target.checked })}
                />
                <span>逆序</span>
              </label>
              <label>
                <span>标签位置</span>
                <select
                  aria-label={`${ariaName}标签位置`}
                  value={labelPosition}
                  onChange={(event) =>
                    updateAxis(position, {
                      labelPosition: event.target.value as NonNullable<WorkSlideChartAxis['labelPosition']>,
                    })
                  }
                >
                  <option value='nextTo'>轴旁</option>
                  <option value='high'>高位</option>
                  <option value='low'>低位</option>
                  <option value='none'>不显示</option>
                </select>
              </label>
              <label>
                <span>主要刻度线</span>
                <select
                  aria-label={`${ariaName}主要刻度线`}
                  value={axis?.majorTickMark ?? 'none'}
                  onChange={(event) =>
                    updateAxis(position, {
                      majorTickMark: event.target.value as NonNullable<WorkSlideChartAxis['majorTickMark']>,
                    })
                  }
                >
                  <option value='none'>无</option>
                  <option value='inside'>向内</option>
                  <option value='outside'>向外</option>
                  <option value='cross'>交叉</option>
                </select>
              </label>
              {categoryAxis && (
                <label>
                  <span>标签间隔</span>
                  <input
                    type='number'
                    min={1}
                    max={31_999}
                    step={1}
                    aria-label={`${ariaName}标签间隔`}
                    value={axis?.labelInterval ?? ''}
                    placeholder='自动'
                    onChange={(event) => updateAxis(position, { labelInterval: optionalInteger(event.target.value) })}
                  />
                </label>
              )}
              {valueAxis && (
                <>
                  <label>
                    <span>最小值</span>
                    <input
                      type='number'
                      step='any'
                      aria-label={`${ariaName}最小值`}
                      value={axis?.minimum ?? ''}
                      placeholder='自动'
                      onChange={(event) => updateAxis(position, { minimum: optionalNumber(event.target.value) })}
                    />
                  </label>
                  <label>
                    <span>最大值</span>
                    <input
                      type='number'
                      step='any'
                      aria-label={`${ariaName}最大值`}
                      value={axis?.maximum ?? ''}
                      placeholder='自动'
                      onChange={(event) => updateAxis(position, { maximum: optionalNumber(event.target.value) })}
                    />
                  </label>
                  <label>
                    <span>主单位</span>
                    <input
                      type='number'
                      min='0'
                      step='any'
                      aria-label={`${ariaName}主单位`}
                      value={axis?.majorUnit ?? ''}
                      placeholder='自动'
                      onChange={(event) => updateAxis(position, { majorUnit: optionalNumber(event.target.value) })}
                    />
                  </label>
                  <label>
                    <span>数字格式</span>
                    <input
                      aria-label={`${ariaName}数字格式`}
                      value={axis?.numberFormat ?? ''}
                      maxLength={255}
                      placeholder='#,##0 或 0.0%'
                      onChange={(event) => {
                        const numberFormat = event.target.value;
                        updateAxis(position, {
                          numberFormat,
                          numberFormatSourceLinked: numberFormat.trim() ? false : undefined,
                        });
                      }}
                    />
                  </label>
                  <label className='axis-check'>
                    <input
                      type='checkbox'
                      aria-label={`${ariaName}主要网格线`}
                      checked={showMajorGridlines}
                      onChange={(event) => updateAxis(position, { showMajorGridlines: event.target.checked })}
                    />
                    <span>主要网格线</span>
                  </label>
                  <label className='axis-check'>
                    <input
                      type='checkbox'
                      aria-label={`${ariaName}链接源数字格式`}
                      checked={sourceLinked}
                      onChange={(event) => updateAxis(position, { numberFormatSourceLinked: event.target.checked })}
                    />
                    <span>链接源格式</span>
                  </label>
                </>
              )}
            </fieldset>
          );
        })}
      </div>
    </section>
  );
}

function optionalNumber(value: string): number | undefined {
  if (!value.trim()) return undefined;
  const number = Number(value);
  return Number.isFinite(number) ? number : undefined;
}

function optionalInteger(value: string): number | undefined {
  if (!value.trim()) return undefined;
  const number = Number(value);
  return Number.isInteger(number) ? number : undefined;
}
