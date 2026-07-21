import JSZip from 'jszip';
import {
  attribute,
  bytesToDataUrl,
  contentTypeForPart,
  directChild,
  directChildren,
  firstDescendant,
  OoxmlPackage,
  parseXml,
  type OoxmlRelationship,
} from './work-ooxml-package';
import {
  readXlsxDrawingAnchor,
  type XlsxDrawingAnchor,
  xlsxDrawingAnchorToBounds,
  xlsxTwoCellAnchorMarkers,
} from './work-xlsx-drawing-geometry';
import {
  XLSX_CHART_CONTENT_TYPE,
  XLSX_CHART_RELATIONSHIP,
  xlsxChartGraphicFrameXml,
  xlsxChartPartXml,
} from './work-xlsx-charts';
import type {
  WorkSpreadsheetChart,
  WorkSpreadsheetContent,
  WorkSpreadsheetImage,
  WorkSpreadsheetSheet,
} from './work-types';

export const MAX_XLSX_WORKSHEET_IMAGE_BYTES = 10 * 1024 * 1024;
const DRAWING_RELATIONSHIP = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing';
const IMAGE_RELATIONSHIP = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships/image';
const RELATIONSHIP_NAMESPACE = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';
const PACKAGE_RELATIONSHIP_NAMESPACE = 'http://schemas.openxmlformats.org/package/2006/relationships';
const DRAWING_NAMESPACE = 'http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing';
const DRAWINGML_NAMESPACE = 'http://schemas.openxmlformats.org/drawingml/2006/main';
const DRAWING_CONTENT_TYPE = 'application/vnd.openxmlformats-officedocument.drawing+xml';
const SUPPORTED_IMAGE_TYPES = new Set([
  'image/bmp',
  'image/gif',
  'image/jpeg',
  'image/png',
  'image/svg+xml',
  'image/webp',
]);

export function isSupportedXlsxWorksheetImageContentType(contentType: string): boolean {
  return SUPPORTED_IMAGE_TYPES.has(contentType.toLowerCase());
}

export interface XlsxWorksheetImage extends XlsxDrawingAnchor {
  id: string;
  name: string;
  altText?: string;
  contentType: string;
  dataUrl: string;
}

export async function readXlsxWorksheetImages(
  archive: OoxmlPackage,
  worksheetPart: string,
  worksheet: Document,
  imageBudget: { bytes: number }
): Promise<XlsxWorksheetImage[]> {
  const drawingReference = directChildren(worksheet.documentElement, 'drawing')[0];
  const relationshipId = drawingReference
    ? (attribute(drawingReference, 'r:id') ?? attribute(drawingReference, 'id'))
    : null;
  if (!relationshipId) return [];
  const worksheetRelationships = await archive.relationships(worksheetPart);
  const drawingRelationship = worksheetRelationships.get(relationshipId);
  if (
    !drawingRelationship ||
    drawingRelationship.targetMode === 'External' ||
    !drawingRelationship.type.endsWith('/drawing') ||
    !archive.has(drawingRelationship.target)
  ) {
    return [];
  }

  const drawing = await archive.xml(drawingRelationship.target);
  const relationships = await archive.relationships(drawingRelationship.target);
  const images: XlsxWorksheetImage[] = [];
  for (const [anchorIndex, anchor] of directChildren(drawing.documentElement).entries()) {
    if (!['twoCellAnchor', 'oneCellAnchor', 'absoluteAnchor'].includes(anchor.localName)) continue;
    const picture = directChild(anchor, 'pic');
    const blip = firstDescendant(picture, 'blip');
    const imageRelationshipId = blip ? (attribute(blip, 'r:embed') ?? attribute(blip, 'embed')) : null;
    const imageRelationship = imageRelationshipId ? relationships.get(imageRelationshipId) : undefined;
    if (!picture || !imageRelationship || !readableImageRelationship(archive, imageRelationship)) continue;
    const contentType = contentTypeForPart(imageRelationship.target);
    if (!isSupportedXlsxWorksheetImageContentType(contentType)) continue;
    const bytes = await archive.bytes(imageRelationship.target);
    if (imageBudget.bytes + bytes.byteLength > MAX_XLSX_WORKSHEET_IMAGE_BYTES) continue;
    imageBudget.bytes += bytes.byteLength;
    const properties = firstDescendant(picture, 'cNvPr');
    const sourceId = attribute(properties ?? picture, 'id') ?? String(anchorIndex + 1);
    const drawingNumber = partNumber(drawingRelationship.target);
    images.push({
      id: `xlsx-image-${drawingNumber}-${sourceId}`,
      name: attribute(properties ?? picture, 'name')?.trim() || `Worksheet image ${anchorIndex + 1}`,
      altText: attribute(properties ?? picture, 'descr')?.trim() || undefined,
      contentType,
      dataUrl: bytesToDataUrl(bytes, contentType),
      ...readXlsxDrawingAnchor(anchor),
    });
  }
  return images;
}

