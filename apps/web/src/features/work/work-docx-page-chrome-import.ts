import { documentPageChromeLegacyFields, normalizeDocumentPageChrome } from './work-document-page-chrome';
import {
  attribute,
  bytesToDataUrl,
  contentTypeForPart,
  descendants,
  directChild,
  directChildren,
  firstDescendant,
  type OoxmlPackage,
  type OoxmlRelationship,
} from './work-ooxml-package';
import type {
  WorkDocumentPageChrome,
  WorkDocumentPageChromeContent,
  WorkDocumentPageChromeVariant,
  WorkDocumentSectionLayout,
} from './work-types';

type Relationships = Map<string, OoxmlRelationship>;
interface ImportedPageChromePart {
  html: string;
  showPageNumber: boolean;
}

export async function importSectionPageChrome(
  section: Element,
  archive: OoxmlPackage,
  documentRelationships: Relationships,
  previous: WorkDocumentSectionLayout,
  oddEvenEnabled: boolean
): Promise<{
  pageChrome: WorkDocumentPageChrome;
  headerText?: string;
  footerText?: string;
  showPageNumbers?: boolean;
}> {
  const fallback = normalizeDocumentPageChrome(previous.pageChrome, previous);
  const variants = await Promise.all(
    (['default', 'first', 'even'] as const).map(async (variant) => {
      const content = await importVariant(section, variant, archive, documentRelationships, fallback[variant]);
      return [variant, content] as const;
    })
  );
  const hasFirstReferences = hasVariantReference(section, 'first');
  const hasEvenReferences = hasVariantReference(section, 'even');
  const pageChrome = normalizeDocumentPageChrome({
    differentFirstPage: enabledElement(firstDescendant(section, 'titlePg')) || hasFirstReferences,
    differentOddEvenPages: oddEvenEnabled || hasEvenReferences,
    ...Object.fromEntries(variants),
  });
  return { pageChrome, ...documentPageChromeLegacyFields(pageChrome) };
}

export function documentUsesOddEvenPageChrome(settings: Document | null): boolean {
  return Boolean(settings && descendants(settings, 'evenAndOddHeaders').some(enabledElement));
}

async function importVariant(
  section: Element,
  variant: WorkDocumentPageChromeVariant,
  archive: OoxmlPackage,
  documentRelationships: Relationships,
  fallback: WorkDocumentPageChromeContent
): Promise<WorkDocumentPageChromeContent> {
  const header = await importReferencedPart(section, 'headerReference', variant, archive, documentRelationships);
  const footer = await importReferencedPart(section, 'footerReference', variant, archive, documentRelationships);
  const hasImportedPart = Boolean(header || footer);
  return {
    headerHtml: header?.html ?? fallback.headerHtml,
    footerHtml: footer?.html ?? fallback.footerHtml,
    showPageNumber: hasImportedPart
      ? Boolean(header?.showPageNumber || footer?.showPageNumber)
      : fallback.showPageNumber,
  };
}

async function importReferencedPart(
  section: Element,
  referenceName: 'headerReference' | 'footerReference',
  variant: WorkDocumentPageChromeVariant,
  archive: OoxmlPackage,
  documentRelationships: Relationships
): Promise<ImportedPageChromePart | null> {
  const reference = directChildren(section, referenceName).find(
    (element) => (attribute(element, 'type') ?? 'default') === variant
  );
  const relationship = documentRelationships.get(attribute(reference ?? section, 'r:id') ?? '');
  if (!relationship || relationship.targetMode === 'External' || !archive.has(relationship.target)) return null;
  const document = await archive.xml(relationship.target);
  return {
    html: await pageChromePartHtml(document, archive, relationship.target),
    showPageNumber: containsPageNumber(document),
  };
}

async function pageChromePartHtml(document: Document, archive: OoxmlPackage, partPath: string): Promise<string> {
  const relationships = await archive.relationships(partPath);
  const root = document.documentElement;
  const blocks: string[] = [];
  for (const child of directChildren(root)) {
    if (child.localName === 'p') blocks.push(await paragraphHtml(child, archive, relationships));
    if (child.localName === 'tbl') blocks.push(await tableHtml(child, archive, relationships));
  }
  return blocks.join('');
}

async function paragraphHtml(paragraph: Element, archive: OoxmlPackage, relationships: Relationships): Promise<string> {
  const alignmentValue = attribute(firstDescendant(directChild(paragraph, 'pPr'), 'jc') ?? paragraph, 'val');
  const alignment =
    alignmentValue === 'center'
      ? 'center'
      : alignmentValue === 'right' || alignmentValue === 'end'
        ? 'right'
        : alignmentValue === 'both' || alignmentValue === 'distribute'
          ? 'justify'
          : alignmentValue === 'left' || alignmentValue === 'start'
            ? 'left'
            : '';
  const field: FieldState = { active: false, instruction: '', separated: false, skipResult: false };
  let html = '';
  for (const child of directChildren(paragraph)) {
    if (child.localName === 'pPr') continue;
    if (child.localName === 'hyperlink') {
      const content = await containerRunsHtml(child, field, archive, relationships);
      const relationship = relationships.get(attribute(child, 'r:id') ?? '');
      const anchor = attribute(child, 'anchor');
      const href = relationship?.targetMode === 'External' ? relationship.target : anchor ? `#${anchor}` : '';
      html += href ? `<a href="${escapeHtml(href)}">${content}</a>` : content;
      continue;
    }
    if (child.localName === 'fldSimple') {
      const instruction = attribute(child, 'instr') ?? '';
      if (!/\bPAGE\b/i.test(instruction)) {
        html += await containerRunsHtml(child, field, archive, relationships);
      }
      continue;
    }
    html += await containerRunsHtml(child, field, archive, relationships);
  }
  return `<p${alignment ? ` style="text-align: ${alignment}"` : ''}>${html}</p>`;
}

