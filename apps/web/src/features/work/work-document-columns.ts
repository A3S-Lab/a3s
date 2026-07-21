import type { WorkDocumentColumns } from './work-types';

interface LegacyDocumentColumns {
  count?: number;
  spacing?: number;
  separator?: boolean;
}

export const DEFAULT_DOCUMENT_COLUMNS: WorkDocumentColumns = {
  count: 1,
  spacing: 12,
  separator: false,
};

const MAX_COLUMNS = 6;
const MIN_COLUMN_PERCENT = 5;

export function normalizeDocumentColumns(columns?: Partial<WorkDocumentColumns>): WorkDocumentColumns {
  const customCount = Array.isArray(columns?.custom) ? columns.custom.length : 0;
  const count = clampInteger(columns?.count, customCount || DEFAULT_DOCUMENT_COLUMNS.count, 1, MAX_COLUMNS);
  const spacing = clampNumber(columns?.spacing, DEFAULT_DOCUMENT_COLUMNS.spacing, 0, 30);
  const normalized: WorkDocumentColumns = {
    count,
    spacing: roundOne(spacing),
    separator: Boolean(columns?.separator),
  };
  if (count < 2 || !customCount) return normalized;

  const source = columns?.custom ?? [];
  const widths = normalizedPercentages(
    Array.from({ length: count }, (_, index) => finiteNumber(source[index]?.widthPercent, 100 / count)),
    100,
    MIN_COLUMN_PERCENT
  );
  normalized.custom = Array.from({ length: count }, (_, index) => ({
    widthPercent: widths[index],
    spacing: index === count - 1 ? 0 : roundOne(clampNumber(source[index]?.spacing, normalized.spacing, 0, 30)),
  }));
  return normalized;
}

export function serializeDocumentColumns(columns: WorkDocumentColumns): string {
  return JSON.stringify(normalizeDocumentColumns(columns));
}

export function parseDocumentColumns(
  source: string | undefined,
  legacy: LegacyDocumentColumns = {},
  fallback?: WorkDocumentColumns
): WorkDocumentColumns {
  if (source?.trim()) {
    try {
      return normalizeDocumentColumns(JSON.parse(source) as Partial<WorkDocumentColumns>);
    } catch {
      // Fall through to the compatible legacy representation.
    }
  }
  if (legacy.count !== undefined || legacy.spacing !== undefined || legacy.separator !== undefined) {
    return normalizeDocumentColumns(legacy);
  }
  return normalizeDocumentColumns(fallback);
}

export function setCustomDocumentColumns(columns: WorkDocumentColumns, enabled: boolean): WorkDocumentColumns {
  const normalized = normalizeDocumentColumns(columns);
  if (!enabled || normalized.count < 2) return { ...normalized, custom: undefined };
  if (normalized.custom) return normalized;
  return normalizeDocumentColumns({
    ...normalized,
    custom: Array.from({ length: normalized.count }, (_, index) => ({
      widthPercent: 100 / normalized.count,
      spacing: index === normalized.count - 1 ? 0 : normalized.spacing,
    })),
  });
}

export function updateDocumentColumnWidth(
  columns: WorkDocumentColumns,
  index: number,
  widthPercent: number
): WorkDocumentColumns {
  const normalized = setCustomDocumentColumns(columns, true);
  if (!normalized.custom?.[index]) return normalized;
  const maximum = 100 - MIN_COLUMN_PERCENT * (normalized.custom.length - 1);
  const target = Math.min(maximum, Math.max(MIN_COLUMN_PERCENT, finiteNumber(widthPercent, 100 / normalized.count)));
  const otherIndexes = normalized.custom.map((_, itemIndex) => itemIndex).filter((itemIndex) => itemIndex !== index);
  const otherWidths = normalizedPercentages(
    otherIndexes.map((itemIndex) => normalized.custom?.[itemIndex].widthPercent ?? 1),
    100 - target,
    MIN_COLUMN_PERCENT
  );
  const custom = normalized.custom.map((column, itemIndex) => {
    const otherIndex = otherIndexes.indexOf(itemIndex);
    return {
      ...column,
      widthPercent: itemIndex === index ? roundOne(target) : otherWidths[otherIndex],
    };
  });
  return { ...normalized, custom };
}

