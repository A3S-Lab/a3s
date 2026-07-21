import { documentCaptionLabel, type WorkDocumentCaptionKind } from './work-document-captions';
import { docxCaptionBookmark, docxCaptionReferenceTarget, docxCaptionSequenceKind } from './work-docx-caption-fields';
import { docxFieldOccurrences, type DocxFieldOccurrence } from './work-docx-field-instructions';
import { attribute, descendants } from './work-ooxml-package';

export interface ImportedDocxCaptionMarkers {
  captions: ImportedCaptionMarker[];
  references: ImportedReferenceMarker[];
}

interface ImportedCaptionMarker {
  start: string;
  end: string;
  id: string;
  kind: WorkDocumentCaptionKind;
}

interface ImportedReferenceMarker {
  start: string;
  end: string;
  targetId: string;
}

const WORD_NAMESPACE = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main';
const XML_NAMESPACE = 'http://www.w3.org/XML/1998/namespace';

export function markDocxCaptionFields(document: Document): ImportedDocxCaptionMarkers {
  const captions: ImportedCaptionMarker[] = [];
  const bookmarkTargets = new Map<string, string>();
  const paragraphs = descendants(document, 'p');
  for (const paragraph of paragraphs) {
    const sequence = docxFieldOccurrences(paragraph)
      .map((field) => ({ field, kind: docxCaptionSequenceKind(field.instruction) }))
      .find((item): item is { field: DocxFieldOccurrence; kind: WorkDocumentCaptionKind } => Boolean(item.kind));
    if (!sequence) continue;
    const bookmark = docxCaptionBookmark(paragraph, sequence.field);
    const bookmarkName = attribute(bookmark ?? paragraph, 'name')?.trim() ?? '';
    const id = uniqueCaptionId(bookmarkName, sequence.kind, captions.length + 1, captions);
    if (bookmarkName) bookmarkTargets.set(bookmarkName, id);
    const marker = captionMarker(captions.length + 1, id, sequence.kind);
    insertParagraphBoundaryMarkers(document, paragraph, marker.start, marker.end);
    captions.push(marker);
  }

  const references: ImportedReferenceMarker[] = [];
  for (const paragraph of paragraphs) {
    for (const field of docxFieldOccurrences(paragraph)) {
      const bookmarkName = docxCaptionReferenceTarget(field.instruction);
      const targetId = bookmarkName ? bookmarkTargets.get(bookmarkName) : undefined;
      if (!targetId) continue;
      const marker = referenceMarker(references.length + 1, targetId);
      insertFieldBoundaryMarkers(document, field, marker.start, marker.end);
      references.push(marker);
    }
  }
  return { captions, references };
}

export function applyImportedDocxCaptionMarkers(document: Document, markers: ImportedDocxCaptionMarkers): void {
  for (const reference of markers.references) {
    const element = document.createElement('span');
    element.dataset.documentCrossReference = 'true';
    element.dataset.referenceTargetId = reference.targetId;
    replaceMarkerRange(document.body, reference.start, reference.end, element);
  }
  for (const caption of markers.captions) {
    const block = markerBlock(document.body, caption.start, caption.end);
    if (!block) continue;
    removeMarkerText(block, caption.start);
    removeMarkerText(block, caption.end);
    removeCaptionPrefix(block, caption.kind);
    const element = document.createElement('figcaption');
    element.dataset.documentCaption = 'true';
    element.dataset.captionId = caption.id;
    element.dataset.captionKind = caption.kind;
    element.className = 'work-document-caption';
    element.append(...Array.from(block.childNodes));
    block.replaceWith(element);
  }
}

export function hasImportedDocxCaptionMarkers(markers: ImportedDocxCaptionMarkers): boolean {
  return markers.captions.length > 0 || markers.references.length > 0;
}

function uniqueCaptionId(
  bookmark: string,
  kind: WorkDocumentCaptionKind,
  index: number,
  existing: ImportedCaptionMarker[]
): string {
  const base = bookmark ? `docx-caption-${bookmark}` : `docx-${kind}-caption-${index}`;
  const ids = new Set(existing.map((caption) => caption.id));
  if (!ids.has(base)) return base;
  let suffix = 2;
  while (ids.has(`${base}-${suffix}`)) suffix += 1;
  return `${base}-${suffix}`;
}

