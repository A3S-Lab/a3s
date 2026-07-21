import { documentSections } from './work-document-section';
import { resolveDocumentFieldsHtml } from './work-document-fields';
import {
  collectDocumentNotes,
  documentNoteKey,
  documentNoteReferenceKeys,
  removeDocumentNoteDefinitions,
  type WorkDocumentNote,
} from './work-document-notes';
import type { WorkDocumentColumns, WorkDocumentContent, WorkDocumentSectionLayout } from './work-types';

export interface WorkDocumentPageSegment {
  sectionId: string;
  html: string;
  columns: WorkDocumentColumns;
}

export interface WorkDocumentPageDescriptor {
  key: string;
  layout: WorkDocumentSectionLayout;
  segments: WorkDocumentPageSegment[];
  pageNumber: number;
  physicalPage: number;
  sectionPage: number;
  blank: boolean;
  footnotes: WorkDocumentNote[];
  endnotes: WorkDocumentNote[];
}

export function documentPageDescriptors(content: WorkDocumentContent): WorkDocumentPageDescriptor[] {
  const noteCollection = collectDocumentNotes(content.html);
  const canonicalContent = { ...content, html: noteCollection.html };
  const sections = documentSections(canonicalContent);
  const pages: WorkDocumentPageDescriptor[] = [];
  let nextPageNumber = sections[0]?.layout.pageNumberStart ?? 1;
  let currentPage: WorkDocumentPageDescriptor | null = null;

  sections.forEach((section, sectionIndex) => {
    const parts = safeSectionPages(section.html);
    const previousBreak = sectionIndex > 0 ? sections[sectionIndex - 1].layout.breakAfter : null;
    const restartsNumbering = section.layout.pageNumberStart !== undefined;
    const continuesCurrentPage =
      sectionIndex > 0 &&
      !restartsNumbering &&
      (previousBreak === 'continuous' || previousBreak === 'nextColumn') &&
      currentPage !== null &&
      samePhysicalPageLayout(currentPage.layout, section.layout);
    let sectionPage = continuesCurrentPage ? 1 : 0;

    if (!continuesCurrentPage) {
      if (previousBreak === 'evenPage' || previousBreak === 'oddPage') {
        const desiredParity = previousBreak === 'evenPage' ? 0 : 1;
        const nextPhysicalPage = pages.length + 1;
        if (nextPhysicalPage % 2 !== desiredParity) {
          pages.push(createPage(section.layout, nextPageNumber, pages.length + 1, 0, true));
          nextPageNumber += 1;
        }
      }
      if (restartsNumbering) nextPageNumber = section.layout.pageNumberStart ?? nextPageNumber;
      sectionPage = 1;
      currentPage = createPage(section.layout, nextPageNumber, pages.length + 1, sectionPage, false);
      pages.push(currentPage);
      nextPageNumber += 1;
    }

    parts.forEach((html, partIndex) => {
      if (partIndex > 0) {
        sectionPage += 1;
        currentPage = createPage(section.layout, nextPageNumber, pages.length + 1, sectionPage, false);
        pages.push(currentPage);
        nextPageNumber += 1;
      }
      currentPage?.segments.push({
        sectionId: section.id,
        html,
        columns: section.layout.columns,
      });
    });
  });

  const result = pages.length
    ? pages
    : [createPage(documentSections(content)[0]?.layout, content.pageNumberStart ?? 1, 1, 1, false)];
  resolveDocumentPageFields(result, sections);
  attachDocumentNotes(result, noteCollection.notes);
  return result;
}

