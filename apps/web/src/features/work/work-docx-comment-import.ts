import type { WorkDocumentComment, WorkDocumentCommentReply } from './work-types';
import { attribute, descendants, OoxmlPackage } from './work-ooxml-package';

export interface ImportedDocxCommentRange {
  id: string;
  start: string;
  end: string;
}

export interface ImportedDocxCommentMarkers {
  comments: WorkDocumentComment[];
  ranges: ImportedDocxCommentRange[];
}

interface DocxCommentDefinition {
  sourceId: string;
  author: string;
  date: string;
  text: string;
  paraId: string;
  parentParaId: string;
  resolved: boolean;
}

interface DocxCommentExtended {
  parentParaId: string;
  resolved: boolean;
}

const WORD_NAMESPACE = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main';
const XML_NAMESPACE = 'http://www.w3.org/XML/1998/namespace';

export async function markDocxComments(document: Document, archive: OoxmlPackage): Promise<ImportedDocxCommentMarkers> {
  if (!archive.has('word/comments.xml')) return { comments: [], ranges: [] };
  const definitions = await readCommentDefinitions(archive);
  const bySourceId = new Map(definitions.map((definition) => [definition.sourceId, definition] as const));
  const workIds = new Map<string, string>();
  const ranges: ImportedDocxCommentRange[] = [];
  const comments: WorkDocumentComment[] = [];
  const starts = descendants(document, 'commentRangeStart').sort(compareDocumentOrder);

  for (const start of starts) {
    const sourceId = attribute(start, 'id')?.trim() ?? '';
    if (!sourceId) continue;
    const end = descendants(document, 'commentRangeEnd').find(
      (candidate) =>
        attribute(candidate, 'id') === sourceId &&
        Boolean(start.compareDocumentPosition(candidate) & Node.DOCUMENT_POSITION_FOLLOWING)
    );
    if (!end?.parentNode || !start.parentNode) continue;

    const workId = workIds.get(sourceId) ?? uniqueWorkCommentId(`docx-comment-${sourceId}`, comments);
    if (!workIds.has(sourceId)) {
      workIds.set(sourceId, workId);
      comments.push(toWorkComment(workId, bySourceId.get(sourceId), definitions));
    }
    const index = ranges.length + 1;
    const range = {
      id: workId,
      start: `__A3S_WORK_COMMENT_START_${index}__`,
      end: `__A3S_WORK_COMMENT_END_${index}__`,
    };
    start.parentNode.insertBefore(markerRun(document, range.start), start);
    end.parentNode.insertBefore(markerRun(document, range.end), end.nextSibling);
    start.remove();
    end.remove();
    for (const reference of descendants(document, 'commentReference').filter(
      (candidate) => attribute(candidate, 'id') === sourceId
    )) {
      reference.remove();
    }
    ranges.push(range);
  }

  return { comments, ranges };
}

export function applyImportedDocxCommentMarkers(document: Document, markers: ImportedDocxCommentMarkers): void {
  for (const marker of markers.ranges) {
    wrapTextBetweenMarkers(document.body, marker.start, marker.end, () => {
      const element = document.createElement('span');
      element.dataset.documentComment = 'true';
      element.dataset.commentId = marker.id;
      return element;
    });
  }
}

export function hasImportedDocxCommentMarkers(markers: ImportedDocxCommentMarkers): boolean {
  return markers.ranges.length > 0;
}

async function readCommentDefinitions(archive: OoxmlPackage): Promise<DocxCommentDefinition[]> {
  const document = await archive.xml('word/comments.xml');
  const extended = await readCommentExtended(archive);
  return descendants(document, 'comment').map((comment) => {
    const paragraphs = descendants(comment, 'p');
    const paraId = attribute(paragraphs.at(-1) ?? comment, 'paraId')?.trim() ?? '';
    const metadata = extended.get(paraId);
    return {
      sourceId: attribute(comment, 'id')?.trim() ?? '',
      author: attribute(comment, 'author')?.trim() || '未知审阅者',
      date: normalizeDate(attribute(comment, 'date')),
      text: commentText(comment),
      paraId,
      parentParaId: metadata?.parentParaId ?? '',
      resolved: metadata?.resolved ?? false,
    };
  });
}

async function readCommentExtended(archive: OoxmlPackage): Promise<Map<string, DocxCommentExtended>> {
  if (!archive.has('word/commentsExtended.xml')) return new Map();
  const document = await archive.xml('word/commentsExtended.xml');
  return new Map(
    descendants(document, 'commentEx')
      .map((comment) => {
        const paraId = attribute(comment, 'paraId')?.trim() ?? '';
        return [
          paraId,
          {
            parentParaId: attribute(comment, 'paraIdParent')?.trim() ?? '',
            resolved: attribute(comment, 'done') === '1' || attribute(comment, 'done') === 'true',
          },
        ] as const;
      })
      .filter(([paraId]) => Boolean(paraId))
  );
}

