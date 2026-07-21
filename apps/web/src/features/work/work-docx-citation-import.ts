import { docxFieldOccurrences, docxFieldResultText, type DocxFieldOccurrence } from './work-docx-field-instructions';
import { documentCitationTagsFromInstruction } from './work-document-citations';

export interface ImportedDocxCitationMarkers {
  citations: ImportedDocxCitationMarker[];
  bibliographies: ImportedDocxBibliographyMarker[];
}

interface ImportedDocxCitationMarker {
  start: string;
  end: string;
  id: string;
  instruction: string;
  tags: string[];
  display: string;
}

interface ImportedDocxBibliographyMarker {
  start: string;
  end: string;
  id: string;
  instruction: string;
  display: string;
}

const WORD_NAMESPACE = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main';
const XML_NAMESPACE = 'http://www.w3.org/XML/1998/namespace';

export function markDocxCitationFields(document: Document): ImportedDocxCitationMarkers {
  const citations: ImportedDocxCitationMarker[] = [];
  const bibliographies: ImportedDocxBibliographyMarker[] = [];
  for (const field of docxFieldOccurrences(document)) {
    if (!canMarkField(field)) continue;
    const tags = documentCitationTagsFromInstruction(field.instruction);
    if (tags.length) {
      const index = citations.length + 1;
      const marker: ImportedDocxCitationMarker = {
        start: `__A3S_WORK_CITATION_START_${index}__`,
        end: `__A3S_WORK_CITATION_END_${index}__`,
        id: `docx-citation-${index}`,
        instruction: field.instruction,
        tags,
        display: docxFieldResultText(field),
      };
      insertFieldBoundaryMarkers(document, field, marker.start, marker.end);
      citations.push(marker);
      continue;
    }
    if (/^\s*BIBLIOGRAPHY\b/i.test(field.instruction)) {
      const index = bibliographies.length + 1;
      const marker: ImportedDocxBibliographyMarker = {
        start: `__A3S_WORK_BIBLIOGRAPHY_START_${index}__`,
        end: `__A3S_WORK_BIBLIOGRAPHY_END_${index}__`,
        id: `docx-bibliography-${index}`,
        instruction: field.instruction,
        display: docxFieldResultText(field),
      };
      insertFieldBoundaryMarkers(document, field, marker.start, marker.end);
      bibliographies.push(marker);
    }
  }
  return { citations, bibliographies };
}

export function applyImportedDocxCitationMarkers(document: Document, markers: ImportedDocxCitationMarkers): void {
  for (const citation of markers.citations) {
    const element = document.createElement('span');
    element.dataset.documentCitation = 'true';
    element.dataset.citationId = citation.id;
    element.dataset.citationTags = citation.tags.join(' ');
    element.dataset.citationInstruction = citation.instruction;
    const convertedDisplay = replaceMarkerRange(document.body, citation.start, citation.end, element);
    const display = citation.display.trim() || convertedDisplay.trim();
    element.dataset.citationDisplay = display;
    element.textContent = display;
  }
  for (const bibliography of markers.bibliographies) {
    replaceBibliographyMarkerRange(document.body, bibliography);
  }
}

export function hasImportedDocxCitationMarkers(markers: ImportedDocxCitationMarkers): boolean {
  return markers.citations.length > 0 || markers.bibliographies.length > 0;
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

function replaceBibliographyMarkerRange(root: HTMLElement, marker: ImportedDocxBibliographyMarker): boolean {
  const nodes = textNodes(root);
  const startNode = nodes.find((node) => node.data.includes(marker.start));
  const endNode = nodes.find((node) => node.data.includes(marker.end));
  if (!startNode || !endNode) return false;
  const startBlock = closestHtmlBlock(startNode.parentElement, root);
  const endBlock = closestHtmlBlock(endNode.parentElement, root);
  if (!startBlock || !endBlock || startBlock.parentElement !== endBlock.parentElement) return false;
  const blocks: HTMLElement[] = [];
  let current: Element | null = startBlock;
  while (current) {
    if (current instanceof HTMLElement) blocks.push(current);
    if (current === endBlock) break;
    current = current.nextElementSibling;
  }
  if (!blocks.length || blocks.at(-1) !== endBlock) return false;
  const convertedDisplay = blocks
    .map((block) => block.textContent ?? '')
    .join('\n')
    .replace(marker.start, '')
    .replace(marker.end, '')
    .trim();
  const section = root.ownerDocument.createElement('section');
  section.dataset.documentBibliography = 'true';
  section.dataset.bibliographyId = marker.id;
  section.dataset.bibliographyInstruction = marker.instruction;
  section.dataset.bibliographyDisplay = marker.display.trim() || convertedDisplay;
  section.className = 'work-document-bibliography';
  const heading = root.ownerDocument.createElement('h2');
  heading.textContent = '参考文献';
  const paragraph = root.ownerDocument.createElement('p');
  paragraph.textContent = marker.display.trim() || convertedDisplay || '尚无文献源';
  section.append(heading, paragraph);
  startBlock.parentElement?.insertBefore(section, startBlock);
  for (const block of blocks) block.remove();
  return true;
}

function closestHtmlBlock(element: HTMLElement | null, root: HTMLElement): HTMLElement | null {
  let current = element;
  while (current && current !== root) {
    if (/^(?:P|DIV|LI|SECTION)$/.test(current.tagName)) return current;
    current = current.parentElement;
  }
  return null;
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