function resolveDocumentPageFields(
  pages: WorkDocumentPageDescriptor[],
  sections: ReturnType<typeof documentSections>
): void {
  const sectionNumbers = new Map(sections.map((section, index) => [section.id, index + 1] as const));
  const sectionPages = new Map<string, number>();
  for (const page of pages) {
    for (const sectionId of new Set(page.segments.map((segment) => segment.sectionId))) {
      sectionPages.set(sectionId, (sectionPages.get(sectionId) ?? 0) + 1);
    }
  }
  const now = new Date();
  for (const page of pages) {
    for (const segment of page.segments) {
      segment.html = resolveDocumentFieldsHtml(segment.html, {
        pageNumber: page.pageNumber,
        totalPages: pages.length,
        sectionNumber: sectionNumbers.get(segment.sectionId) ?? 1,
        sectionPages: sectionPages.get(segment.sectionId) ?? 1,
        now,
      });
    }
  }
}

function samePhysicalPageLayout(current: WorkDocumentSectionLayout, next: WorkDocumentSectionLayout): boolean {
  return (
    current.pageSize === next.pageSize &&
    current.orientation === next.orientation &&
    current.margins.top === next.margins.top &&
    current.margins.right === next.margins.right &&
    current.margins.bottom === next.margins.bottom &&
    current.margins.left === next.margins.left
  );
}

function createPage(
  layout: WorkDocumentSectionLayout | undefined,
  pageNumber: number,
  physicalPage: number,
  sectionPage: number,
  blank: boolean
): WorkDocumentPageDescriptor {
  const effectiveLayout =
    layout ??
    ({
      pageSize: 'a4',
      orientation: 'portrait',
      margins: { top: 25, right: 23, bottom: 25, left: 23 },
      columns: { count: 1, spacing: 12, separator: false },
      breakAfter: 'nextPage',
    } satisfies WorkDocumentSectionLayout);
  return {
    key: `document-page-${physicalPage}`,
    layout: effectiveLayout,
    segments: [],
    pageNumber,
    physicalPage,
    sectionPage,
    blank,
    footnotes: [],
    endnotes: [],
  };
}

function safeSectionPages(source: string): string[] {
  const document = new DOMParser().parseFromString(removeDocumentNoteDefinitions(source), 'text/html');
  sanitizeDocument(document);
  const pages: HTMLElement[] = [document.createElement('div')];
  for (const node of Array.from(document.body.childNodes)) {
    if (node instanceof Element && node.hasAttribute('data-page-break')) {
      pages.push(document.createElement('div'));
      continue;
    }
    pages.at(-1)?.append(node.cloneNode(true));
  }
  return pages.map((page) => page.innerHTML);
}

function sanitizeDocument(document: Document) {
  for (const element of Array.from(document.body.querySelectorAll('script, iframe, object, embed, link, meta'))) {
    element.remove();
  }
  for (const element of Array.from(document.body.querySelectorAll('*'))) {
    for (const attribute of Array.from(element.attributes)) {
      if (attribute.name.toLowerCase().startsWith('on')) element.removeAttribute(attribute.name);
    }
    if (
      element instanceof HTMLAnchorElement &&
      /^(?:javascript|vbscript|data):/i.test(element.getAttribute('href')?.trim() ?? '')
    ) {
      element.removeAttribute('href');
    }
  }
}

function attachDocumentNotes(pages: WorkDocumentPageDescriptor[], notes: WorkDocumentNote[]) {
  const safeNotes = notes.map((note) => ({ ...note, html: safeNoteHtml(note.html) }));
  const byKey = new Map(safeNotes.map((note) => [documentNoteKey(note.kind, note.id), note] as const));
  for (const page of pages) {
    const keys = new Set(page.segments.flatMap((segment) => documentNoteReferenceKeys(segment.html)));
    page.footnotes = Array.from(keys)
      .map((key) => byKey.get(key))
      .filter((note): note is WorkDocumentNote => note?.kind === 'footnote');
  }
  const lastContentPage = [...pages].reverse().find((page) => !page.blank);
  if (lastContentPage) lastContentPage.endnotes = safeNotes.filter((note) => note.kind === 'endnote');
}

function safeNoteHtml(source: string): string {
  const document = new DOMParser().parseFromString(source, 'text/html');
  sanitizeDocument(document);
  return document.body.innerHTML;
}
