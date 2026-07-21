import JSZip from 'jszip';
import {
  attribute,
  descendants,
  directChild,
  directChildren,
  firstDescendant,
  OoxmlPackage,
  parseXml,
} from './work-ooxml-package';
import { formatSpreadsheetCellRanges, parseSpreadsheetCellRanges } from './work-spreadsheet-ranges';
import { spreadsheetPivotValidation } from './work-spreadsheet-pivots';
import { createWorkId } from './work-templates';
import type {
  WorkSpreadsheetContent,
  WorkSpreadsheetPivotFilterValue,
  WorkSpreadsheetPivotReportFilter,
  WorkSpreadsheetPivotTable,
  WorkSpreadsheetPivotValue,
} from './work-types';
import { createXlsxPivotXmlParts, workPivotAggregation } from './work-xlsx-pivot-xml';

const PACKAGE_RELATIONSHIP_NAMESPACE = 'http://schemas.openxmlformats.org/package/2006/relationships';
const DOCUMENT_RELATIONSHIP_NAMESPACE = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';
const XMLNS_NAMESPACE = 'http://www.w3.org/2000/xmlns/';
const PIVOT_TABLE_RELATIONSHIP = `${DOCUMENT_RELATIONSHIP_NAMESPACE}/pivotTable`;
const PIVOT_CACHE_DEFINITION_RELATIONSHIP = `${DOCUMENT_RELATIONSHIP_NAMESPACE}/pivotCacheDefinition`;
const PIVOT_CACHE_RECORDS_RELATIONSHIP = `${DOCUMENT_RELATIONSHIP_NAMESPACE}/pivotCacheRecords`;
const PIVOT_TABLE_CONTENT_TYPE = 'application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml';
const PIVOT_CACHE_DEFINITION_CONTENT_TYPE =
  'application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml';
const PIVOT_CACHE_RECORDS_CONTENT_TYPE =
  'application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheRecords+xml';

export interface XlsxPivotTableImport {
  destinationSheetName: string;
  sourceSheetName: string;
  sourceReference: string;
  name: string;
  anchor: string;
  outputReference: string;
  rowFields: number[];
  columnFields: number[];
  reportFilters: WorkSpreadsheetPivotReportFilter[];
  values: WorkSpreadsheetPivotValue[];
  rowGrandTotals: boolean;
  columnGrandTotals: boolean;
  styleName: string;
  refreshOnLoad: boolean;
}

export interface XlsxUnsupportedPivot {
  code: string;
  message: string;
  location?: string;
}

export interface XlsxPivotReadResult {
  tables: XlsxPivotTableImport[];
  unsupported: XlsxUnsupportedPivot[];
  detected: number;
}

interface XlsxPivotCache {
  sourceSheetName?: string;
  sourceReference?: string;
  fields: string[];
  fieldItems: WorkSpreadsheetPivotFilterValue[][];
  refreshOnLoad: boolean;
  unsupported?: XlsxUnsupportedPivot;
}

