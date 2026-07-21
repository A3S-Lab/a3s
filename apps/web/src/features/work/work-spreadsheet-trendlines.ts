import { normalizeWorkSpreadsheetTrendline, type WorkSpreadsheetTrendline } from './work-types';
import { roundChartNumber } from './work-spreadsheet-chart-svg-utils';

export interface SpreadsheetTrendlinePoint {
  x: number;
  y: number;
}

export interface SpreadsheetTrendlineFit {
  points: SpreadsheetTrendlinePoint[];
  equation?: string;
  rSquared?: number;
}

export function fitSpreadsheetTrendline(
  xValues: readonly number[],
  yValues: readonly number[],
  source: WorkSpreadsheetTrendline
): SpreadsheetTrendlineFit | null {
  const trendline = normalizeWorkSpreadsheetTrendline(source);
  const observed = yValues.flatMap((y, index) => {
    const x = xValues[index];
    return Number.isFinite(x) && Number.isFinite(y) ? [{ x, y }] : [];
  });
  if (observed.length < 2) return null;
  if (trendline.type === 'movingAverage') return movingAverageFit(observed, trendline.period ?? 2);
  const model = regressionModel(observed, trendline);
  if (!model) return null;
  const minimum = Math.min(...observed.map((point) => point.x)) - (trendline.backward ?? 0);
  const maximum = Math.max(...observed.map((point) => point.x)) + (trendline.forward ?? 0);
  const points = sampledPoints(minimum, maximum, model.predict);
  if (points.length < 2) return null;
  return {
    points,
    equation: model.equation,
    rSquared: coefficientOfDetermination(observed, model.predict),
  };
}

interface RegressionModel {
  predict: (x: number) => number;
  equation: string;
}

function regressionModel(
  observed: SpreadsheetTrendlinePoint[],
  trendline: WorkSpreadsheetTrendline
): RegressionModel | null {
  if (trendline.type === 'linear') return polynomialModel(observed, 1, trendline.intercept);
  if (trendline.type === 'polynomial') {
    return polynomialModel(observed, Math.min(trendline.order ?? 2, observed.length - 1), trendline.intercept);
  }
  if (trendline.type === 'exponential') {
    const usable = observed.filter((point) => point.y > 0);
    const coefficients = linearCoefficients(usable.map((point) => ({ x: point.x, y: Math.log(point.y) })));
    if (!coefficients) return null;
    const [intercept, slope] = coefficients;
    const factor = Math.exp(intercept);
    return {
      predict: (x) => factor * Math.exp(slope * x),
      equation: `y = ${formatCoefficient(factor)}e^(${formatCoefficient(slope)}x)`,
    };
  }
  if (trendline.type === 'logarithmic') {
    const usable = observed.filter((point) => point.x > 0);
    const coefficients = linearCoefficients(usable.map((point) => ({ x: Math.log(point.x), y: point.y })));
    if (!coefficients) return null;
    const [intercept, slope] = coefficients;
    return {
      predict: (x) => (x > 0 ? intercept + slope * Math.log(x) : Number.NaN),
      equation: `y = ${formatCoefficient(slope)}ln(x) ${signedConstant(intercept)}`,
    };
  }
  if (trendline.type === 'power') {
    const usable = observed.filter((point) => point.x > 0 && point.y > 0);
    const coefficients = linearCoefficients(usable.map((point) => ({ x: Math.log(point.x), y: Math.log(point.y) })));
    if (!coefficients) return null;
    const [intercept, exponent] = coefficients;
    const factor = Math.exp(intercept);
    return {
      predict: (x) => (x > 0 ? factor * x ** exponent : Number.NaN),
      equation: `y = ${formatCoefficient(factor)}x^${formatCoefficient(exponent)}`,
    };
  }
  return null;
}

function polynomialModel(
  observed: SpreadsheetTrendlinePoint[],
  order: number,
  fixedIntercept: number | undefined
): RegressionModel | null {
  const startPower = fixedIntercept === undefined ? 0 : 1;
  const coefficientCount = order - startPower + 1;
  if (coefficientCount < 1 || observed.length < coefficientCount) return null;
  const matrix = Array.from({ length: coefficientCount }, (_, row) =>
    Array.from({ length: coefficientCount }, (_, column) =>
      observed.reduce((sum, point) => sum + point.x ** (row + column + startPower * 2), 0)
    )
  );
  const vector = Array.from({ length: coefficientCount }, (_, row) =>
    observed.reduce((sum, point) => sum + point.x ** (row + startPower) * (point.y - (fixedIntercept ?? 0)), 0)
  );
  const solved = solveLinearSystem(matrix, vector);
  if (!solved) return null;
  const coefficients = fixedIntercept === undefined ? solved : [fixedIntercept, ...solved];
  const predict = (x: number) => coefficients.reduce((sum, coefficient, power) => sum + coefficient * x ** power, 0);
  return {
    predict,
    equation: polynomialEquation(coefficients),
  };
}

