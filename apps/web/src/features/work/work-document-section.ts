import { clampDocumentMargin, documentMargins } from './work-document-layout';
import { normalizeDocumentCaptionsHtml } from './work-document-captions';
import { normalizeDocumentCitationsHtml } from './work-document-citations';
import { normalizeDocumentFieldsHtml } from './work-document-fields';
import { normalizeDocumentNotesHtml } from './work-document-notes';
import {
  DEFAULT_DOCUMENT_COLUMNS,
  normalizeDocumentColumns,
  parseDocumentColumns,
  serializeDocumentColumns,
} from './work-document-columns';
import {
  documentPageChromeLegacyFields,
  normalizeDocumentPageChrome,
  parseDocumentPageChrome,
  serializeDocumentPageChrome,
} from './work-document-page-chrome';
import type { WorkDocumentContent, WorkDocumentSectionBreakType, WorkDocumentSectionLayout } from './work-types';

export interface WorkDocumentSection {
  id: string;
  layout: WorkDocumentSectionLayout;
  html: string;
}

export interface DocumentSectionNodeAttributes {
  id: string;
  pageSize: WorkDocumentSectionLayout['pageSize'];
  orientation: WorkDocumentSectionLayout['orientation'];
  marginTop: number;
  marginRight: number;
  marginBottom: number;
  marginLeft: number;
  columnCount: number;
  columnSpacing: number;
  columnSeparator: boolean;
  columnLayout: string;
  breakAfter: WorkDocumentSectionBreakType;
  headerText: string;
  footerText: string;
  showPageNumbers: boolean;
  pageNumberStart: number | null;
  pageChrome: string;
}

const SECTION_SELECTOR = 'section[data-document-section]';

export function documentInitialSectionLayout(content: WorkDocumentContent): WorkDocumentSectionLayout {
  const pageChrome = normalizeDocumentPageChrome(content.pageChrome, content);
  const legacy = documentPageChromeLegacyFields(pageChrome);
  return {
    pageSize: content.pageSize,
    orientation: content.orientation ?? 'portrait',
    margins: documentMargins(content),
    columns: normalizeDocumentColumns(content.columns),
    breakAfter: 'nextPage',
    headerText: legacy.headerText,
    footerText: legacy.footerText,
    showPageNumbers: legacy.showPageNumbers,
    pageNumberStart: validPageNumber(content.pageNumberStart),
    pageChrome,
  };
}

export function normalizeDocumentHtml(content: WorkDocumentContent): string {
  const document = new DOMParser().parseFromString(content.html, 'text/html');
  const directSections = Array.from(document.body.children).filter((element) => element.matches(SECTION_SELECTOR));
  if (!directSections.length) {
    const section = document.createElement('section');
    applyDocumentSectionDomAttributes(section, documentInitialSectionLayout(content), 'document-section-1');
    while (document.body.firstChild) section.append(document.body.firstChild);
    if (!section.childNodes.length) section.innerHTML = '<p></p>';
    document.body.append(section);
    return normalizeDocumentSemanticHtml(document.body.innerHTML, content.bibliography);
  }

  let activeSection = directSections[0] as HTMLElement;
  for (const node of Array.from(document.body.childNodes)) {
    if (node instanceof HTMLElement && node.matches(SECTION_SELECTOR)) {
      activeSection = node;
      continue;
    }
    activeSection.append(node);
  }
  let fallback = documentInitialSectionLayout(content);
  directSections.forEach((element, index) => {
    const layout = documentSectionLayoutFromElement(element as HTMLElement, fallback);
    applyDocumentSectionDomAttributes(
      element as HTMLElement,
      layout,
      element.getAttribute('data-section-id') || `document-section-${index + 1}`
    );
    if (!element.childNodes.length) element.innerHTML = '<p></p>';
    fallback = layout;
  });
  return normalizeDocumentSemanticHtml(document.body.innerHTML, content.bibliography);
}

export function documentSections(content: WorkDocumentContent): WorkDocumentSection[] {
  const document = new DOMParser().parseFromString(normalizeDocumentHtml(content), 'text/html');
  let fallback = documentInitialSectionLayout(content);
  return Array.from(document.body.querySelectorAll<HTMLElement>(`:scope > ${SECTION_SELECTOR}`)).map(
    (element, index) => {
      const layout = documentSectionLayoutFromElement(element, fallback);
      fallback = layout;
      return {
        id: element.dataset.sectionId || `document-section-${index + 1}`,
        layout,
        html: element.innerHTML || '<p></p>',
      };
    }
  );
}