export async function patchXlsxPivotTables(buffer: ArrayBuffer, content: WorkSpreadsheetContent): Promise<ArrayBuffer> {
  if (!content.sheets.some((sheet) => sheet.pivotTables?.length)) return buffer;
  const archive = await OoxmlPackage.load(buffer);
  const worksheetParts = await workbookWorksheetParts(archive);
  const candidates = content.sheets.flatMap((ownerSheet) =>
    (ownerSheet.pivotTables ?? []).flatMap((pivot) => {
      if (!ownerSheet.id || !spreadsheetPivotValidation(content, ownerSheet.id, pivot).valid) return [];
      const worksheetPart = worksheetParts.get(ownerSheet.name.slice(0, 31) || '工作表');
      return worksheetPart ? [{ ownerSheet, pivot, worksheetPart }] : [];
    })
  );
  if (!candidates.length) return buffer;

  const zip = await JSZip.loadAsync(buffer);
  const workbookEntry = zip.file('xl/workbook.xml');
  const workbookRelationshipsEntry = zip.file('xl/_rels/workbook.xml.rels');
  if (!workbookEntry || !workbookRelationshipsEntry) return buffer;
  const workbook = parseXml(await workbookEntry.async('text'), 'xl/workbook.xml');
  const workbookRelationships = parseXml(await workbookRelationshipsEntry.async('text'), 'xl/_rels/workbook.xml.rels');
  removeRelationships(workbookRelationships, '/pivotCacheDefinition');
  for (const existing of directChildren(workbook.documentElement, 'pivotCaches')) existing.remove();
  const pivotCaches = workbook.createElementNS(workbook.documentElement.namespaceURI, 'pivotCaches');
  ensureRelationshipNamespace(workbook);
  const worksheetRelationships = new Map<string, Document>();
  const contentTypeParts: Array<{ path: string; contentType: string }> = [];
  let exported = 0;

  for (const [index, candidate] of candidates.entries()) {
    const partIndex = index + 1;
    const cacheId = partIndex;
    const parts = createXlsxPivotXmlParts(content, candidate.pivot, cacheId);
    if (!parts) continue;
    const pivotTablePath = `xl/pivotTables/pivotTable${partIndex}.xml`;
    const pivotCacheDefinitionPath = `xl/pivotCache/pivotCacheDefinition${partIndex}.xml`;
    const pivotCacheRecordsPath = `xl/pivotCache/pivotCacheRecords${partIndex}.xml`;
    const workbookRelationshipId = appendRelationship(
      workbookRelationships,
      PIVOT_CACHE_DEFINITION_RELATIONSHIP,
      `pivotCache/pivotCacheDefinition${partIndex}.xml`
    );
    const pivotCache = workbook.createElementNS(workbook.documentElement.namespaceURI, 'pivotCache');
    pivotCache.setAttribute('cacheId', String(cacheId));
    pivotCache.setAttributeNS(DOCUMENT_RELATIONSHIP_NAMESPACE, 'r:id', workbookRelationshipId);
    pivotCaches.append(pivotCache);

    const worksheetRelationshipDocument =
      worksheetRelationships.get(candidate.worksheetPart) ??
      (await readRelationshipsDocument(zip, relationshipsPartPath(candidate.worksheetPart)));
    if (!worksheetRelationships.has(candidate.worksheetPart)) {
      removeRelationships(worksheetRelationshipDocument, '/pivotTable');
      worksheetRelationships.set(candidate.worksheetPart, worksheetRelationshipDocument);
    }
    appendRelationship(
      worksheetRelationshipDocument,
      PIVOT_TABLE_RELATIONSHIP,
      `../pivotTables/pivotTable${partIndex}.xml`
    );

    zip.file(pivotTablePath, parts.table);
    zip.file(pivotCacheDefinitionPath, parts.cacheDefinition);
    zip.file(pivotCacheRecordsPath, parts.cacheRecords);
    zip.file(
      `xl/pivotTables/_rels/pivotTable${partIndex}.xml.rels`,
      relationshipsXml([
        {
          id: 'rId1',
          type: PIVOT_CACHE_DEFINITION_RELATIONSHIP,
          target: `../pivotCache/pivotCacheDefinition${partIndex}.xml`,
        },
      ])
    );
    zip.file(
      `xl/pivotCache/_rels/pivotCacheDefinition${partIndex}.xml.rels`,
      relationshipsXml([
        {
          id: 'rId1',
          type: PIVOT_CACHE_RECORDS_RELATIONSHIP,
          target: `pivotCacheRecords${partIndex}.xml`,
        },
      ])
    );
    contentTypeParts.push(
      { path: pivotTablePath, contentType: PIVOT_TABLE_CONTENT_TYPE },
      { path: pivotCacheDefinitionPath, contentType: PIVOT_CACHE_DEFINITION_CONTENT_TYPE },
      { path: pivotCacheRecordsPath, contentType: PIVOT_CACHE_RECORDS_CONTENT_TYPE }
    );
    exported += 1;
  }
  if (!exported) return buffer;

  insertWorkbookPivotCaches(workbook, pivotCaches);
  zip.file('xl/workbook.xml', new XMLSerializer().serializeToString(workbook));
  zip.file('xl/_rels/workbook.xml.rels', new XMLSerializer().serializeToString(workbookRelationships));
  for (const [worksheetPart, relationships] of worksheetRelationships) {
    zip.file(relationshipsPartPath(worksheetPart), new XMLSerializer().serializeToString(relationships));
  }
  await updateContentTypes(zip, contentTypeParts);
  return zip.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });
}