export function xlsxWorksheetImagesToSheet(
  images: readonly XlsxWorksheetImage[],
  config: WorkSpreadsheetSheet['config']
): WorkSpreadsheetImage[] {
  return images.flatMap((image) => {
    const bounds = xlsxDrawingAnchorToBounds(image, config);
    if (!bounds) return [];
    return [
      {
        id: image.id,
        name: image.name,
        altText: image.altText,
        contentType: image.contentType,
        src: image.dataUrl,
        ...bounds,
      },
    ];
  });
}

export async function patchXlsxWorksheetDrawings(
  buffer: ArrayBuffer,
  content: WorkSpreadsheetContent
): Promise<ArrayBuffer> {
  if (
    !content.sheets.some(
      (sheet) => sheet.images?.some((image) => serializableImage(image)) || (sheet.charts?.length ?? 0) > 0
    )
  ) {
    return buffer;
  }
  const archive = await OoxmlPackage.load(buffer);
  const worksheetParts = await exportedWorksheetParts(archive);
  const zip = await JSZip.loadAsync(buffer);
  const drawingPaths: string[] = [];
  const chartPaths: string[] = [];
  const imageTypes = new Map<string, string>();
  let drawingIndex = 1;
  let mediaIndex = 1;
  let chartIndex = 1;

  for (const sheet of content.sheets) {
    const images = (sheet.images ?? []).flatMap((image) => {
      const data = serializableImage(image);
      return data ? [{ image, data }] : [];
    });
    const charts = sheet.charts ?? [];
    if (!images.length && !charts.length) continue;
    const worksheetPart = worksheetParts.get(sheet.name.slice(0, 31) || '工作表');
    const worksheetEntry = worksheetPart ? zip.file(worksheetPart) : null;
    if (!worksheetPart || !worksheetEntry) continue;

    const drawingPath = `xl/drawings/drawing${drawingIndex}.xml`;
    const drawingRelationshipsPath = `xl/drawings/_rels/drawing${drawingIndex}.xml.rels`;
    drawingIndex += 1;
    const media = images.map(({ image, data }, index) => {
      const path = `xl/media/image${mediaIndex}.${data.extension}`;
      mediaIndex += 1;
      imageTypes.set(data.extension, data.contentType);
      zip.file(path, data.bytes);
      return {
        image,
        path,
        relationshipId: `rId${index + 1}`,
        objectId: index + 1,
      };
    });
    const chartParts = charts.map((chart, index) => {
      const path = `xl/charts/chart${chartIndex}.xml`;
      const currentChartIndex = chartIndex;
      chartIndex += 1;
      chartPaths.push(path);
      zip.file(path, xlsxChartPartXml(chart, content, sheet, currentChartIndex));
      return {
        chart,
        path,
        relationshipId: `rId${media.length + index + 1}`,
        objectId: media.length + index + 1,
      };
    });

    zip.file(drawingPath, drawingXml(media, chartParts, sheet));
    zip.file(drawingRelationshipsPath, drawingRelationshipsXml(media, chartParts));
    drawingPaths.push(drawingPath);
    await bindDrawingToWorksheet(zip, worksheetPart, worksheetEntry, drawingPath);
  }

  await updateContentTypes(zip, drawingPaths, chartPaths, imageTypes);
  return zip.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });
}

function readableImageRelationship(archive: OoxmlPackage, relationship: OoxmlRelationship): boolean {
  return (
    relationship.targetMode !== 'External' && relationship.type.endsWith('/image') && archive.has(relationship.target)
  );
}

interface SerializableImage {
  contentType: string;
  extension: string;
  bytes: Uint8Array;
}

function serializableImage(image: WorkSpreadsheetImage): SerializableImage | null {
  const match = /^data:([^;,]+);base64,([a-z0-9+/=\s]+)$/i.exec(image.src);
  if (!match) return null;
  const contentType = image.contentType?.toLowerCase() || match[1].toLowerCase();
  const extension = imageExtension(contentType);
  if (!extension || !isSupportedXlsxWorksheetImageContentType(contentType)) return null;
  try {
    const binary = atob(match[2].replace(/\s+/g, ''));
    return {
      contentType,
      extension,
      bytes: Uint8Array.from(binary, (character) => character.charCodeAt(0)),
    };
  } catch {
    return null;
  }
}

