import JSZip from 'jszip';
import {
  attribute,
  descendants,
  directChildren,
  firstDescendant,
  OoxmlPackage,
  type OoxmlRelationship,
  parseXml,
} from './work-ooxml-package';
import type { WorkSlide, WorkSlideComment } from './work-types';

export interface PptxCommentAuthor {
  id: string;
  name: string;
  initials: string;
}

export interface PptxSlideCommentReadResult {
  comments: WorkSlideComment[];
  hasLegacyComments: boolean;
  hasUnsupportedThreadedComments: boolean;
  hasUnreadableComments: boolean;
  hasMalformedMetadata: boolean;
}

const PRESENTATION_NAMESPACE = 'http://schemas.openxmlformats.org/presentationml/2006/main';
const DRAWING_NAMESPACE = 'http://schemas.openxmlformats.org/drawingml/2006/main';
const OFFICE_RELATIONSHIPS_NAMESPACE = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';
const PACKAGE_RELATIONSHIPS_NAMESPACE = 'http://schemas.openxmlformats.org/package/2006/relationships';
const CONTENT_TYPES_NAMESPACE = 'http://schemas.openxmlformats.org/package/2006/content-types';
const COMMENTS_RELATIONSHIP = `${OFFICE_RELATIONSHIPS_NAMESPACE}/comments`;
const COMMENT_AUTHORS_RELATIONSHIP = `${OFFICE_RELATIONSHIPS_NAMESPACE}/commentAuthors`;
const COMMENTS_CONTENT_TYPE = 'application/vnd.openxmlformats-officedocument.presentationml.comments+xml';
const COMMENT_AUTHORS_CONTENT_TYPE = 'application/vnd.openxmlformats-officedocument.presentationml.commentAuthors+xml';

export async function loadPptxCommentAuthors(
  archive: OoxmlPackage,
  presentationRelationships: Map<string, OoxmlRelationship>
): Promise<Map<string, PptxCommentAuthor>> {
  const relationship = Array.from(presentationRelationships.values()).find((item) =>
    item.type.endsWith('/commentAuthors')
  );
  if (!relationship || !archive.has(relationship.target)) return new Map();
  const document = await archive.xml(relationship.target);
  return new Map(
    descendants(document, 'cmAuthor').map((author) => {
      const id = attribute(author, 'id')?.trim() ?? '';
      return [
        id,
        {
          id,
          name: attribute(author, 'name')?.trim() || '未知审阅者',
          initials: attribute(author, 'initials')?.trim() || '',
        },
      ] as const;
    })
  );
}

export async function readPptxSlideComments(
  archive: OoxmlPackage,
  relationships: Map<string, OoxmlRelationship>,
  authors: Map<string, PptxCommentAuthor>,
  slideNumber: number,
  slideWidthEmu: number,
  slideHeightEmu: number
): Promise<PptxSlideCommentReadResult> {
  const result: PptxSlideCommentReadResult = {
    comments: [],
    hasLegacyComments: false,
    hasUnsupportedThreadedComments: false,
    hasUnreadableComments: false,
    hasMalformedMetadata: false,
  };
  const commentRelationships = Array.from(relationships.values()).filter((item) => item.type.endsWith('/comments'));
  for (const relationship of commentRelationships) {
    if (!archive.has(relationship.target)) {
      result.hasUnreadableComments = true;
      continue;
    }
    const document = await archive.xml(relationship.target);
    const root = document.documentElement;
    if (root.namespaceURI !== PRESENTATION_NAMESPACE || root.localName !== 'cmLst') {
      result.hasUnsupportedThreadedComments = true;
      continue;
    }
    result.hasLegacyComments = true;
    for (const [index, element] of directChildren(root, 'cm').entries()) {
      const authorId = attribute(element, 'authorId')?.trim() ?? '';
      const author = authors.get(authorId);
      const sourceIndex = attribute(element, 'idx')?.trim() || String(index + 1);
      const position = firstDescendant(element, 'pos');
      const sourceDate = attribute(element, 'dt');
      if (
        !author ||
        !normalizeDate(sourceDate) ||
        !position ||
        !validCoordinate(attribute(position ?? element, 'x'), slideWidthEmu) ||
        !validCoordinate(attribute(position ?? element, 'y'), slideHeightEmu)
      ) {
        result.hasMalformedMetadata = true;
      }
      result.comments.push({
        id: `pptx-comment-${slideNumber}-${safeIdPart(sourceIndex)}-${index + 1}`,
        author: author?.name ?? '未知审阅者',
        initials: author?.initials || undefined,
        date: normalizeDate(sourceDate),
        text: firstDescendant(element, 'text')?.textContent ?? '',
        x: coordinatePercent(attribute(position ?? element, 'x'), slideWidthEmu),
        y: coordinatePercent(attribute(position ?? element, 'y'), slideHeightEmu),
      });
    }
  }
  return result;
}