export async function readXlsxPivotTables(buffer: ArrayBuffer): Promise<XlsxPivotReadResult> {
  return inspectXlsxPivotTables(await OoxmlPackage.load(buffer));
}

export async function inspectXlsxPivotTables(archive: OoxmlPackage): Promise<XlsxPivotReadResult> {
  if (!archive.has('xl/workbook.xml')) return { tables: [], unsupported: [], detected: 0 };
  const workbook = await archive.xml('xl/workbook.xml');
  const workbookRelationships = await archive.relationships('xl/workbook.xml');
  const cacheById = new Map<number, XlsxPivotCache>();
  for (const cache of directChildren(
    firstDescendant(workbook, 'pivotCaches') ?? workbook.documentElement,
    'pivotCache'
  )) {
    const cacheId = integerAttribute(cache, 'cacheId');
    const relationship = workbookRelationships.get(attribute(cache, 'r:id') ?? attribute(cache, 'id') ?? '');
    if (cacheId === null || !relationship || !archive.has(relationship.target)) continue;
    cacheById.set(cacheId, parsePivotCache(await archive.xml(relationship.target)));
  }

  const worksheetParts = await workbookWorksheetParts(archive);
  const tables: XlsxPivotTableImport[] = [];
  const unsupported: XlsxUnsupportedPivot[] = [];
  let detected = 0;
  for (const [destinationSheetName, worksheetPart] of worksheetParts) {
    const relationships = await archive.relationships(worksheetPart);
    for (const relationship of relationships.values()) {
      if (!relationship.type.endsWith('/pivotTable')) continue;
      detected += 1;
      if (relationship.targetMode === 'External' || !archive.has(relationship.target)) {
        unsupported.push({
          code: 'xlsx.pivots.relationship',
          message: 'The pivot-table relationship is external or missing.',
          location: destinationSheetName,
        });
        continue;
      }
      const parsed = parsePivotTable(await archive.xml(relationship.target), destinationSheetName, cacheById);
      if ('unsupported' in parsed) unsupported.push(parsed.unsupported);
      else tables.push(parsed.table);
    }
  }
  if (!detected && (archive.paths('xl/pivotTables/').length || archive.paths('xl/pivotCache/').length)) {
    unsupported.push({
      code: 'xlsx.pivots.orphaned',
      message: 'Pivot-table package parts are present without a readable worksheet relationship.',
    });
  }
  if (archive.paths('xl/slicers/').length || archive.paths('xl/slicerCaches/').length) {
    unsupported.push({
      code: 'xlsx.pivots.slicers',
      message: 'Pivot slicers and timelines are not editable in Work.',
    });
  }
  return { tables, unsupported, detected: Math.max(detected, tables.length + unsupported.length) };
}