function imageExtension(contentType: string): string | null {
  const extensions: Record<string, string> = {
    'image/bmp': 'bmp',
    'image/gif': 'gif',
    'image/jpeg': 'jpeg',
    'image/png': 'png',
    'image/svg+xml': 'svg',
    'image/webp': 'webp',
  };
  return extensions[contentType] ?? null;
}

async function exportedWorksheetParts(archive: OoxmlPackage): Promise<Map<string, string>> {
  if (!archive.has('xl/workbook.xml')) return new Map();
  const workbook = await archive.xml('xl/workbook.xml');
  const relationships = await archive.relationships('xl/workbook.xml');
  const parts = new Map<string, string>();
  for (const sheet of directChildren(firstDescendant(workbook, 'sheets') ?? workbook.documentElement, 'sheet')) {
    const name = attribute(sheet, 'name');
    const relationship = relationships.get(attribute(sheet, 'r:id') ?? attribute(sheet, 'id') ?? '');
    if (name && relationship?.type.endsWith('/worksheet')) parts.set(name, relationship.target);
  }
  return parts;
}

function drawingXml(
  media: Array<{
    image: WorkSpreadsheetImage;
    relationshipId: string;
    objectId: number;
  }>,
  charts: Array<{
    chart: WorkSpreadsheetChart;
    relationshipId: string;
    objectId: number;
  }>,
  sheet: WorkSpreadsheetSheet
): string {
  return [
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
    `<xdr:wsDr xmlns:xdr="${DRAWING_NAMESPACE}" xmlns:a="${DRAWINGML_NAMESPACE}" xmlns:r="${RELATIONSHIP_NAMESPACE}">`,
    ...media.map(({ image, relationshipId, objectId }) => {
      return [
        '<xdr:twoCellAnchor editAs="oneCell">',
        xlsxTwoCellAnchorMarkers(image, sheet),
        '<xdr:pic><xdr:nvPicPr>',
        `<xdr:cNvPr id="${objectId}" name="${escapeXml(image.name?.trim() || `Image ${objectId}`)}"${
          image.altText?.trim() ? ` descr="${escapeXml(image.altText.trim())}"` : ''
        }/>`,
        '<xdr:cNvPicPr><a:picLocks noChangeAspect="1"/></xdr:cNvPicPr></xdr:nvPicPr>',
        `<xdr:blipFill><a:blip r:embed="${relationshipId}"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill>`,
        '<xdr:spPr><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></xdr:spPr>',
        '</xdr:pic><xdr:clientData/></xdr:twoCellAnchor>',
      ].join('');
    }),
    ...charts.map(({ chart, relationshipId, objectId }) =>
      xlsxChartGraphicFrameXml(chart, relationshipId, objectId, sheet)
    ),
    '</xdr:wsDr>',
  ].join('');
}

function drawingRelationshipsXml(
  media: Array<{ path: string; relationshipId: string }>,
  charts: Array<{ path: string; relationshipId: string }>
): string {
  return [
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
    `<Relationships xmlns="${PACKAGE_RELATIONSHIP_NAMESPACE}">`,
    ...media.map(
      ({ path, relationshipId }) =>
        `<Relationship Id="${relationshipId}" Type="${IMAGE_RELATIONSHIP}" Target="../media/${path.split('/').at(-1)}"/>`
    ),
    ...charts.map(
      ({ path, relationshipId }) =>
        `<Relationship Id="${relationshipId}" Type="${XLSX_CHART_RELATIONSHIP}" Target="../charts/${path
          .split('/')
          .at(-1)}"/>`
    ),
    '</Relationships>',
  ].join('');
}

