import { attribute, directChild } from './work-ooxml-package';
import type { WorkSpreadsheetHeaderFooterSections, WorkSpreadsheetPageSetup } from './work-types';

const OOXML_TO_FRIENDLY_TOKEN: Record<string, string> = {
  P: '{page}',
  N: '{pages}',
  A: '{sheet}',
  F: '{file}',
  Z: '{path}',
  D: '{date}',
  T: '{time}',
};

const FRIENDLY_TO_OOXML_TOKEN: Record<string, string> = Object.fromEntries(
  Object.entries(OOXML_TO_FRIENDLY_TOKEN).map(([ooxml, friendly]) => [friendly, `&${ooxml}`])
);

const RICH_FORMAT_CODES = new Set(['B', 'I', 'U', 'E', 'S', 'X', 'Y', 'O', 'H', '+', '-']);

export interface XlsxHeaderFooterInspection {
  hasFormatting: boolean;
  hasImage: boolean;
}

type XlsxHeaderFooterSettings = Pick<
  WorkSpreadsheetPageSetup,
  'header' | 'footer' | 'scaleWithDocument' | 'alignWithMargins'
>;

export function parseXlsxHeaderFooterSections(
  source: string | null | undefined
): WorkSpreadsheetHeaderFooterSections | undefined {
  if (!source) return undefined;
  const sections: Required<WorkSpreadsheetHeaderFooterSections> = { left: '', center: '', right: '' };
  let section: keyof WorkSpreadsheetHeaderFooterSections = 'center';
  for (let index = 0; index < source.length; index += 1) {
    if (source[index] !== '&' || index === source.length - 1) {
      sections[section] += source[index];
      continue;
    }
    const code = source[index + 1];
    if (code === '&') {
      sections[section] += '&';
      index += 1;
      continue;
    }
    if (code === 'L' || code === 'C' || code === 'R') {
      section = code === 'L' ? 'left' : code === 'C' ? 'center' : 'right';
      index += 1;
      continue;
    }
    const friendlyToken = OOXML_TO_FRIENDLY_TOKEN[code];
    if (friendlyToken) {
      sections[section] += friendlyToken;
      index += 1;
      continue;
    }
    const formattingEnd = formattingCodeEnd(source, index);
    if (formattingEnd !== null || code === 'G') {
      index = formattingEnd ?? index + 1;
      continue;
    }
    sections[section] += `&${code}`;
    index += 1;
  }
  return Object.values(sections).some(Boolean) ? sections : undefined;
}

export function serializeXlsxHeaderFooterSections(sections: WorkSpreadsheetHeaderFooterSections | undefined): string {
  if (!sections) return '';
  return (
    [
      ['L', sections.left],
      ['C', sections.center],
      ['R', sections.right],
    ] as const
  )
    .flatMap(([code, template]) => (template ? [`&${code}${serializeTemplate(template)}`] : []))
    .join('');
}

export function readXlsxHeaderFooter(element: Element | undefined): XlsxHeaderFooterSettings {
  if (!element) return {};
  const settings: XlsxHeaderFooterSettings = {};
  const header = parseXlsxHeaderFooterSections(directChild(element, 'oddHeader')?.textContent);
  const footer = parseXlsxHeaderFooterSections(directChild(element, 'oddFooter')?.textContent);
  if (header) settings.header = header;
  if (footer) settings.footer = footer;
  if (attribute(element, 'scaleWithDoc') !== null) {
    settings.scaleWithDocument = booleanAttribute(element, 'scaleWithDoc');
  }
  if (attribute(element, 'alignWithMargins') !== null) {
    settings.alignWithMargins = booleanAttribute(element, 'alignWithMargins');
  }
  return settings;
}

export function writeXlsxHeaderFooter(document: Document, pageSetup: WorkSpreadsheetPageSetup): Element | null {
  const header = serializeXlsxHeaderFooterSections(pageSetup.header);
  const footer = serializeXlsxHeaderFooterSections(pageSetup.footer);
  if (!header && !footer && pageSetup.scaleWithDocument === undefined && pageSetup.alignWithMargins === undefined) {
    return null;
  }
  const element = document.createElementNS(document.documentElement.namespaceURI, 'headerFooter');
  if (pageSetup.scaleWithDocument !== undefined) {
    element.setAttribute('scaleWithDoc', pageSetup.scaleWithDocument ? '1' : '0');
  }
  if (pageSetup.alignWithMargins !== undefined) {
    element.setAttribute('alignWithMargins', pageSetup.alignWithMargins ? '1' : '0');
  }
  appendTextChild(document, element, 'oddHeader', header);
  appendTextChild(document, element, 'oddFooter', footer);
  return element;
}

export function inspectXlsxHeaderFooterText(source: string | null | undefined): XlsxHeaderFooterInspection {
  const inspection: XlsxHeaderFooterInspection = { hasFormatting: false, hasImage: false };
  if (!source) return inspection;
  for (let index = 0; index < source.length - 1; index += 1) {
    if (source[index] !== '&') continue;
    const code = source[index + 1];
    if (code === '&') {
      index += 1;
      continue;
    }
    if (code === 'G') inspection.hasImage = true;
    if (formattingCodeEnd(source, index) !== null) inspection.hasFormatting = true;
  }
  return inspection;
}

function serializeTemplate(template: string): string {
  const escaped = template.replaceAll('&', '&&');
  return escaped.replace(
    /\{(?:page|pages|sheet|file|path|date|time)\}/g,
    (token) => FRIENDLY_TO_OOXML_TOKEN[token] ?? token
  );
}

function formattingCodeEnd(source: string, ampersandIndex: number): number | null {
  const code = source[ampersandIndex + 1];
  if (RICH_FORMAT_CODES.has(code)) return ampersandIndex + 1;
  if (/^\d$/.test(code)) {
    let end = ampersandIndex + 1;
    while (end + 1 < source.length && /^\d$/.test(source[end + 1])) end += 1;
    return end;
  }
  if (code === '"') {
    const closingQuote = source.indexOf('"', ampersandIndex + 2);
    return closingQuote < 0 ? source.length - 1 : closingQuote;
  }
  if (code === 'K') {
    let end = ampersandIndex + 1;
    while (end + 1 < source.length && end < ampersandIndex + 7 && /^[0-9A-Fa-f+-]$/.test(source[end + 1])) {
      end += 1;
    }
    return end;
  }
  return null;
}

function appendTextChild(document: Document, parent: Element, name: string, value: string): void {
  if (!value) return;
  const child = document.createElementNS(document.documentElement.namespaceURI, name);
  child.textContent = value;
  parent.append(child);
}

function booleanAttribute(element: Element, name: string): boolean {
  const value = attribute(element, name)?.toLowerCase();
  return value === '1' || value === 'true';
}