export function applyImportedXlsxPivotTables(
  content: WorkSpreadsheetContent,
  imported: XlsxPivotReadResult | null | undefined
): WorkSpreadsheetContent {
  if (!imported?.tables.length) return content;
  const sheets = content.sheets.map((sheet) => ({
    ...sheet,
    pivotTables: sheet.pivotTables ? [...sheet.pivotTables] : undefined,
  }));
  const byName = new Map(sheets.map((sheet) => [sheet.name, sheet]));
  for (const table of imported.tables) {
    const ownerSheet = byName.get(table.destinationSheetName);
    const sourceSheet = byName.get(table.sourceSheetName);
    if (!ownerSheet || !sourceSheet?.id) continue;
    const pivot: WorkSpreadsheetPivotTable = {
      id: createWorkId('pivot'),
      name: table.name,
      sourceSheetId: sourceSheet.id,
      sourceReference: table.sourceReference,
      anchor: table.anchor,
      rowFields: [...table.rowFields],
      columnFields: [...table.columnFields],
      reportFilters: table.reportFilters.map((filter) => ({ ...filter })),
      values: table.values.map((value) => ({ ...value })),
      rowGrandTotals: table.rowGrandTotals,
      columnGrandTotals: table.columnGrandTotals,
      styleName: table.styleName,
      refreshOnLoad: table.refreshOnLoad,
      outputReference: table.outputReference,
    };
    ownerSheet.pivotTables = [...(ownerSheet.pivotTables ?? []), pivot];
  }
  return { ...content, sheets };
}