async function bindDrawingToWorksheet(
  zip: JSZip,
  worksheetPart: string,
  worksheetEntry: JSZip.JSZipObject,
  drawingPath: string
): Promise<void> {
  const worksheet = parseXml(await worksheetEntry.async('text'), worksheetPart);
  for (const existing of directChildren(worksheet.documentElement, 'drawing')) existing.remove();
  const relationshipPath = relationshipsPartPath(worksheetPart);
  const relationshipEntry = zip.file(relationshipPath);
  const relationships = relationshipEntry
    ? parseXml(await relationshipEntry.async('text'), relationshipPath)
    : parseXml(
        `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="${PACKAGE_RELATIONSHIP_NAMESPACE}"/>`,
        relationshipPath
      );
  for (const existing of directChildren(relationships.documentElement, 'Relationship')) {
    if (attribute(existing, 'Type')?.endsWith('/drawing')) existing.remove();
  }
  const relationshipId = nextRelationshipId(relationships);
  const relationship = relationships.createElementNS(PACKAGE_RELATIONSHIP_NAMESPACE, 'Relationship');
  relationship.setAttribute('Id', relationshipId);
  relationship.setAttribute('Type', DRAWING_RELATIONSHIP);
  relationship.setAttribute('Target', relativePartTarget(worksheetPart, drawingPath));
  relationships.documentElement.append(relationship);

  const drawing = worksheet.createElementNS(worksheet.documentElement.namespaceURI, 'drawing');
  drawing.setAttributeNS(RELATIONSHIP_NAMESPACE, 'r:id', relationshipId);
  const insertionPoint = directChildren(worksheet.documentElement).find((element) =>
    [
      'legacyDrawing',
      'legacyDrawingHF',
      'picture',
      'oleObjects',
      'controls',
      'webPublishItems',
      'tableParts',
      'extLst',
    ].includes(element.localName)
  );
  worksheet.documentElement.insertBefore(drawing, insertionPoint ?? null);
  zip.file(worksheetPart, new XMLSerializer().serializeToString(worksheet));
  zip.file(relationshipPath, new XMLSerializer().serializeToString(relationships));
}

function nextRelationshipId(document: Document): string {
  const ids = new Set(
    directChildren(document.documentElement, 'Relationship')
      .map((relationship) => attribute(relationship, 'Id'))
      .filter((id): id is string => Boolean(id))
  );
  let index = 1;
  while (ids.has(`rId${index}`)) index += 1;
  return `rId${index}`;
}

async function updateContentTypes(
  zip: JSZip,
  drawingPaths: readonly string[],
  chartPaths: readonly string[],
  imageTypes: ReadonlyMap<string, string>
): Promise<void> {
  const entry = zip.file('[Content_Types].xml');
  if (!entry) return;
  const document = parseXml(await entry.async('text'), '[Content_Types].xml');
  const namespace = document.documentElement.namespaceURI;
  for (const [extension, contentType] of imageTypes) {
    const existing = directChildren(document.documentElement, 'Default').find(
      (element) => attribute(element, 'Extension')?.toLowerCase() === extension
    );
    if (existing) {
      existing.setAttribute('ContentType', contentType);
      continue;
    }
    const definition = document.createElementNS(namespace, 'Default');
    definition.setAttribute('Extension', extension);
    definition.setAttribute('ContentType', contentType);
    document.documentElement.append(definition);
  }
  for (const path of drawingPaths) {
    const partName = `/${path}`;
    const existing = directChildren(document.documentElement, 'Override').find(
      (element) => attribute(element, 'PartName') === partName
    );
    const override = existing ?? document.createElementNS(namespace, 'Override');
    override.setAttribute('PartName', partName);
    override.setAttribute('ContentType', DRAWING_CONTENT_TYPE);
    if (!existing) document.documentElement.append(override);
  }
  for (const path of chartPaths) {
    const partName = `/${path}`;
    const existing = directChildren(document.documentElement, 'Override').find(
      (element) => attribute(element, 'PartName') === partName
    );
    const override = existing ?? document.createElementNS(namespace, 'Override');
    override.setAttribute('PartName', partName);
    override.setAttribute('ContentType', XLSX_CHART_CONTENT_TYPE);
    if (!existing) document.documentElement.append(override);
  }
  zip.file('[Content_Types].xml', new XMLSerializer().serializeToString(document));
}

function relationshipsPartPath(sourcePart: string): string {
  const segments = sourcePart.split('/');
  const name = segments.pop() ?? '';
  return [...segments, '_rels', `${name}.rels`].join('/');
}

function relativePartTarget(sourcePart: string, targetPart: string): string {
  const source = sourcePart.split('/').slice(0, -1);
  const target = targetPart.split('/');
  while (source.length && target.length && source[0] === target[0]) {
    source.shift();
    target.shift();
  }
  return [...source.map(() => '..'), ...target].join('/');
}

function partNumber(path: string): string {
  return /(\d+)(?:\.xml)?$/i.exec(path)?.[1] ?? '1';
}

function escapeXml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&apos;');
}
