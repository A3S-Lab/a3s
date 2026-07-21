import { normalizeDocumentColumns } from './work-document-columns';
import { attribute, directChildren } from './work-ooxml-package';
import type { WorkDocumentColumns } from './work-types';

const TWIPS_PER_MILLIMETER = 1440 / 25.4;

export function importDocxColumns(element: Element, fallback: WorkDocumentColumns): WorkDocumentColumns {
  const columnElements = directChildren(element, 'col');
  const sourceCount = numberAttribute(element, 'num') || columnElements.length || fallback.count;
  const count = Math.min(6, Math.max(1, sourceCount));
  const spacingAttribute = attribute(element, 'space');
  const spacing =
    spacingAttribute === null ? fallback.spacing : twipsToMillimeters(Number(spacingAttribute), fallback.spacing);
  const base = {
    count,
    spacing,
    separator: enabledAttribute(element, 'sep'),
  };
  if (!columnElements.length) return normalizeDocumentColumns(base);

  return normalizeDocumentColumns({
    ...base,
    count: Math.min(count, columnElements.length),
    custom: columnElements.slice(0, count).map((column, index) => {
      const columnSpacing = attribute(column, 'space');
      return {
        widthPercent: Math.max(1, numberAttribute(column, 'w')),
        spacing:
          index === Math.min(count, columnElements.length) - 1
            ? 0
            : columnSpacing === null
              ? spacing
              : twipsToMillimeters(Number(columnSpacing), spacing),
      };
    }),
  });
}

function numberAttribute(element: Element, name: string): number {
  const value = Number(attribute(element, name));
  return Number.isFinite(value) ? value : 0;
}

function enabledAttribute(element: Element, name: string): boolean {
  const value = attribute(element, name)?.toLowerCase();
  return value === '1' || value === 'true' || value === 'on';
}

function twipsToMillimeters(value: number, fallback: number): number {
  return Number.isFinite(value) ? roundOne(Math.max(0, value) / TWIPS_PER_MILLIMETER) : fallback;
}

function roundOne(value: number): number {
  return Math.round(value * 10) / 10;
}
