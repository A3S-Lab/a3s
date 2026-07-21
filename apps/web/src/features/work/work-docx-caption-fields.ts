import type { WorkDocumentCaptionKind } from './work-document-captions';
import type { DocxFieldOccurrence } from './work-docx-field-instructions';
import { attribute, descendants } from './work-ooxml-package';

export function docxCaptionSequenceKind(instruction: string): WorkDocumentCaptionKind | null {
  const identifier = /^\s*SEQ\s+([^\s\\]+)/i.exec(instruction)?.[1]?.toLowerCase();
  if (!identifier) return null;
  if (identifier === 'table' || identifier === '表') return 'table';
  if (identifier === 'figure' || identifier === 'fig' || identifier === '图') return 'figure';
  return null;
}

export function docxCaptionReferenceTarget(instruction: string): string | null {
  return /^\s*REF\s+([^\s\\]+)/i.exec(instruction)?.[1] ?? null;
}

export function docxCaptionBookmark(paragraph: Element, field: DocxFieldOccurrence): Element | undefined {
  const bookmarks = descendants(paragraph, 'bookmarkStart').filter(
    (bookmark) => !/^_(?:GoBack|Toc)/i.test(attribute(bookmark, 'name') ?? '')
  );
  const enclosing = bookmarks.filter((bookmark) => bookmarkContains(paragraph, bookmark, field));
  if (enclosing.length) return enclosing.at(-1);
  return bookmarks.filter((bookmark) => precedes(bookmark, field.start)).at(-1) ?? bookmarks[0];
}

function bookmarkContains(paragraph: Element, start: Element, field: DocxFieldOccurrence): boolean {
  const id = attribute(start, 'id');
  if (!id) return false;
  const end = descendants(paragraph, 'bookmarkEnd').find((bookmark) => attribute(bookmark, 'id') === id);
  return Boolean(end && precedes(start, field.start) && precedes(field.end, end));
}

function precedes(candidate: Element, target: Element): boolean {
  return Boolean(candidate.compareDocumentPosition(target) & Node.DOCUMENT_POSITION_FOLLOWING);
}