export async function patchPptxComments(
  buffer: ArrayBuffer,
  slides: readonly WorkSlide[],
  slideWidth: number,
  slideHeight: number
): Promise<ArrayBuffer> {
  if (!slides.some((slide) => slide.comments?.length)) return buffer;
  const archive = await JSZip.loadAsync(buffer);
  const authors = collectExportAuthors(slides);
  const slideWidthEmu = Math.round(slideWidth * 914_400);
  const slideHeightEmu = Math.round(slideHeight * 914_400);
  const commentParts: string[] = [];

  for (const [slideIndex, slide] of slides.entries()) {
    if (!slide.comments?.length) continue;
    const partPath = `ppt/comments/comment${slideIndex + 1}.xml`;
    commentParts.push(partPath);
    archive.file(partPath, serializeSlideComments(slide.comments, authors, slideWidthEmu, slideHeightEmu));
    await upsertRelationship(
      archive,
      `ppt/slides/slide${slideIndex + 1}.xml`,
      COMMENTS_RELATIONSHIP,
      `../comments/comment${slideIndex + 1}.xml`
    );
  }

  archive.file('ppt/commentAuthors.xml', serializeCommentAuthors(authors));
  await upsertRelationship(archive, 'ppt/presentation.xml', COMMENT_AUTHORS_RELATIONSHIP, 'commentAuthors.xml');
  await upsertContentTypes(archive, commentParts);
  return archive.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });
}

interface ExportCommentAuthor extends PptxCommentAuthor {
  nextIndex: number;
  lastIndex: number;
  colorIndex: number;
}

function collectExportAuthors(slides: readonly WorkSlide[]): Map<string, ExportCommentAuthor> {
  const authors = new Map<string, ExportCommentAuthor>();
  for (const comment of slides.flatMap((slide) => slide.comments ?? [])) {
    const name = comment.author.trim() || 'A3S Work';
    const initials = comment.initials?.trim() || commentInitials(name);
    const key = authorKey(name, initials);
    if (authors.has(key)) continue;
    authors.set(key, {
      id: String(authors.size),
      name,
      initials,
      nextIndex: 1,
      lastIndex: 0,
      colorIndex: authors.size % 8,
    });
  }
  return authors;
}

function serializeSlideComments(
  comments: readonly WorkSlideComment[],
  authors: Map<string, ExportCommentAuthor>,
  slideWidthEmu: number,
  slideHeightEmu: number
): string {
  const document = presentationDocument('cmLst');
  for (const comment of comments) {
    const name = comment.author.trim() || 'A3S Work';
    const initials = comment.initials?.trim() || commentInitials(name);
    const author = authors.get(authorKey(name, initials));
    if (!author) continue;
    const index = author.nextIndex;
    author.nextIndex += 1;
    author.lastIndex = Math.max(author.lastIndex, index);
    const element = document.createElementNS(PRESENTATION_NAMESPACE, 'p:cm');
    element.setAttribute('authorId', author.id);
    element.setAttribute('dt', commentDate(comment.date));
    element.setAttribute('idx', String(index));
    const position = document.createElementNS(PRESENTATION_NAMESPACE, 'p:pos');
    position.setAttribute('x', String(percentCoordinate(comment.x, slideWidthEmu)));
    position.setAttribute('y', String(percentCoordinate(comment.y, slideHeightEmu)));
    const text = document.createElementNS(PRESENTATION_NAMESPACE, 'p:text');
    text.textContent = comment.text;
    element.append(position, text);
    document.documentElement.append(element);
  }
  return new XMLSerializer().serializeToString(document);
}

function serializeCommentAuthors(authors: Map<string, ExportCommentAuthor>): string {
  const document = presentationDocument('cmAuthorLst');
  for (const author of authors.values()) {
    const element = document.createElementNS(PRESENTATION_NAMESPACE, 'p:cmAuthor');
    element.setAttribute('id', author.id);
    element.setAttribute('name', author.name);
    element.setAttribute('initials', author.initials);
    element.setAttribute('lastIdx', String(author.lastIndex));
    element.setAttribute('clrIdx', String(author.colorIndex));
    document.documentElement.append(element);
  }
  return new XMLSerializer().serializeToString(document);
}