async function containerRunsHtml(
  container: Element,
  field: FieldState,
  archive: OoxmlPackage,
  relationships: Relationships
): Promise<string> {
  if (container.localName === 'r') return runHtml(container, field, archive, relationships);
  let html = '';
  for (const run of descendants(container, 'r')) html += await runHtml(run, field, archive, relationships);
  return html;
}

async function runHtml(
  run: Element,
  field: FieldState,
  archive: OoxmlPackage,
  relationships: Relationships
): Promise<string> {
  let content = '';
  for (const child of directChildren(run)) {
    if (child.localName === 'fldChar') {
      updateFieldState(field, attribute(child, 'fldCharType'));
      continue;
    }
    if (child.localName === 'instrText') {
      if (field.active && !field.separated) field.instruction += child.textContent ?? '';
      continue;
    }
    if (!fieldContentVisible(field)) continue;
    if (child.localName === 't') content += escapeHtml(child.textContent ?? '');
    if (child.localName === 'tab') content += '&#9;';
    if (child.localName === 'br' || child.localName === 'cr') content += '<br>';
    if (child.localName === 'drawing') content += await drawingHtml(child, archive, relationships);
  }
  if (!content) return '';

  const properties = directChild(run, 'rPr');
  if (enabledElement(directChild(properties ?? run, 'b'))) content = `<strong>${content}</strong>`;
  if (enabledElement(directChild(properties ?? run, 'i'))) content = `<em>${content}</em>`;
  const underline = directChild(properties ?? run, 'u');
  if (underline && attribute(underline, 'val') !== 'none') content = `<u>${content}</u>`;
  if (
    enabledElement(directChild(properties ?? run, 'strike')) ||
    enabledElement(directChild(properties ?? run, 'dstrike'))
  ) {
    content = `<s>${content}</s>`;
  }
  const verticalAlign = attribute(directChild(properties ?? run, 'vertAlign') ?? run, 'val');
  if (verticalAlign === 'superscript') content = `<sup>${content}</sup>`;
  if (verticalAlign === 'subscript') content = `<sub>${content}</sub>`;
  const color = attribute(directChild(properties ?? run, 'color') ?? run, 'val');
  if (color && /^[0-9a-f]{6}$/i.test(color)) content = `<span style="color: #${color}">${content}</span>`;
  return content;
}

async function drawingHtml(drawing: Element, archive: OoxmlPackage, relationships: Relationships): Promise<string> {
  const relationship = relationships.get(attribute(firstDescendant(drawing, 'blip') ?? drawing, 'embed') ?? '');
  if (!relationship || relationship.targetMode === 'External' || !archive.has(relationship.target)) return '';
  const source = bytesToDataUrl(await archive.bytes(relationship.target), contentTypeForPart(relationship.target));
  const extent = firstDescendant(drawing, 'extent');
  const width = Math.max(1, Math.round(Number(attribute(extent ?? drawing, 'cx')) / 9525));
  const height = Math.max(1, Math.round(Number(attribute(extent ?? drawing, 'cy')) / 9525));
  return `<img src="${source}" alt="Header or footer image"${width > 1 ? ` width="${width}"` : ''}${
    height > 1 ? ` height="${height}"` : ''
  }>`;
}

async function tableHtml(table: Element, archive: OoxmlPackage, relationships: Relationships): Promise<string> {
  const rows: string[] = [];
  for (const row of directChildren(table, 'tr')) {
    const cells: string[] = [];
    for (const cell of directChildren(row, 'tc')) {
      const paragraphs = await Promise.all(
        directChildren(cell, 'p').map((item) => paragraphHtml(item, archive, relationships))
      );
      cells.push(`<td>${paragraphs.join('')}</td>`);
    }
    rows.push(`<tr>${cells.join('')}</tr>`);
  }
  return `<table><tbody>${rows.join('')}</tbody></table>`;
}

function containsPageNumber(document: Document): boolean {
  return (
    descendants(document, 'instrText').some((element) => /\bPAGE\b/i.test(element.textContent ?? '')) ||
    descendants(document, 'fldSimple').some((element) => /\bPAGE\b/i.test(attribute(element, 'instr') ?? ''))
  );
}

function hasVariantReference(section: Element, variant: WorkDocumentPageChromeVariant): boolean {
  return [...directChildren(section, 'headerReference'), ...directChildren(section, 'footerReference')].some(
    (element) => (attribute(element, 'type') ?? 'default') === variant
  );
}

function enabledElement(element: Element | undefined): boolean {
  if (!element) return false;
  const value = attribute(element, 'val')?.toLowerCase();
  return value !== '0' && value !== 'false' && value !== 'off';
}

function escapeHtml(value: string): string {
  return value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
}

function updateFieldState(field: FieldState, fieldType: string | null): void {
  if (fieldType === 'begin') {
    field.active = true;
    field.instruction = '';
    field.separated = false;
    field.skipResult = false;
    return;
  }
  if (fieldType === 'separate' && field.active) {
    field.separated = true;
    field.skipResult = /\bPAGE\b/i.test(field.instruction);
    return;
  }
  if (fieldType === 'end') {
    field.active = false;
    field.instruction = '';
    field.separated = false;
    field.skipResult = false;
  }
}

function fieldContentVisible(field: FieldState): boolean {
  return !field.active || (field.separated && !field.skipResult);
}

interface FieldState {
  active: boolean;
  instruction: string;
  separated: boolean;
  skipResult: boolean;
}
