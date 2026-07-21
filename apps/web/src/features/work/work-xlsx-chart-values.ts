import { attribute, descendants, directChild, firstDescendant } from './work-ooxml-package';

export function cachedText(parent: Element): string {
  const directValue = directChild(parent, 'v')?.textContent?.trim();
  return directValue || cachedValues(parent)[0] || '';
}

export function richText(parent: Element): string {
  return descendants(parent, 't')
    .map((node) => node.textContent ?? '')
    .join('');
}

export function cachedValues(parent: Element): string[] {
  const cache =
    firstDescendant(parent, 'strCache') ??
    firstDescendant(parent, 'numCache') ??
    firstDescendant(parent, 'strLit') ??
    firstDescendant(parent, 'numLit');
  if (!cache) return [];
  const indexed = descendants(cache, 'pt')
    .map((point, order) => ({
      index: finiteNumber(attribute(point, 'idx'), order),
      value: directChild(point, 'v')?.textContent ?? '',
    }))
    .sort((left, right) => left.index - right.index);
  if (!indexed.length) return [];
  const values = Array<string>(indexed.at(-1)!.index + 1).fill('');
  for (const point of indexed) values[point.index] = point.value;
  return values;
}

export function formulaReference(parent: Element): string | undefined {
  return firstDescendant(parent, 'f')?.textContent?.trim().replace(/^=/, '') || undefined;
}

export function stringCacheXml(values: readonly string[]): string {
  return [
    '<c:strCache>',
    `<c:ptCount val="${values.length}"/>`,
    ...values.map((value, index) => `<c:pt idx="${index}"><c:v>${escapeXml(String(value))}</c:v></c:pt>`),
    '</c:strCache>',
  ].join('');
}

export function numberCacheXml(values: readonly number[]): string {
  return [
    '<c:numCache><c:formatCode>General</c:formatCode>',
    `<c:ptCount val="${values.length}"/>`,
    ...values.map((value, index) => `<c:pt idx="${index}"><c:v>${finiteNumber(value)}</c:v></c:pt>`),
    '</c:numCache>',
  ].join('');
}

export function stringLiteralXml(values: readonly string[]): string {
  return [
    '<c:strLit>',
    `<c:ptCount val="${values.length}"/>`,
    ...values.map((value, index) => `<c:pt idx="${index}"><c:v>${escapeXml(String(value))}</c:v></c:pt>`),
    '</c:strLit>',
  ].join('');
}

export function numberLiteralXml(values: readonly number[]): string {
  return [
    '<c:numLit><c:formatCode>General</c:formatCode>',
    `<c:ptCount val="${values.length}"/>`,
    ...values.map((value, index) => `<c:pt idx="${index}"><c:v>${finiteNumber(value)}</c:v></c:pt>`),
    '</c:numLit>',
  ].join('');
}

export function finiteNumber(value: unknown, fallback = 0): number {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

export function escapeXml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&apos;');
}