function captionMarker(index: number, id: string, kind: WorkDocumentCaptionKind): ImportedCaptionMarker {
  return {
    start: `__A3S_WORK_CAPTION_START_${index}__`,
    end: `__A3S_WORK_CAPTION_END_${index}__`,
    id,
    kind,
  };
}

function referenceMarker(index: number, targetId: string): ImportedReferenceMarker {
  return {
    start: `__A3S_WORK_REFERENCE_START_${index}__`,
    end: `__A3S_WORK_REFERENCE_END_${index}__`,
    targetId,
  };
}

function insertParagraphBoundaryMarkers(document: Document, paragraph: Element, start: string, end: string): void {
  const firstContent = Array.from(paragraph.children).find((element) => element.localName !== 'pPr');
  paragraph.insertBefore(markerRun(document, start), firstContent ?? null);
  paragraph.append(markerRun(document, end));
}

function insertFieldBoundaryMarkers(document: Document, field: DocxFieldOccurrence, start: string, end: string): void {
  if (field.start.localName === 'fldChar' && field.end.localName === 'fldChar') {
    field.start.parentNode?.insertBefore(markerText(document, start), field.start);
    field.end.parentNode?.insertBefore(markerText(document, end), field.end.nextSibling);
    return;
  }
  field.start.parentNode?.insertBefore(markerRun(document, start), field.start);
  field.end.parentNode?.insertBefore(markerRun(document, end), field.end.nextSibling);
}

function markerRun(document: Document, text: string): Element {
  const run = document.createElementNS(WORD_NAMESPACE, 'w:r');
  run.append(markerText(document, text));
  return run;
}

function markerText(document: Document, text: string): Element {
  const value = document.createElementNS(WORD_NAMESPACE, 'w:t');
  value.setAttributeNS(XML_NAMESPACE, 'xml:space', 'preserve');
  value.textContent = text;
  return value;
}

function markerBlock(root: HTMLElement, start: string, end: string): HTMLElement | null {
  const startNode = textNodes(root).find((node) => node.data.includes(start));
  const endNode = textNodes(root).find((node) => node.data.includes(end));
  if (!startNode || !endNode) return null;
  let current = startNode.parentElement;
  while (current && current !== root) {
    if (current.contains(endNode) && /^(?:P|DIV|LI)$/.test(current.tagName)) return current;
    current = current.parentElement;
  }
  return null;
}

function replaceMarkerRange(root: HTMLElement, start: string, end: string, replacement: HTMLElement): boolean {
  const nodes = textNodes(root);
  const startNode = nodes.find((node) => node.data.includes(start));
  const endNode = nodes.find((node) => node.data.includes(end));
  if (!startNode || !endNode) return false;
  const range = root.ownerDocument.createRange();
  range.setStart(startNode, startNode.data.indexOf(start));
  range.setEnd(endNode, endNode.data.indexOf(end) + end.length);
  range.deleteContents();
  range.insertNode(replacement);
  return true;
}

function removeMarkerText(root: HTMLElement, marker: string): void {
  for (const node of textNodes(root)) node.data = node.data.replace(marker, '');
}

function removeCaptionPrefix(root: HTMLElement, kind: WorkDocumentCaptionKind): void {
  const label = documentCaptionLabel(kind);
  const source = root.textContent ?? '';
  const expression =
    kind === 'table'
      ? /^\s*(?:表|Table)\s*(?:\d+)?[\s:：.\-–—　]*/i
      : /^\s*(?:图|Figure|Fig\.?)\s*(?:\d+)?[\s:：.\-–—　]*/i;
  const match = expression.exec(source);
  if (!match || (!source.trimStart().startsWith(label) && !/^(?:Figure|Fig|Table)/i.test(source.trimStart()))) return;
  let remaining = match[0].length;
  for (const node of textNodes(root)) {
    if (remaining <= 0) break;
    const removed = Math.min(remaining, node.data.length);
    node.data = node.data.slice(removed);
    remaining -= removed;
  }
}

function textNodes(root: ParentNode): Text[] {
  const walker = root.ownerDocument?.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const nodes: Text[] = [];
  if (!walker) return nodes;
  while (walker.nextNode()) nodes.push(walker.currentNode as Text);
  return nodes;
}