async function workbookWorksheetParts(archive: OoxmlPackage): Promise<Map<string, string>> {
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

function parsePivotCache(document: Document): XlsxPivotCache {
  const root = document.documentElement;
  const cacheSource = directChild(root, 'cacheSource');
  const sourceType = attribute(cacheSource ?? root, 'type');
  const worksheetSource = directChild(cacheSource ?? root, 'worksheetSource');
  const sourceSheetName = attribute(worksheetSource ?? root, 'sheet')?.trim() || undefined;
  const sourceReference = attribute(worksheetSource ?? root, 'ref')?.trim() || undefined;
  const cacheFields = directChildren(directChild(root, 'cacheFields') ?? root, 'cacheField');
  const fields = cacheFields.map((field, index) => attribute(field, 'name')?.trim() || `Field ${index + 1}`);
  const fieldItems = cacheFields.map((field) =>
    directChildren(directChild(field, 'sharedItems') ?? field).flatMap((item) => {
      const value = pivotCacheItemValue(item);
      return value ? [value.value] : [];
    })
  );
  let unsupported: XlsxUnsupportedPivot | undefined;
  if (
    sourceType !== 'worksheet' ||
    !sourceSheetName ||
    !sourceReference ||
    attribute(worksheetSource ?? root, 'name')
  ) {
    unsupported = {
      code: sourceType === 'external' ? 'xlsx.pivots.external-source' : 'xlsx.pivots.source',
      message: 'Only worksheet pivots with an explicit cell-range source are editable.',
    };
  } else if (
    descendants(root, 'fieldGroup').length ||
    directChildren(directChild(root, 'cacheFields') ?? root, 'cacheField').some((field) => attribute(field, 'formula'))
  ) {
    unsupported = {
      code: 'xlsx.pivots.grouping',
      message: 'Grouped or calculated pivot-cache fields remain cached values only.',
    };
  } else if (descendants(root, 'calculatedItems').length || descendants(root, 'calculatedMembers').length) {
    unsupported = {
      code: 'xlsx.pivots.calculated-items',
      message: 'Calculated pivot items and members remain cached values only.',
    };
  }
  return {
    sourceSheetName,
    sourceReference,
    fields,
    fieldItems,
    refreshOnLoad: booleanAttribute(root, 'refreshOnLoad'),
    unsupported,
  };
}

function pivotCacheItemValue(element: Element): { value: WorkSpreadsheetPivotFilterValue } | null {
  const source = attribute(element, 'v');
  if (element.localName === 'm') return { value: null };
  if (element.localName === 'b') return { value: source === '1' || source?.toLowerCase() === 'true' };
  if (element.localName === 'n') {
    const value = Number(source);
    return Number.isFinite(value) ? { value } : null;
  }
  if (element.localName === 's' || element.localName === 'e' || element.localName === 'd') {
    return { value: source ?? '' };
  }
  return null;
}

function parsePivotReportFilters(
  root: Element,
  cache: XlsxPivotCache,
  location: string
):
  | { reportFilters: WorkSpreadsheetPivotReportFilter[] }
  | {
      unsupported: XlsxUnsupportedPivot;
    } {
  const pageFields = directChild(root, 'pageFields');
  if (!pageFields) return { reportFilters: [] };
  const pageWrap = integerAttribute(root, 'pageWrap') ?? 0;
  if (pageWrap > 0 || booleanAttribute(root, 'pageOverThenDown')) {
    return {
      unsupported: {
        code: 'xlsx.pivots.report-filter-layout',
        message: 'Wrapped or horizontal report-filter layouts remain cached values only.',
        location,
      },
    };
  }
  const pivotFields = directChildren(directChild(root, 'pivotFields') ?? root, 'pivotField');
  const reportFilters: WorkSpreadsheetPivotReportFilter[] = [];
  for (const pageField of directChildren(pageFields, 'pageField')) {
    const fieldIndex = integerAttribute(pageField, 'fld');
    const pivotField = fieldIndex === null ? undefined : pivotFields[fieldIndex];
    if (fieldIndex === null || !pivotField || !cache.fields[fieldIndex]) {
      return {
        unsupported: {
          code: 'xlsx.pivots.report-filter-field',
          message: 'A report filter refers to a missing pivot-cache field.',
          location,
        },
      };
    }
    const items = directChildren(directChild(pivotField, 'items') ?? pivotField, 'item');
    if (
      booleanAttribute(pivotField, 'multipleItemSelectionAllowed') ||
      items.some((item) => booleanAttribute(item, 'h'))
    ) {
      return {
        unsupported: {
          code: 'xlsx.pivots.report-filter-multi-select',
          message: 'Multi-selection report filters remain cached values only.',
          location,
        },
      };
    }
    const itemIndex = integerAttribute(pageField, 'item');
    if (itemIndex === null) {
      reportFilters.push({ fieldIndex });
      continue;
    }
    const pivotItem = items[itemIndex];
    if (!pivotItem) {
      return {
        unsupported: {
          code: 'xlsx.pivots.report-filter-item',
          message: 'A report filter refers to a missing pivot item.',
          location,
        },
      };
    }
    if (attribute(pivotItem, 't') === 'default') {
      reportFilters.push({ fieldIndex });
      continue;
    }
    const sharedItemIndex = integerAttribute(pivotItem, 'x');
    const sharedItems = cache.fieldItems[fieldIndex] ?? [];
    if (sharedItemIndex === null || sharedItemIndex < 0 || sharedItemIndex >= sharedItems.length) {
      return {
        unsupported: {
          code: 'xlsx.pivots.report-filter-item',
          message: 'A report filter item is missing from the pivot cache.',
          location,
        },
      };
    }
    reportFilters.push({
      fieldIndex,
      selectedItem: sharedItems[sharedItemIndex],
    });
  }
  return { reportFilters };
}

function parsePivotTable(
  document: Document,
  destinationSheetName: string,
  cacheById: ReadonlyMap<number, XlsxPivotCache>
): { table: XlsxPivotTableImport } | { unsupported: XlsxUnsupportedPivot } {
  const root = document.documentElement;
  const name = attribute(root, 'name')?.trim() || 'PivotTable';
  const locationLabel = destinationSheetName ? `${destinationSheetName} · ${name}` : name;
  const cacheId = integerAttribute(root, 'cacheId');
  const cache = cacheId === null ? undefined : cacheById.get(cacheId);
  if (!cache || cache.unsupported || !cache.sourceSheetName || !cache.sourceReference) {
    return {
      unsupported: {
        ...(cache?.unsupported ?? {
          code: 'xlsx.pivots.cache',
          message: 'The pivot cache definition is missing or unreadable.',
        }),
        location: locationLabel,
      },
    };
  }
  if (directChildren(root, 'filters').length || booleanAttribute(root, 'dataOnRows')) {
    return {
      unsupported: {
        code: 'xlsx.pivots.filters-layout',
        message: 'Label or value filters and values-on-rows layouts remain cached values only.',
        location: locationLabel,
      },
    };
  }
  const parsedReportFilters = parsePivotReportFilters(root, cache, locationLabel);
  if ('unsupported' in parsedReportFilters) return parsedReportFilters;
  const reportFilters = parsedReportFilters.reportFilters;
  const rowFields = readPivotFieldIndexes(directChild(root, 'rowFields'));
  const columnFields = readPivotFieldIndexes(directChild(root, 'colFields')).filter((index) => index !== -2);
  if (rowFields.includes(-2)) {
    return {
      unsupported: {
        code: 'xlsx.pivots.values-on-rows',
        message: 'Values-on-rows pivot layouts remain cached values only.',
        location: locationLabel,
      },
    };
  }
  const values: WorkSpreadsheetPivotValue[] = [];
  for (const dataField of directChildren(directChild(root, 'dataFields') ?? root, 'dataField')) {
    const fieldIndex = integerAttribute(dataField, 'fld');
    const aggregation = workPivotAggregation(attribute(dataField, 'subtotal'));
    if (fieldIndex === null || !aggregation || !cache.fields[fieldIndex]) {
      return {
        unsupported: {
          code: 'xlsx.pivots.aggregation',
          message: 'The pivot uses an unsupported value calculation or field reference.',
          location: locationLabel,
        },
      };
    }
    values.push({
      fieldIndex,
      aggregation,
      caption: attribute(dataField, 'name')?.trim() || undefined,
    });
  }
  const location = directChild(root, 'location');
  const outputReference = attribute(location ?? root, 'ref')?.trim() ?? '';
  const outputRange = parseSpreadsheetCellRanges(outputReference);
  const reportFilterFields = reportFilters.map((filter) => filter.fieldIndex);
  if (
    !rowFields.length ||
    !values.length ||
    outputRange?.length !== 1 ||
    [...rowFields, ...columnFields, ...reportFilterFields].some((index) => index < 0 || !cache.fields[index]) ||
    new Set([...rowFields, ...columnFields, ...reportFilterFields]).size !==
      rowFields.length + columnFields.length + reportFilterFields.length
  ) {
    return {
      unsupported: {
        code: 'xlsx.pivots.layout',
        message: 'The pivot layout cannot be represented by the editable Work pivot model.',
        location: locationLabel,
      },
    };
  }
  const reportFilterRows = reportFilters.length ? reportFilters.length + 1 : 0;
  if (outputRange[0].row[0] < reportFilterRows) {
    return {
      unsupported: {
        code: 'xlsx.pivots.report-filter-layout',
        message: 'The report-filter placement cannot be represented by the Work report layout.',
        location: locationLabel,
      },
    };
  }
  const ownedRange = {
    row: [outputRange[0].row[0] - reportFilterRows, outputRange[0].row[1]] as [number, number],
    column: [outputRange[0].column[0], outputRange[0].column[1]] as [number, number],
  };
  const anchor = formatSpreadsheetCellRanges([
    {
      row: [ownedRange.row[0], ownedRange.row[0]],
      column: [ownedRange.column[0], ownedRange.column[0]],
    },
  ]);
  return {
    table: {
      destinationSheetName,
      sourceSheetName: cache.sourceSheetName,
      sourceReference: cache.sourceReference,
      name,
      anchor,
      outputReference: formatSpreadsheetCellRanges([ownedRange]),
      rowFields,
      columnFields,
      reportFilters,
      values,
      rowGrandTotals: booleanAttribute(root, 'rowGrandTotals', true),
      columnGrandTotals: booleanAttribute(root, 'colGrandTotals', true),
      styleName: attribute(directChild(root, 'pivotTableStyleInfo') ?? root, 'name') || 'PivotStyleLight16',
      refreshOnLoad: cache.refreshOnLoad,
    },
  };
}

function readPivotFieldIndexes(parent: Element | undefined): number[] {
  if (!parent) return [];
  return directChildren(parent, 'field').flatMap((field) => {
    const index = integerAttribute(field, 'x');
    return index === null ? [] : [index];
  });
}

async function readRelationshipsDocument(zip: JSZip, path: string): Promise<Document> {
  const entry = zip.file(path);
  return entry
    ? parseXml(await entry.async('text'), path)
    : parseXml(
        `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="${PACKAGE_RELATIONSHIP_NAMESPACE}"/>`,
        path
      );
}

function appendRelationship(document: Document, type: string, target: string): string {
  const id = nextRelationshipId(document);
  const relationship = document.createElementNS(PACKAGE_RELATIONSHIP_NAMESPACE, 'Relationship');
  relationship.setAttribute('Id', id);
  relationship.setAttribute('Type', type);
  relationship.setAttribute('Target', target);
  document.documentElement.append(relationship);
  return id;
}

function removeRelationships(document: Document, typeSuffix: string): void {
  for (const relationship of directChildren(document.documentElement, 'Relationship')) {
    if (attribute(relationship, 'Type')?.endsWith(typeSuffix)) relationship.remove();
  }
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

function relationshipsXml(relationships: Array<{ id: string; type: string; target: string }>): string {
  return [
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
    `<Relationships xmlns="${PACKAGE_RELATIONSHIP_NAMESPACE}">`,
    ...relationships.map(
      (relationship) =>
        `<Relationship Id="${relationship.id}" Type="${relationship.type}" Target="${relationship.target}"/>`
    ),
    '</Relationships>',
  ].join('');
}

function insertWorkbookPivotCaches(document: Document, pivotCaches: Element): void {
  const laterElements = new Set([
    'smartTagPr',
    'smartTagTypes',
    'webPublishing',
    'fileRecoveryPr',
    'webPublishObjects',
    'extLst',
  ]);
  const insertionPoint = directChildren(document.documentElement).find((element) =>
    laterElements.has(element.localName)
  );
  document.documentElement.insertBefore(pivotCaches, insertionPoint ?? null);
}

function ensureRelationshipNamespace(document: Document): void {
  if (document.documentElement.lookupPrefix(DOCUMENT_RELATIONSHIP_NAMESPACE)) return;
  document.documentElement.setAttributeNS(XMLNS_NAMESPACE, 'xmlns:r', DOCUMENT_RELATIONSHIP_NAMESPACE);
}

async function updateContentTypes(zip: JSZip, parts: Array<{ path: string; contentType: string }>): Promise<void> {
  const entry = zip.file('[Content_Types].xml');
  if (!entry) return;
  const document = parseXml(await entry.async('text'), '[Content_Types].xml');
  const namespace = document.documentElement.namespaceURI;
  for (const override of directChildren(document.documentElement, 'Override')) {
    const partName = attribute(override, 'PartName') ?? '';
    if (partName.startsWith('/xl/pivotTables/') || partName.startsWith('/xl/pivotCache/')) override.remove();
  }
  for (const part of parts) {
    const override = document.createElementNS(namespace, 'Override');
    override.setAttribute('PartName', `/${part.path}`);
    override.setAttribute('ContentType', part.contentType);
    document.documentElement.append(override);
  }
  zip.file('[Content_Types].xml', new XMLSerializer().serializeToString(document));
}

function relationshipsPartPath(sourcePart: string): string {
  const segments = sourcePart.split('/');
  const name = segments.pop() ?? '';
  return [...segments, '_rels', `${name}.rels`].join('/');
}

function integerAttribute(element: Element, name: string): number | null {
  const source = attribute(element, name);
  if (source === null || !source.trim()) return null;
  const value = Number(source);
  return Number.isInteger(value) ? value : null;
}

function booleanAttribute(element: Element, name: string, fallback = false): boolean {
  const value = attribute(element, name);
  if (value === null) return fallback;
  return value === '1' || value.toLowerCase() === 'true';
}
