import {
  normalizeWorkSpreadsheetErrorBars,
  normalizeWorkSpreadsheetTrendline,
  type WorkSpreadsheetChartSeries,
  type WorkSpreadsheetChartType,
  workSpreadsheetErrorBarTypeLabel,
  workSpreadsheetErrorBarValueTypeLabel,
  workSpreadsheetTrendlineTypeLabel,
} from './work-types';

export function chartSeriesTrendlineContext(series: Pick<WorkSpreadsheetChartSeries, 'trendlines'>): string {
  if (!series.trendlines?.length) return '';
  const descriptions = series.trendlines.map((source) => {
    const trendline = normalizeWorkSpreadsheetTrendline(source);
    return [
      `${workSpreadsheetTrendlineTypeLabel(trendline.type)}${trendline.name ? `“${trendline.name}”` : ''}`,
      trendline.type === 'polynomial' ? `${trendline.order} 阶` : '',
      trendline.type === 'movingAverage' ? `${trendline.period} 期` : '',
      trendline.forward ? `前推 ${trendline.forward}` : '',
      trendline.backward ? `后推 ${trendline.backward}` : '',
      trendline.intercept !== undefined ? `截距 ${trendline.intercept}` : '',
      trendline.displayEquation ? '显示公式' : '',
      trendline.displayRSquared ? '显示 R 方' : '',
    ]
      .filter(Boolean)
      .join('，');
  });
  return `；趋势线：${descriptions.join('；')}`;
}

export function chartSeriesErrorBarContext(
  series: Pick<WorkSpreadsheetChartSeries, 'errorBars'>,
  chartType: WorkSpreadsheetChartType
): string {
  if (!series.errorBars?.length) return '';
  const descriptions = series.errorBars.map((source) => {
    const errorBars = normalizeWorkSpreadsheetErrorBars(source, chartType);
    const amount =
      errorBars.valueType === 'percentage'
        ? `${errorBars.value}%`
        : errorBars.valueType === 'fixedValue'
          ? String(errorBars.value)
          : errorBars.valueType === 'standardDeviation'
            ? `${errorBars.value} 倍`
            : '';
    const custom =
      errorBars.valueType === 'custom'
        ? [
            customErrorAmount('正误差', errorBars.plusReference, errorBars.plusValues),
            customErrorAmount('负误差', errorBars.minusReference, errorBars.minusValues),
          ]
            .filter(Boolean)
            .join('，')
        : '';
    return [
      errorBars.direction.toUpperCase(),
      `${workSpreadsheetErrorBarTypeLabel(errorBars.barType)}${workSpreadsheetErrorBarValueTypeLabel(
        errorBars.valueType
      )}`,
      amount,
      custom,
      errorBars.showEndCaps === false ? '无端帽' : '',
    ]
      .filter(Boolean)
      .join(' ');
  });
  return `；误差线：${descriptions.join('；')}`;
}

function customErrorAmount(label: string, reference: string | undefined, values: number[] | undefined): string {
  if (reference) return `${label} ${reference}`;
  return values?.length ? `${label} ${values.join('、')}` : '';
}