export function documentColumnGridTemplate(columns: WorkDocumentColumns): string {
  const custom = normalizeDocumentColumns(columns).custom;
  if (!custom) return '';
  return custom
    .flatMap((column, index) => [
      `minmax(0, ${formatNumber(column.widthPercent)}fr)`,
      ...(index < custom.length - 1 ? [`${formatNumber(column.spacing)}mm`] : []),
    ])
    .join(' ');
}

export function documentUnequalColumnGroups(html: string, columns: WorkDocumentColumns): string[] {
  const custom = normalizeDocumentColumns(columns).custom;
  if (!custom) return [html];
  const document = new DOMParser().parseFromString(html, 'text/html');
  const nodes = Array.from(document.body.childNodes).filter(
    (node) => node.nodeType !== Node.TEXT_NODE || Boolean(node.textContent?.trim())
  );
  const groups = custom.map(() => document.createElement('div'));
  if (!nodes.length) return groups.map(() => '');

  const costs = nodes.map(blockCost);
  const totalCost = costs.reduce((total, cost) => total + cost, 0);
  let columnIndex = 0;
  let assignedCost = 0;
  for (const [nodeIndex, node] of nodes.entries()) {
    if (columnIndex < custom.length - 1 && groups[columnIndex].childNodes.length > 0) {
      const targetPercent = custom.slice(0, columnIndex + 1).reduce((total, column) => total + column.widthPercent, 0);
      const targetCost = (totalCost * targetPercent) / 100;
      const currentDistance = Math.abs(targetCost - assignedCost);
      const nextDistance = Math.abs(targetCost - assignedCost - costs[nodeIndex]);
      const remainingNodes = nodes.length - nodeIndex;
      const remainingColumns = custom.length - columnIndex - 1;
      if (currentDistance <= nextDistance || remainingNodes <= remainingColumns) columnIndex += 1;
    }
    groups[columnIndex].append(node.cloneNode(true));
    assignedCost += costs[nodeIndex];
  }
  return groups.map((group) => group.innerHTML);
}

function normalizedPercentages(values: number[], total: number, minimum: number): number[] {
  if (!values.length) return [];
  const result = Array<number>(values.length).fill(0);
  const remaining = new Set(values.map((_, index) => index));
  let remainingTotal = total;
  while (remaining.size) {
    const sourceTotal = Array.from(remaining).reduce(
      (sum, index) => sum + Math.max(0.001, finiteNumber(values[index], 1)),
      0
    );
    const belowMinimum = Array.from(remaining).filter(
      (index) => (Math.max(0.001, finiteNumber(values[index], 1)) / sourceTotal) * remainingTotal < minimum
    );
    if (!belowMinimum.length) {
      for (const index of remaining) {
        result[index] = (Math.max(0.001, finiteNumber(values[index], 1)) / sourceTotal) * remainingTotal;
      }
      break;
    }
    for (const index of belowMinimum) {
      result[index] = minimum;
      remaining.delete(index);
      remainingTotal -= minimum;
    }
  }
  const rounded = result.map(roundOne);
  const adjustmentIndex = rounded.indexOf(Math.max(...rounded));
  rounded[adjustmentIndex] = roundOne(
    rounded[adjustmentIndex] + total - rounded.reduce((sum, value) => sum + value, 0)
  );
  return rounded;
}

function blockCost(node: Node): number {
  if (!(node instanceof HTMLElement)) return Math.max(1, (node.textContent?.trim().length ?? 0) / 80);
  const textCost = Math.max(1, (node.textContent?.trim().length ?? 0) / 100);
  const tableCost = node.tagName === 'TABLE' ? Math.max(4, node.querySelectorAll('tr').length * 2) : 0;
  const imageCost = node.querySelectorAll('img').length * 5;
  const headingCost = /^H[1-6]$/.test(node.tagName) ? 2 : 0;
  return textCost + tableCost + imageCost + headingCost;
}

function clampInteger(value: number | undefined, fallback: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, Math.round(finiteNumber(value, fallback))));
}

function clampNumber(value: number | undefined, fallback: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, finiteNumber(value, fallback)));
}

function finiteNumber(value: number | undefined, fallback: number): number {
  return Number.isFinite(value) ? Number(value) : fallback;
}

function roundOne(value: number): number {
  return Math.round(value * 10) / 10;
}

function formatNumber(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(1).replace(/\.0$/, '');
}
