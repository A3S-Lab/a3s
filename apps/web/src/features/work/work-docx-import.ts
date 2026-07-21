import JSZip from 'jszip';
import {
  documentContentLayoutProperties,
  documentInitialSectionLayout,
  documentSectionDomAttributes,
} from './work-document-section';
import { readDocxBibliography } from './work-docx-bibliography';
import { normalizeDocumentCaptionsHtml } from './work-document-captions';
import { normalizeDocumentCitationsHtml } from './work-document-citations';
import { normalizeDocumentFieldsHtml } from './work-document-fields';
import { normalizeDocumentNotesHtml } from './work-document-notes';
import {
  applyImportedDocxCaptionMarkers,
  hasImportedDocxCaptionMarkers,
  markDocxCaptionFields,
  type ImportedDocxCaptionMarkers,
} from './work-docx-caption-import';
import {
  applyImportedDocxChangeMarkers,
  hasImportedDocxChangeMarkers,
  markDocxTextChanges,
  type ImportedDocxChangeMarkers,
} from './work-docx-change-import';
import {
  applyImportedDocxCitationMarkers,
  hasImportedDocxCitationMarkers,
  markDocxCitationFields,
  type ImportedDocxCitationMarkers,
} from './work-docx-citation-import';
import {
  applyImportedDocxCommentMarkers,
  hasImportedDocxCommentMarkers,
  markDocxComments,
  type ImportedDocxCommentMarkers,
} from './work-docx-comment-import';
import { importDocxColumns } from './work-docx-column-import';
import {
  applyImportedDocxFieldMarkers,
  hasImportedDocxFieldMarkers,
  markDocxBodyFields,
  type ImportedDocxFieldMarkers,
} from './work-docx-field-import';
import { extractMammothDocumentNotes, placeMammothDocumentNotes } from './work-docx-note-import';
import { documentUsesOddEvenPageChrome, importSectionPageChrome } from './work-docx-page-chrome-import';
import { attribute, descendants, firstDescendant, OoxmlPackage } from './work-ooxml-package';
import type {
  WorkDocumentContent,
  WorkDocumentMargins,
  WorkDocumentSectionBreakType,
  WorkDocumentSectionLayout,
} from './work-types';

type ImportedDocumentLayout = Omit<WorkDocumentContent, 'type' | 'html'>;

export interface PreparedDocxImport {
  conversionBuffer: ArrayBuffer;
  sections: Array<{ id: string; layout: WorkDocumentSectionLayout }>;
  captionMarkers: ImportedDocxCaptionMarkers;
  changeMarkers: ImportedDocxChangeMarkers;
  commentMarkers: ImportedDocxCommentMarkers;
  fieldMarkers: ImportedDocxFieldMarkers;
  citationMarkers: ImportedDocxCitationMarkers;
  bibliography?: WorkDocumentContent['bibliography'];
  trackChanges: boolean;
}

const TWIPS_PER_MILLIMETER = 1440 / 25.4;
const WORD_NAMESPACE = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main';
const XML_NAMESPACE = 'http://www.w3.org/XML/1998/namespace';