function toWorkComment(
  id: string,
  root: DocxCommentDefinition | undefined,
  definitions: DocxCommentDefinition[]
): WorkDocumentComment {
  if (!root) {
    return {
      id,
      author: '未知审阅者',
      date: '',
      text: '此批注的内容无法读取。',
      resolved: false,
    };
  }
  const replies = definitions
    .filter((definition) => definition !== root && commentRoot(definition, definitions) === root)
    .map(
      (definition): WorkDocumentCommentReply => ({
        id: `docx-comment-reply-${definition.sourceId}`,
        author: definition.author,
        date: definition.date,
        text: definition.text,
      })
    );
  return {
    id,
    author: root.author,
    date: root.date,
    text: root.text,
    resolved: root.resolved,
    replies: replies.length ? replies : undefined,
  };
}

function commentRoot(definition: DocxCommentDefinition, definitions: DocxCommentDefinition[]): DocxCommentDefinition {
  const byParaId = new Map(definitions.map((item) => [item.paraId, item] as const));
  const visited = new Set<string>();
  let current = definition;
  while (current.parentParaId && !visited.has(current.parentParaId)) {
    visited.add(current.parentParaId);
    const parent = byParaId.get(current.parentParaId);
    if (!parent) break;
    current = parent;
  }
  return current;
}

function commentText(comment: Element): string {
  const paragraphs = descendants(comment, 'p');
  const text = paragraphs
    .map((paragraph) =>
      descendants(paragraph, 't')
        .map((node) => node.textContent ?? '')
        .join('')
        .trim()
    )
    .filter(Boolean)
    .join('\n');
  return text || '（空批注）';
}

function markerRun(document: Document, value: string): Element {
  const run = document.createElementNS(WORD_NAMESPACE, 'w:r');
  const text = document.createElementNS(WORD_NAMESPACE, 'w:t');
  text.setAttributeNS(XML_NAMESPACE, 'xml:space', 'preserve');
  text.textContent = value;
  run.append(text);
  return run;
}

function wrapTextBetweenMarkers(
  root: HTMLElement,
  startMarker: string,
  endMarker: string,
  createWrapper: () => HTMLElement
): boolean {
  const nodes = textNodes(root);
  const startNode = nodes.find((node) => node.data.includes(startMarker));
  const endNode = nodes.find((node) => node.data.includes(endMarker));
  if (!startNode || !endNode) return false;
  const startNodeIndex = nodes.indexOf(startNode);
  const endNodeIndex = nodes.indexOf(endNode);
  const startIndex = startNode.data.indexOf(startMarker);
  const endIndex = endNode.data.indexOf(endMarker);
  if (startNodeIndex > endNodeIndex || (startNode === endNode && endIndex < startIndex + startMarker.length)) {
    return false;
  }

  let startOffset = startIndex;
  let endOffset = endIndex;
  if (startNode === endNode) {
    const value = startNode.data;
    endOffset = endIndex - startMarker.length;
    startNode.data =
      value.slice(0, startIndex) +
      value.slice(startIndex + startMarker.length, endIndex) +
      value.slice(endIndex + endMarker.length);
  } else {
    startNode.data = startNode.data.slice(0, startIndex) + startNode.data.slice(startIndex + startMarker.length);
    endNode.data = endNode.data.slice(0, endIndex) + endNode.data.slice(endIndex + endMarker.length);
  }

  let wrapped = false;
  for (let index = startNodeIndex; index <= endNodeIndex; index += 1) {
    const node = nodes[index];
    const from = index === startNodeIndex ? startOffset : 0;
    const to = index === endNodeIndex ? endOffset : node.data.length;
    if (from >= to || !node.parentNode) continue;
    wrapTextSegment(node, from, to, createWrapper());
    wrapped = true;
  }
  return wrapped;
}

function wrapTextSegment(node: Text, from: number, to: number, wrapper: HTMLElement): void {
  let selected = node;
  if (from > 0) selected = node.splitText(from);
  const length = to - from;
  if (length < selected.data.length) selected.splitText(length);
  selected.parentNode?.insertBefore(wrapper, selected);
  wrapper.append(selected);
}

function textNodes(root: ParentNode): Text[] {
  const document = root.ownerDocument;
  const walker = document?.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const nodes: Text[] = [];
  if (!walker) return nodes;
  while (walker.nextNode()) nodes.push(walker.currentNode as Text);
  return nodes;
}

function uniqueWorkCommentId(base: string, comments: WorkDocumentComment[]): string {
  const ids = new Set(comments.map((comment) => comment.id));
  if (!ids.has(base)) return base;
  let suffix = 2;
  while (ids.has(`${base}-${suffix}`)) suffix += 1;
  return `${base}-${suffix}`;
}

function normalizeDate(value: string | null): string {
  if (!value) return '';
  const time = Date.parse(value);
  return Number.isFinite(time) ? new Date(time).toISOString() : '';
}

function compareDocumentOrder(left: Element, right: Element): number {
  if (left === right) return 0;
  return left.compareDocumentPosition(right) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1;
}