async function upsertRelationship(archive: JSZip, sourcePart: string, type: string, target: string): Promise<void> {
  const path = relationshipPartPath(sourcePart);
  const entry = archive.file(path);
  const document = entry
    ? parseXml(await entry.async('text'), path)
    : parseXml(`<Relationships xmlns="${PACKAGE_RELATIONSHIPS_NAMESPACE}"/>`, path);
  const root = document.documentElement;
  const existing = directChildren(root, 'Relationship').find((item) => attribute(item, 'Type') === type);
  if (existing) {
    existing.setAttribute('Target', target);
  } else {
    const relationship = document.createElementNS(PACKAGE_RELATIONSHIPS_NAMESPACE, 'Relationship');
    relationship.setAttribute('Id', nextRelationshipId(root));
    relationship.setAttribute('Type', type);
    relationship.setAttribute('Target', target);
    root.append(relationship);
  }
  archive.file(path, new XMLSerializer().serializeToString(document));
}

async function upsertContentTypes(archive: JSZip, commentParts: string[]): Promise<void> {
  const path = '[Content_Types].xml';
  const entry = archive.file(path);
  if (!entry) throw new Error('PPTX content types part is missing.');
  const document = parseXml(await entry.async('text'), path);
  upsertContentType(document, '/ppt/commentAuthors.xml', COMMENT_AUTHORS_CONTENT_TYPE);
  for (const part of commentParts) upsertContentType(document, `/${part}`, COMMENTS_CONTENT_TYPE);
  archive.file(path, new XMLSerializer().serializeToString(document));
}

function upsertContentType(document: Document, partName: string, contentType: string): void {
  const root = document.documentElement;
  const existing = directChildren(root, 'Override').find((item) => attribute(item, 'PartName') === partName);
  if (existing) {
    existing.setAttribute('ContentType', contentType);
    return;
  }
  const override = document.createElementNS(CONTENT_TYPES_NAMESPACE, 'Override');
  override.setAttribute('PartName', partName);
  override.setAttribute('ContentType', contentType);
  root.append(override);
}

function presentationDocument(rootName: string): Document {
  return parseXml(
    `<p:${rootName} xmlns:a="${DRAWING_NAMESPACE}" xmlns:r="${OFFICE_RELATIONSHIPS_NAMESPACE}" xmlns:p="${PRESENTATION_NAMESPACE}"/>`,
    `PPTX ${rootName}`
  );
}

function relationshipPartPath(sourcePart: string): string {
  const separator = sourcePart.lastIndexOf('/');
  const directory = separator >= 0 ? sourcePart.slice(0, separator + 1) : '';
  const fileName = separator >= 0 ? sourcePart.slice(separator + 1) : sourcePart;
  return `${directory}_rels/${fileName}.rels`;
}

function nextRelationshipId(root: Element): string {
  const used = new Set(directChildren(root, 'Relationship').map((item) => attribute(item, 'Id') ?? ''));
  let index = 1;
  while (used.has(`rId${index}`)) index += 1;
  return `rId${index}`;
}

function coordinatePercent(value: string | null, size: number): number {
  const coordinate = Number(value);
  if (!Number.isFinite(coordinate) || coordinate < 0 || size <= 0) return 50;
  return Math.round(clamp((coordinate / size) * 100, 0, 100) * 10_000) / 10_000;
}

function validCoordinate(value: string | null, size: number): boolean {
  const coordinate = Number(value);
  return value !== null && Number.isFinite(coordinate) && coordinate >= 0 && coordinate <= size;
}

function percentCoordinate(value: number, size: number): number {
  return Math.round((clamp(Number.isFinite(value) ? value : 50, 0, 100) / 100) * size);
}

function commentDate(value: string): string {
  const time = Date.parse(value);
  return Number.isFinite(time) ? new Date(time).toISOString() : new Date().toISOString();
}

function normalizeDate(value: string | null): string {
  if (!value) return '';
  const time = Date.parse(value);
  return Number.isFinite(time) ? new Date(time).toISOString() : '';
}

function commentInitials(author: string): string {
  const parts = author.trim().split(/\s+/).filter(Boolean);
  if (!parts.length) return 'AW';
  return parts
    .slice(0, 2)
    .map((part) => Array.from(part)[0] ?? '')
    .join('')
    .toUpperCase();
}

function authorKey(author: string, initials: string): string {
  return `${author}\u0000${initials}`;
}

function safeIdPart(value: string): string {
  const normalized = value.replace(/[^a-z0-9_-]/gi, '-').replace(/-+/g, '-');
  return normalized || 'comment';
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}