export async function prepareDocxImport(buffer: ArrayBuffer): Promise<PreparedDocxImport> {
  const archive = await OoxmlPackage.load(buffer);
  const bibliography = (await readDocxBibliography(archive)).bibliography;
  const fallback = documentInitialSectionLayout({ type: 'document', html: '<p></p>', pageSize: 'a4' });
  if (!archive.has('word/document.xml')) {
    return {
      conversionBuffer: buffer,
      sections: [{ id: 'document-section-1', layout: fallback }],
      captionMarkers: { captions: [], references: [] },
      changeMarkers: { changes: [] },
      commentMarkers: { comments: [], ranges: [] },
      fieldMarkers: { fields: [] },
      citationMarkers: { citations: [], bibliographies: [] },
      bibliography,
      trackChanges: false,
    };
  }

  const document = await archive.xml('word/document.xml');
  const commentMarkers = await markDocxComments(document, archive);
  const changeMarkers = markDocxTextChanges(document);
  const captionMarkers = markDocxCaptionFields(document);
  const citationMarkers = markDocxCitationFields(document);
  const fieldMarkers = markDocxBodyFields(document);
  const settings = archive.has('word/settings.xml') ? await archive.xml('word/settings.xml') : null;
  const trackChanges =
    Boolean(settings && firstDescendant(settings, 'trackRevisions')) || changeMarkers.changes.length > 0;
  const sectionElements = effectiveSectionProperties(document);
  if (!sectionElements.length) {
    return {
      conversionBuffer:
        hasImportedDocxCaptionMarkers(captionMarkers) ||
        hasImportedDocxChangeMarkers(changeMarkers) ||
        hasImportedDocxCommentMarkers(commentMarkers) ||
        hasImportedDocxCitationMarkers(citationMarkers) ||
        hasImportedDocxFieldMarkers(fieldMarkers)
          ? await writeDocumentXml(buffer, document)
          : buffer,
      sections: [{ id: 'document-section-1', layout: fallback }],
      captionMarkers,
      changeMarkers,
      commentMarkers,
      fieldMarkers,
      citationMarkers,
      bibliography,
      trackChanges,
    };
  }
  const relationships = await archive.relationships('word/document.xml');
  const oddEvenPageChrome = documentUsesOddEvenPageChrome(settings);
  const sections: PreparedDocxImport['sections'] = [];
  let previous = fallback;
  for (const [index, element] of sectionElements.entries()) {
    const layout = await parseSectionLayout(element, archive, relationships, previous, oddEvenPageChrome);
    sections.push({ id: `document-section-${index + 1}`, layout });
    previous = layout;
  }
  if (sections.length > 1) addSectionMarkers(document, sectionElements);
  return {
    conversionBuffer:
      sections.length > 1 ||
      hasImportedDocxCaptionMarkers(captionMarkers) ||
      hasImportedDocxChangeMarkers(changeMarkers) ||
      hasImportedDocxCommentMarkers(commentMarkers) ||
      hasImportedDocxCitationMarkers(citationMarkers) ||
      hasImportedDocxFieldMarkers(fieldMarkers)
        ? await writeDocumentXml(buffer, document)
        : buffer,
    sections,
    captionMarkers,
    changeMarkers,
    commentMarkers,
    fieldMarkers,
    citationMarkers,
    bibliography,
    trackChanges,
  };
}

export function applyDocxSectionsToHtml(
  html: string,
  sections: PreparedDocxImport['sections'],
  captionMarkers: ImportedDocxCaptionMarkers = { captions: [], references: [] },
  changeMarkers: ImportedDocxChangeMarkers = { changes: [] },
  commentMarkers: ImportedDocxCommentMarkers = { comments: [], ranges: [] },
  fieldMarkers: ImportedDocxFieldMarkers = { fields: [] },
  citationMarkers: ImportedDocxCitationMarkers = { citations: [], bibliographies: [] },
  bibliography?: WorkDocumentContent['bibliography']
): string {
  const document = new DOMParser().parseFromString(html, 'text/html');
  applyImportedDocxCaptionMarkers(document, captionMarkers);
  applyImportedDocxCitationMarkers(document, citationMarkers);
  applyImportedDocxFieldMarkers(document, fieldMarkers);
  applyImportedDocxChangeMarkers(document, changeMarkers);
  applyImportedDocxCommentMarkers(document, commentMarkers);
  const notes = extractMammothDocumentNotes(document);
  const sourceNodes = Array.from(document.body.childNodes);
  document.body.replaceChildren();
  let sectionIndex = 0;
  let section = createHtmlSection(document, sections[sectionIndex]);

  for (const node of sourceNodes) {
    if (isSectionMarker(node, sectionIndex)) {
      ensureSectionContent(section);
      document.body.append(section);
      sectionIndex += 1;
      section = createHtmlSection(document, sections[sectionIndex] ?? sections.at(-1));
      continue;
    }
    section.append(node);
  }
  ensureSectionContent(section);
  document.body.append(section);
  while (sectionIndex + 1 < sections.length) {
    sectionIndex += 1;
    const missing = createHtmlSection(document, sections[sectionIndex]);
    ensureSectionContent(missing);
    document.body.append(missing);
  }
  placeMammothDocumentNotes(document, notes);
  return normalizeDocumentCitationsHtml(
    normalizeDocumentFieldsHtml(normalizeDocumentCaptionsHtml(normalizeDocumentNotesHtml(document.body.innerHTML))),
    bibliography
  );
}

export async function readDocxLayout(buffer: ArrayBuffer): Promise<ImportedDocumentLayout> {
  const prepared = await prepareDocxImport(buffer);
  return documentContentLayoutProperties(prepared.sections[0].layout);
}

