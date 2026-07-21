import {
  type WorkSpreadsheetChartAxes,
  type WorkSpreadsheetChartAxis,
  type WorkSpreadsheetChartAxisPosition,
  type WorkSpreadsheetChartType,
} from '../work-types';
import {
  workSpreadsheetChartAxisDefaultLabelPosition,
  workSpreadsheetChartAxisIsCategoryAxis,
  workSpreadsheetChartAxisIsValueAxis,
  workSpreadsheetChartAxisShowsMajorGridlinesByDefault,
} from '../work-spreadsheet-chart-axis';

interface SpreadsheetChartAxisEditorProps {
  axes: WorkSpreadsheetChartAxes | undefined;
  chartType: WorkSpreadsheetChartType;
  showSecondaryAxes: boolean;
  onChange: (axes: WorkSpreadsheetChartAxes) => void;
}

const PRIMARY_AXES: Array<{ position: WorkSpreadsheetChartAxisPosition; label: string }> = [
  { position: 'bottom', label: '横坐标轴' },
  { position: 'left', label: '纵坐标轴' },
];

const SECONDARY_AXES: Array<{ position: WorkSpreadsheetChartAxisPosition; label: string }> = [
  { position: 'top', label: '次横坐标轴' },
  { position: 'right', label: '次纵坐标轴' },
];

export function SpreadsheetChartAxisEditor({
  axes,
  chartType,
  showSecondaryAxes,
  onChange,
}: SpreadsheetChartAxisEditorProps) {
  const items = showSecondaryAxes ? [...PRIMARY_AXES, ...SECONDARY_AXES] : PRIMARY_AXES;
  const updateAxis = (position: WorkSpreadsheetChartAxisPosition, changes: Partial<WorkSpreadsheetChartAxis>) => {
    onChange({
      ...axes,
      [position]: {
        ...axes?.[position],
        ...changes,
      },
    });
  };

  return (
    <section className='work-spreadsheet-chart-axes' aria-label='图表坐标轴设置'>
      <header>
        <strong>坐标轴</strong>
        <span>标题、刻度与显示方式</span>
      </header>
      <div>
        {items.map(({ position, label }) => {
          const axis = axes?.[position];
          const valueAxis = workSpreadsheetChartAxisIsValueAxis(chartType, position);
          const categoryAxis = workSpreadsheetChartAxisIsCategoryAxis(chartType, position);
          const labelPosition =
            axis?.labelPosition ?? workSpreadsheetChartAxisDefaultLabelPosition(chartType, position);
          const showMajorGridlines =
            axis?.showMajorGridlines ?? workSpreadsheetChartAxisShowsMajorGridlinesByDefault(chartType, position);
          const sourceLinked = axis?.numberFormatSourceLinked ?? !axis?.numberFormat;
          return (
            <fieldset key={position}>
              <legend>{label}</legend>
              <label>
                <span>标题</span>
                <input
                  aria-label={`${label}标题`}
                  value={axis?.title ?? ''}
                  maxLength={255}
                  onChange={(event) => updateAxis(position, { title: event.target.value })}
                />
              </label>
              <label>
                <span>标题引用（可选）</span>
                <input
                  aria-label={`${label}标题引用`}
                  value={axis?.titleReference ?? ''}
                  placeholder="'报告'!$D$1"
                  onChange={(event) => updateAxis(position, { titleReference: event.target.value })}
                />
              </label>
              <label className='axis-check'>
                <input
                  type='checkbox'
                  aria-label={`${label}逆序显示`}
                  checked={axis?.reverseOrder === true}
                  onChange={(event) => updateAxis(position, { reverseOrder: event.target.checked })}
                />
                <span>逆序显示</span>
              </label>
              <label>
                <span>标签位置</span>
                <select
                  aria-label={`${label}标签位置`}
                  value={labelPosition}
                  onChange={(event) =>
                    updateAxis(position, {
                      labelPosition: event.target.value as NonNullable<WorkSpreadsheetChartAxis['labelPosition']>,
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
                  aria-label={`${label}主要刻度线`}
                  value={axis?.majorTickMark ?? 'none'}
                  onChange={(event) =>
                    updateAxis(position, {
                      majorTickMark: event.target.value as NonNullable<WorkSpreadsheetChartAxis['majorTickMark']>,
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
                    aria-label={`${label}标签间隔`}
                    value={axis?.labelInterval ?? ''}
                    placeholder='自动'
                    onChange={(event) => updateAxis(position, { labelInterval: optionalInteger(event.target.value) })}
                  />
                </label>
              )}
              {valueAxis && (
                <>
                  <label>
                    <span>最小值（自动）</span>
                    <input
                      type='number'
                      step='any'
                      aria-label={`${label}最小值`}
                      value={axis?.minimum ?? ''}
                      onChange={(event) => updateAxis(position, { minimum: optionalNumber(event.target.value) })}
                    />
                  </label>
                  <label>
                    <span>最大值（自动）</span>
                    <input
                      type='number'
                      step='any'
                      aria-label={`${label}最大值`}
                      value={axis?.maximum ?? ''}
                      onChange={(event) => updateAxis(position, { maximum: optionalNumber(event.target.value) })}
                    />
                  </label>
                  <label>
                    <span>主单位（自动）</span>
                    <input
                      type='number'
                      min='0'
                      step='any'
                      aria-label={`${label}主单位`}
                      value={axis?.majorUnit ?? ''}
                      onChange={(event) => updateAxis(position, { majorUnit: optionalNumber(event.target.value) })}
                    />
                  </label>
                  <label>
                    <span>数字格式</span>
                    <input
                      aria-label={`${label}数字格式`}
                      value={axis?.numberFormat ?? ''}
                      maxLength={255}
                      placeholder='自动（如 #,##0 或 0.0%）'
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
                      aria-label={`${label}显示主要网格线`}
                      checked={showMajorGridlines}
                      onChange={(event) => updateAxis(position, { showMajorGridlines: event.target.checked })}
                    />
                    <span>显示主要网格线</span>
                  </label>
                  <label className='axis-check'>
                    <input
                      type='checkbox'
                      aria-label={`${label}链接源数字格式`}
                      checked={sourceLinked}
                      onChange={(event) => updateAxis(position, { numberFormatSourceLinked: event.target.checked })}
                    />
                    <span>链接源数字格式</span>
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