export function documentSectionNodeAttributes(
  layout: WorkDocumentSectionLayout,
  id: string
): DocumentSectionNodeAttributes {
  const columns = normalizeDocumentColumns(layout.columns);
  const pageChrome = normalizeDocumentPageChrome(layout.pageChrome, layout);
  const legacy = documentPageChromeLegacyFields(pageChrome);
  return {
    id,
    pageSize: layout.pageSize,
    orientation: layout.orientation,
    marginTop: layout.margins.top,
    marginRight: layout.margins.right,
    marginBottom: layout.margins.bottom,
    marginLeft: layout.margins.left,
    columnCount: columns.count,
    columnSpacing: columns.spacing,
    columnSeparator: columns.separator,
    columnLayout: serializeDocumentColumns(columns),
    breakAfter: validBreakType(layout.breakAfter),
    headerText: legacy.headerText ?? '',
    footerText: legacy.footerText ?? '',
    showPageNumbers: Boolean(legacy.showPageNumbers),
    pageNumberStart: validPageNumber(layout.pageNumberStart) ?? null,
    pageChrome: serializeDocumentPageChrome(pageChrome),
  };
}

export function documentSectionLayoutFromNodeAttributes(
  attributes: Partial<DocumentSectionNodeAttributes>,
  fallback?: WorkDocumentSectionLayout
): WorkDocumentSectionLayout {
  const base =
    fallback ??
    ({
      pageSize: 'a4',
      orientation: 'portrait',
      margins: { top: 25, right: 23, bottom: 25, left: 23 },
      columns: DEFAULT_DOCUMENT_COLUMNS,
      breakAfter: 'nextPage',
    } satisfies WorkDocumentSectionLayout);
  const pageChrome = parseDocumentPageChrome(
    attributes.pageChrome,
    {
      headerText: attributes.headerText,
      footerText: attributes.footerText,
      showPageNumbers: attributes.showPageNumbers,
    },
    base.pageChrome
  );
  const legacy = documentPageChromeLegacyFields(pageChrome);
  return {
    pageSize: attributes.pageSize === 'letter' ? 'letter' : attributes.pageSize === 'a4' ? 'a4' : base.pageSize,
    orientation:
      attributes.orientation === 'landscape'
        ? 'landscape'
        : attributes.orientation === 'portrait'
          ? 'portrait'
          : base.orientation,
    margins: {
      top: clampDocumentMargin(finiteNumber(attributes.marginTop, base.margins.top)),
      right: clampDocumentMargin(finiteNumber(attributes.marginRight, base.margins.right)),
      bottom: clampDocumentMargin(finiteNumber(attributes.marginBottom, base.margins.bottom)),
      left: clampDocumentMargin(finiteNumber(attributes.marginLeft, base.margins.left)),
    },
    columns: parseDocumentColumns(
      attributes.columnLayout,
      {
        count: attributes.columnCount,
        spacing: attributes.columnSpacing,
        separator: attributes.columnSeparator,
      },
      base.columns
    ),
    breakAfter: validBreakType(attributes.breakAfter ?? base.breakAfter),
    headerText: legacy.headerText,
    footerText: legacy.footerText,
    showPageNumbers: legacy.showPageNumbers,
    pageNumberStart: validPageNumber(attributes.pageNumberStart ?? undefined),
    pageChrome,
  };
}

export function documentSectionDomAttributes(layout: WorkDocumentSectionLayout, id: string): Record<string, string> {
  const attributes = documentSectionNodeAttributes(layout, id);
  return {
    'data-document-section': 'true',
    'data-section-id': attributes.id,
    'data-section-page-size': attributes.pageSize,
    'data-section-orientation': attributes.orientation,
    'data-section-margin-top': String(attributes.marginTop),
    'data-section-margin-right': String(attributes.marginRight),
    'data-section-margin-bottom': String(attributes.marginBottom),
    'data-section-margin-left': String(attributes.marginLeft),
    'data-section-column-count': String(attributes.columnCount),
    'data-section-column-spacing': String(attributes.columnSpacing),
    'data-section-column-separator': String(attributes.columnSeparator),
    'data-section-column-layout': attributes.columnLayout,
    'data-section-break-after': attributes.breakAfter,
    'data-section-header-text': attributes.headerText,
    'data-section-footer-text': attributes.footerText,
    'data-section-show-page-numbers': String(attributes.showPageNumbers),
    'data-section-page-number-start': attributes.pageNumberStart === null ? '' : String(attributes.pageNumberStart),
    'data-section-page-chrome': attributes.pageChrome,
  };
}