async function parseSectionLayout(
  section: Element,
  archive: OoxmlPackage,
  relationships: Awaited<ReturnType<OoxmlPackage['relationships']>>,
  previous: WorkDocumentSectionLayout,
  oddEvenPageChrome: boolean
): Promise<WorkDocumentSectionLayout> {
  const pageSize = firstDescendant(section, 'pgSz');
  const width = numberAttribute(pageSize, 'w');
  const height = numberAttribute(pageSize, 'h');
  const orientation =
    attribute(pageSize ?? section, 'orient') === 'landscape' || (width > 0 && height > 0 && width > height)
      ? 'landscape'
      : pageSize
        ? 'portrait'
        : previous.orientation;
  const shortEdge = Math.min(width || 11_906, height || 16_838);
  const size = pageSize
    ? Math.abs(shortEdge - 12_240) < Math.abs(shortEdge - 11_906)
      ? 'letter'
      : 'a4'
    : previous.pageSize;
  const marginsElement = firstDescendant(section, 'pgMar');
  const columnsElement = firstDescendant(section, 'cols');
  const pageChrome = await importSectionPageChrome(section, archive, relationships, previous, oddEvenPageChrome);
  const pageNumberStart = numberAttribute(firstDescendant(section, 'pgNumType'), 'start');
  return {
    pageSize: size,
    orientation,
    margins: marginsElement ? parseMargins(marginsElement, previous.margins) : { ...previous.margins },
    columns: columnsElement ? importDocxColumns(columnsElement, previous.columns) : { ...previous.columns },
    breakAfter: parseSectionBreak(firstDescendant(section, 'type')),
    ...pageChrome,
    pageNumberStart: pageNumberStart > 0 ? pageNumberStart : undefined,
  };
}

function addSectionMarkers(document: Document, sectionElements: Element[]): void {
  for (let index = 0; index < sectionElements.length - 1; index += 1) {
    const paragraph = closestAncestor(sectionElements[index], 'p');
    if (!paragraph?.parentNode) continue;
    const marker = document.createElementNS(WORD_NAMESPACE, 'w:p');
    const run = document.createElementNS(WORD_NAMESPACE, 'w:r');
    const text = document.createElementNS(WORD_NAMESPACE, 'w:t');
    text.setAttributeNS(XML_NAMESPACE, 'xml:space', 'preserve');
    text.textContent = sectionMarker(index);
    run.append(text);
    marker.append(run);
    paragraph.parentNode.insertBefore(marker, paragraph.nextSibling);
  }
}

async function writeDocumentXml(buffer: ArrayBuffer, document: Document): Promise<ArrayBuffer> {
  const zip = await JSZip.loadAsync(buffer);
  zip.file('word/document.xml', new XMLSerializer().serializeToString(document));
  return zip.generateAsync({ type: 'arraybuffer' });
}

function effectiveSectionProperties(document: Document): Element[] {
  return descendants(document, 'sectPr').filter((element) => !closestAncestor(element, 'sectPrChange'));
}

function createHtmlSection(
  document: Document,
  section: PreparedDocxImport['sections'][number] | undefined
): HTMLElement {
  const element = document.createElement('section');
  const fallback = documentInitialSectionLayout({ type: 'document', html: '<p></p>', pageSize: 'a4' });
  const layout = section?.layout ?? fallback;
  const id = section?.id ?? 'document-section';
  for (const [name, value] of Object.entries(documentSectionDomAttributes(layout, id))) {
    element.setAttribute(name, value);
  }
  return element;
}

function ensureSectionContent(section: HTMLElement) {
  if (!section.childNodes.length) section.innerHTML = '<p></p>';
}

function isSectionMarker(node: ChildNode, index: number): boolean {
  return node instanceof HTMLElement && node.textContent?.trim() === sectionMarker(index);
}

function sectionMarker(index: number): string {
  return `__A3S_WORK_DOCUMENT_SECTION_${index + 1}__`;
}

function parseMargins(element: Element, fallback: WorkDocumentMargins): WorkDocumentMargins {
  return {
    top: twipsToMillimeters(numberAttribute(element, 'top'), fallback.top),
    right: twipsToMillimeters(numberAttribute(element, 'right'), fallback.right),
    bottom: twipsToMillimeters(numberAttribute(element, 'bottom'), fallback.bottom),
    left: twipsToMillimeters(numberAttribute(element, 'left'), fallback.left),
  };
}

function parseSectionBreak(element: Element | undefined): WorkDocumentSectionBreakType {
  if (!element) return 'nextPage';
  const value = attribute(element, 'val');
  if (value === 'continuous' || value === 'evenPage' || value === 'oddPage' || value === 'nextColumn') return value;
  return 'nextPage';
}

function numberAttribute(element: Element | undefined, name: string): number {
  if (!element) return 0;
  const value = Number(attribute(element, name));
  return Number.isFinite(value) ? value : 0;
}

function closestAncestor(element: Element, localName: string): Element | null {
  let current: Element | null = element;
  while (current) {
    if (current.localName === localName) return current;
    current = current.parentElement;
  }
  return null;
}

function twipsToMillimeters(value: number, fallback: number): number {
  if (value <= 0) return fallback;
  return Math.round((value / TWIPS_PER_MILLIMETER) * 10) / 10;
}