function linearCoefficients(observed: SpreadsheetTrendlinePoint[]): [number, number] | null {
  if (observed.length < 2) return null;
  const count = observed.length;
  const sumX = observed.reduce((sum, point) => sum + point.x, 0);
  const sumY = observed.reduce((sum, point) => sum + point.y, 0);
  const sumXX = observed.reduce((sum, point) => sum + point.x * point.x, 0);
  const sumXY = observed.reduce((sum, point) => sum + point.x * point.y, 0);
  const denominator = count * sumXX - sumX * sumX;
  if (Math.abs(denominator) < 1e-12) return null;
  const slope = (count * sumXY - sumX * sumY) / denominator;
  return [(sumY - slope * sumX) / count, slope];
}

function solveLinearSystem(matrix: number[][], vector: number[]): number[] | null {
  const size = vector.length;
  const augmented = matrix.map((row, index) => [...row, vector[index]]);
  for (let column = 0; column < size; column += 1) {
    let pivot = column;
    for (let row = column + 1; row < size; row += 1) {
      if (Math.abs(augmented[row][column]) > Math.abs(augmented[pivot][column])) pivot = row;
    }
    if (Math.abs(augmented[pivot][column]) < 1e-12) return null;
    [augmented[column], augmented[pivot]] = [augmented[pivot], augmented[column]];
    const divisor = augmented[column][column];
    for (let index = column; index <= size; index += 1) augmented[column][index] /= divisor;
    for (let row = 0; row < size; row += 1) {
      if (row === column) continue;
      const factor = augmented[row][column];
      for (let index = column; index <= size; index += 1) {
        augmented[row][index] -= factor * augmented[column][index];
      }
    }
  }
  return augmented.map((row) => row[size]);
}

function sampledPoints(minimum: number, maximum: number, predict: (x: number) => number): SpreadsheetTrendlinePoint[] {
  const span = maximum - minimum;
  if (!Number.isFinite(span) || span <= 0) return [];
  return Array.from({ length: 65 }, (_, index) => {
    const x = minimum + (index / 64) * span;
    return { x, y: predict(x) };
  }).filter((point) => Number.isFinite(point.y));
}

function movingAverageFit(observed: SpreadsheetTrendlinePoint[], period: number): SpreadsheetTrendlineFit | null {
  if (observed.length < period) return null;
  const points = observed.slice(period - 1).map((point, index) => ({
    x: point.x,
    y: observed.slice(index, index + period).reduce((sum, item) => sum + item.y, 0) / period,
  }));
  return { points, equation: `Moving average (${period})` };
}

function coefficientOfDetermination(
  observed: SpreadsheetTrendlinePoint[],
  predict: (x: number) => number
): number | undefined {
  const usable = observed
    .map((point) => ({ actual: point.y, predicted: predict(point.x) }))
    .filter((point) => Number.isFinite(point.predicted));
  if (usable.length < 2) return undefined;
  const mean = usable.reduce((sum, point) => sum + point.actual, 0) / usable.length;
  const total = usable.reduce((sum, point) => sum + (point.actual - mean) ** 2, 0);
  const residual = usable.reduce((sum, point) => sum + (point.actual - point.predicted) ** 2, 0);
  if (total < 1e-12) return residual < 1e-12 ? 1 : 0;
  return Math.max(0, Math.min(1, 1 - residual / total));
}

function polynomialEquation(coefficients: number[]): string {
  const terms = coefficients
    .map((coefficient, power) => {
      if (Math.abs(coefficient) < 1e-10) return '';
      const absolute = formatCoefficient(Math.abs(coefficient));
      const variable = power === 0 ? '' : power === 1 ? 'x' : `x^${power}`;
      return `${coefficient < 0 ? '-' : '+'}${absolute}${variable}`;
    })
    .filter(Boolean);
  if (!terms.length) return 'y = 0';
  return `y = ${terms.join(' ').replace(/^\+/, '').replaceAll('+', '+ ').replaceAll('-', '- ').trim()}`;
}

function formatCoefficient(value: number): string {
  return String(roundChartNumber(value));
}

function signedConstant(value: number): string {
  return `${value < 0 ? '-' : '+'} ${formatCoefficient(Math.abs(value))}`;
}
