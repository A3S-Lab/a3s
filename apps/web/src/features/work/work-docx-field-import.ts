import { docxFieldOccurrences, docxFieldResultText, type DocxFieldOccurrence } from './work-docx-field-instructions';
import { docxDocumentFieldKind, type WorkDocumentFieldKind } from './work-document-fields';

export interface ImportedDocxFieldMarkers {
  fields: ImportedDocxFieldMarker[];
}

interface ImportedDocxFieldMarker {
  start: string;
  end: string;
  id: string;
  kind: WorkDocumentFieldKind;
  instruction: string;
  display: string;
}

const WORD_NAMESPACE = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main';
const XML_NAMESPACE = 'http://www.w3.org/XML/1998/namespace';

export function markDocxBodyFields(document: Document): ImportedDocxFieldMarkers {
  const fields = docxFieldOccurrences(document).flatMap((field, index) => {
    const kind = docxDocumentFieldKind(field.instruction);
    if (!kind || !canMarkField(field)) return [];
    const marker: ImportedDocxFieldMarker = {
      start: `__A3S_WORK_FIELD_START_${index + 1}__`,
      end: `__A3S_WORK_FIELD_END_${index + 1}__`,
      id: `docx-field-${index + 1}`,
      kind,
      instruction: field.instruction,
      display: docxFieldResultText(field),
    };
    insertFieldBoundaryMarkers(document, field, marker.start, marker.end);
    return [marker];
  });
  return { fields };
}

export function applyImportedDocxFieldMarkers(document: Document, markers: ImportedDocxFieldMarkers): void {
  for (const field of markers.fields) {
    const element = document.createElement('span');
    element.dataset.documentField = 'true';
    element.dataset.fieldId = field.id;
    element.dataset.fieldKind = field.kind;
    element.dataset.fieldInstruction = field.instruction;
    const convertedDisplay = replaceMarkerRange(document.body, field.start, field.end, element);
    const display = field.display.trim() || convertedDisplay.trim();
    element.dataset.fieldDisplay = display;
    element.textContent = display;
  }
}

export function hasImportedDocxFieldMarkers(markers: ImportedDocxFieldMarkers): boolean {
  return markers.fields.length > 0;
}

function canMarkField(field: DocxFieldOccurrence): boolean {
  return field.start !== field.end || field.start.localName === 'fldSimple';
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

function replaceMarkerRange(root: HTMLElement, start: string, end: string, replacement: HTMLElement): string {
  const nodes = textNodes(root);
  const startNode = nodes.find((node) => node.data.includes(start));
  const endNode = nodes.find((node) => node.data.includes(end));
  if (!startNode || !endNode) return '';
  const range = root.ownerDocument.createRange();
  range.setStart(startNode, startNode.data.indexOf(start));
  range.setEnd(endNode, endNode.data.indexOf(end) + end.length);
  const content = range.cloneContents();
  removeMarkerText(content, start);
  removeMarkerText(content, end);
  const display = content.textContent ?? '';
  range.deleteContents();
  range.insertNode(replacement);
  return display;
}

function removeMarkerText(root: ParentNode, marker: string): void {
  for (const node of textNodes(root)) node.data = node.data.replace(marker, '');
}

function textNodes(root: ParentNode): Text[] {
  const document = root.ownerDocument;
  const walker = document?.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const nodes: Text[] = [];
  if (!walker) return nodes;
  while (walker.nextNode()) nodes.push(walker.currentNode as Text);
  return nodes;
}