export function documentSectionLayoutFromElement(
  element: HTMLElement,
  fallback: WorkDocumentSectionLayout
): WorkDocumentSectionLayout {
  return documentSectionLayoutFromNodeAttributes(
    {
      pageSize: element.dataset.sectionPageSize as WorkDocumentSectionLayout['pageSize'],
      orientation: element.dataset.sectionOrientation as WorkDocumentSectionLayout['orientation'],
      marginTop: numberValue(element.dataset.sectionMarginTop),
      marginRight: numberValue(element.dataset.sectionMarginRight),
      marginBottom: numberValue(element.dataset.sectionMarginBottom),
      marginLeft: numberValue(element.dataset.sectionMarginLeft),
      columnCount: numberValue(element.dataset.sectionColumnCount),
      columnSpacing: numberValue(element.dataset.sectionColumnSpacing),
      columnSeparator: element.dataset.sectionColumnSeparator === 'true',
      columnLayout: element.dataset.sectionColumnLayout ?? '',
      breakAfter: element.dataset.sectionBreakAfter as WorkDocumentSectionBreakType,
      headerText: element.dataset.sectionHeaderText ?? '',
      footerText: element.dataset.sectionFooterText ?? '',
      showPageNumbers: element.dataset.sectionShowPageNumbers === 'true',
      pageNumberStart: numberValue(element.dataset.sectionPageNumberStart) ?? null,
      pageChrome: element.dataset.sectionPageChrome ?? '',
    },
    fallback
  );
}

export function syncDocumentContentFromHtml(content: WorkDocumentContent, html: string): WorkDocumentContent {
  const normalized = normalizeDocumentHtml({ ...content, html });
  const first = documentSections({ ...content, html: normalized })[0]?.layout ?? documentInitialSectionLayout(content);
  return {
    ...content,
    html: normalized,
    pageSize: first.pageSize,
    orientation: first.orientation,
    margins: first.margins,
    columns: first.columns,
    headerText: first.headerText,
    footerText: first.footerText,
    showPageNumbers: first.showPageNumbers,
    pageNumberStart: first.pageNumberStart,
    pageChrome: first.pageChrome,
  };
}

export function documentContentLayoutProperties(
  layout: WorkDocumentSectionLayout
): Omit<WorkDocumentContent, 'type' | 'html'> {
  return {
    pageSize: layout.pageSize,
    orientation: layout.orientation,
    margins: layout.margins,
    columns: layout.columns,
    headerText: layout.headerText,
    footerText: layout.footerText,
    showPageNumbers: layout.showPageNumbers,
    pageNumberStart: layout.pageNumberStart,
    pageChrome: layout.pageChrome,
  };
}

function normalizeDocumentSemanticHtml(source: string, bibliography: WorkDocumentContent['bibliography']): string {
  return normalizeDocumentCitationsHtml(
    normalizeDocumentFieldsHtml(normalizeDocumentCaptionsHtml(normalizeDocumentNotesHtml(source))),
    bibliography
  );
}

function applyDocumentSectionDomAttributes(element: HTMLElement, layout: WorkDocumentSectionLayout, id: string) {
  for (const [name, value] of Object.entries(documentSectionDomAttributes(layout, id))) {
    element.setAttribute(name, value);
  }
}

function validBreakType(value: string | undefined): WorkDocumentSectionBreakType {
  if (value === 'continuous' || value === 'evenPage' || value === 'oddPage' || value === 'nextColumn') return value;
  return 'nextPage';
}

function validPageNumber(value: number | null | undefined): number | undefined {
  return Number.isFinite(value) && Number(value) > 0 ? Math.min(9999, Math.round(Number(value))) : undefined;
}

function finiteNumber(value: number | null | undefined, fallback: number): number {
  return Number.isFinite(value) ? Number(value) : fallback;
}

function numberValue(value: string | undefined): number | undefined {
  if (!value?.trim()) return undefined;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}
