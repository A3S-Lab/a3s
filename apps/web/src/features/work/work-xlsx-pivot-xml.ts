import type { Cell } from '@fortune-sheet/core';
import { spreadsheetPivotFilterValueKey } from './work-spreadsheet-pivot-values';
import { formatSpreadsheetCellRanges, parseSpreadsheetCellRanges } from './work-spreadsheet-ranges';
import { spreadsheetPivotFields } from './work-spreadsheet-pivots';
import type {
  WorkSpreadsheetContent,
  WorkSpreadsheetPivotAggregation,
  WorkSpreadsheetPivotTable,
  WorkSpreadsheetSheet,
} from './work-types';

const SPREADSHEET_NAMESPACE = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main';
const RELATIONSHIP_NAMESPACE = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';

export interface XlsxPivotXmlParts {
  table: string;
  cacheDefinition: string;
  cacheRecords: string;
}

interface PivotSourceValue {
  value: unknown;
  type: 'blank' | 'number' | 'string' | 'boolean' | 'date' | 'error';
}

interface PivotCacheField {
  name: string;
  values: PivotSourceValue[];
  sharedItems?: PivotSourceValue[];
}

export function createXlsxPivotXmlParts(
  content: WorkSpreadsheetContent,
  pivot: WorkSpreadsheetPivotTable,
  cacheId: number
): XlsxPivotXmlParts | null {
  const sourceSheet = content.sheets.find((sheet) => sheet.id === pivot.sourceSheetId);
  const ranges = parseSpreadsheetCellRanges(pivot.sourceReference);
  const fields = spreadsheetPivotFields(content, pivot);
  if (!sourceSheet || ranges?.length !== 1 || !fields.length || !pivot.outputReference) return null;
  const sourceRange = ranges[0];
  const dimensionIndexes = new Set([
    ...pivot.rowFields,
    ...pivot.columnFields,
    ...(pivot.reportFilters ?? []).map((filter) => filter.fieldIndex),
  ]);
  const cacheFields: PivotCacheField[] = fields.map((field) => {
    const values: PivotSourceValue[] = [];
    for (let row = sourceRange.row[0] + 1; row <= sourceRange.row[1]; row += 1) {
      values.push(pivotSourceValue(sourceSheet.data?.[row]?.[sourceRange.column[0] + field.index]));
    }
    return {
      name: field.name,
      values,
      sharedItems: dimensionIndexes.has(field.index) ? uniqueSortedPivotValues(values) : undefined,
    };
  });
  return {
    table: pivotTableXml(pivot, cacheFields, cacheId),
    cacheDefinition: pivotCacheDefinitionXml(sourceSheet, pivot, cacheFields),
    cacheRecords: pivotCacheRecordsXml(cacheFields),
  };
}

export function xlsxPivotAggregation(aggregation: WorkSpreadsheetPivotAggregation): string {
  if (aggregation === 'count') return 'countNums';
  if (aggregation === 'counta') return 'count';
  if (aggregation === 'stdDevP') return 'stdDevp';
  if (aggregation === 'varP') return 'varp';
  return aggregation;
}

export function workPivotAggregation(value: string | null): WorkSpreadsheetPivotAggregation | null {
  if (!value || value === 'sum') return 'sum';
  if (value === 'countNums') return 'count';
  if (value === 'count') return 'counta';
  if (value === 'average') return 'average';
  if (value === 'max') return 'max';
  if (value === 'min') return 'min';
  if (value === 'product') return 'product';
  if (value === 'stdDev') return 'stdDev';
  if (value === 'stdDevp') return 'stdDevP';
  if (value === 'var') return 'var';
  if (value === 'varp') return 'varP';
  return null;
}

function pivotTableXml(pivot: WorkSpreadsheetPivotTable, fields: PivotCacheField[], cacheId: number): string {
  const dataFieldIndexes = new Set(pivot.values.map((value) => value.fieldIndex));
  const reportFilterIndexes = new Set((pivot.reportFilters ?? []).map((filter) => filter.fieldIndex));
  const pivotFields = fields
    .map((field, fieldIndex) => {
      const row = pivot.rowFields.includes(fieldIndex);
      const column = pivot.columnFields.includes(fieldIndex);
      const page = reportFilterIndexes.has(fieldIndex);
      const attributes = [
        row ? 'axis="axisRow"' : '',
        column ? 'axis="axisCol"' : '',
        page ? 'axis="axisPage"' : '',
        dataFieldIndexes.has(fieldIndex) ? 'dataField="1"' : '',
        'compact="0"',
        'outline="0"',
        'showAll="0"',
        'defaultSubtotal="0"',
      ]
        .filter(Boolean)
        .join(' ');
      if (!row && !column && !page) return `<pivotField ${attributes}/>`;
      const items = field.sharedItems ?? [];
      return [
        `<pivotField ${attributes}>`,
        `<items count="${items.length + 1}">`,
        ...items.map((_item, index) => `<item x="${index}"/>`),
        '<item t="default"/>',
        '</items>',
        '</pivotField>',
      ].join('');
    })
    .join('');
  const rowFields = pivot.rowFields.map((fieldIndex) => `<field x="${fieldIndex}"/>`).join('');
  const columnFieldIndexes = [...pivot.columnFields];
  if (pivot.values.length > 1) columnFieldIndexes.push(-2);
  const columnFields = columnFieldIndexes.map((fieldIndex) => `<field x="${fieldIndex}"/>`).join('');
  const pageFields = (pivot.reportFilters ?? [])
    .map((filter) => {
      const items = fields[filter.fieldIndex]?.sharedItems ?? [];
      const selectedIndex =
        filter.selectedItem === undefined
          ? items.length
          : items.findIndex(
              (item) => pivotSourceFilterKey(item) === spreadsheetPivotFilterValueKey(filter.selectedItem!)
            );
      return `<pageField fld="${filter.fieldIndex}" hier="-1" item="${
        selectedIndex >= 0 ? selectedIndex : items.length
      }"/>`;
    })
    .join('');
  const dataFields = pivot.values
    .map((value) => {
      const caption = value.caption?.trim() || fields[value.fieldIndex]?.name || `Value ${value.fieldIndex + 1}`;
      return `<dataField name="${escapeXml(caption)}" fld="${value.fieldIndex}" subtotal="${xlsxPivotAggregation(
        value.aggregation
      )}" baseField="0" baseItem="0"/>`;
    })
    .join('');
  return [
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
    `<pivotTableDefinition xmlns="${SPREADSHEET_NAMESPACE}" name="${escapeXml(pivot.name)}" cacheId="${cacheId}"`,
    ' applyNumberFormats="0" applyBorderFormats="0" applyFontFormats="0" applyPatternFormats="0"',
    ' applyAlignmentFormats="0" applyWidthHeightFormats="1" dataCaption="值" updatedVersion="8"',
    ` minRefreshableVersion="3" createdVersion="8" useAutoFormatting="1" compact="0" compactData="0"`,
    ` rowGrandTotals="${pivot.rowGrandTotals ? 1 : 0}" colGrandTotals="${pivot.columnGrandTotals ? 1 : 0}">`,
    `<location ref="${escapeXml(pivotTableReference(pivot))}" firstHeaderRow="1" firstDataRow="1" firstDataCol="${Math.max(
      1,
      pivot.rowFields.length
    )}"/>`,
    `<pivotFields count="${fields.length}">${pivotFields}</pivotFields>`,
    `<rowFields count="${pivot.rowFields.length}">${rowFields}</rowFields>`,
    '<rowItems count="1"><i t="grand"><x/></i></rowItems>',
    columnFieldIndexes.length ? `<colFields count="${columnFieldIndexes.length}">${columnFields}</colFields>` : '',
    '<colItems count="1"><i t="grand"><x/></i></colItems>',
    pageFields ? `<pageFields count="${pivot.reportFilters!.length}">${pageFields}</pageFields>` : '',
    `<dataFields count="${pivot.values.length}">${dataFields}</dataFields>`,
    `<pivotTableStyleInfo name="${escapeXml(
      pivot.styleName || 'PivotStyleLight16'
    )}" showRowHeaders="1" showColHeaders="1" showRowStripes="0" showColStripes="0" showLastColumn="1"/>`,
    '</pivotTableDefinition>',
  ].join('');
}

function pivotTableReference(pivot: WorkSpreadsheetPivotTable): string {
  const ranges = parseSpreadsheetCellRanges(pivot.outputReference ?? '');
  const reportFilterRows = pivot.reportFilters?.length ? pivot.reportFilters.length + 1 : 0;
  if (ranges?.length !== 1 || !reportFilterRows) return pivot.outputReference ?? pivot.anchor;
  const range = ranges[0];
  const tableStartRow = range.row[0] + reportFilterRows;
  if (tableStartRow > range.row[1]) return pivot.outputReference ?? pivot.anchor;
  return formatSpreadsheetCellRanges([
    {
      row: [tableStartRow, range.row[1]],
      column: [range.column[0], range.column[1]],
    },
  ]);
}

function pivotCacheDefinitionXml(
  sourceSheet: WorkSpreadsheetSheet,
  pivot: WorkSpreadsheetPivotTable,
  fields: PivotCacheField[]
): string {
  const recordCount = fields[0]?.values.length ?? 0;
  return [
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
    `<pivotCacheDefinition xmlns="${SPREADSHEET_NAMESPACE}" xmlns:r="${RELATIONSHIP_NAMESPACE}" r:id="rId1"`,
    ` saveData="1" refreshOnLoad="${pivot.refreshOnLoad ? 1 : 0}" enableRefresh="1"`,
    ` createdVersion="8" refreshedVersion="8" minRefreshableVersion="3" recordCount="${recordCount}">`,
    '<cacheSource type="worksheet">',
    `<worksheetSource ref="${escapeXml(pivot.sourceReference)}" sheet="${escapeXml(sourceSheet.name)}"/>`,
    '</cacheSource>',
    `<cacheFields count="${fields.length}">`,
    ...fields.map((field) => pivotCacheFieldXml(field)),
    '</cacheFields>',
    '</pivotCacheDefinition>',
  ].join('');
}

function pivotCacheFieldXml(field: PivotCacheField): string {
  const values = field.sharedItems ?? field.values;
  const statistics = pivotValueStatistics(values);
  const attributes = [
    `containsBlank="${statistics.blank ? 1 : 0}"`,
    `containsString="${statistics.string ? 1 : 0}"`,
    `containsNumber="${statistics.number ? 1 : 0}"`,
    `containsInteger="${statistics.integer ? 1 : 0}"`,
    `containsDate="${statistics.date ? 1 : 0}"`,
    `containsMixedTypes="${statistics.typeCount > 1 ? 1 : 0}"`,
    `containsSemiMixedTypes="${statistics.string && statistics.typeCount > 1 ? 1 : 0}"`,
  ];
  if (!field.sharedItems) {
    return `<cacheField name="${escapeXml(field.name)}" numFmtId="0"><sharedItems ${attributes.join(
      ' '
    )}/></cacheField>`;
  }
  return [
    `<cacheField name="${escapeXml(field.name)}" numFmtId="0">`,
    `<sharedItems count="${field.sharedItems.length}" ${attributes.join(' ')}>`,
    ...field.sharedItems.map((value) => pivotCacheValueXml(value)),
    '</sharedItems>',
    '</cacheField>',
  ].join('');
}

function pivotCacheRecordsXml(fields: PivotCacheField[]): string {
  const recordCount = fields[0]?.values.length ?? 0;
  const sharedIndexes = fields.map((field) =>
    field.sharedItems
      ? new Map(field.sharedItems.map((value, index) => [pivotValueKey(value), index] as const))
      : undefined
  );
  const records: string[] = [];
  for (let row = 0; row < recordCount; row += 1) {
    records.push('<r>');
    for (let column = 0; column < fields.length; column += 1) {
      const value = fields[column].values[row];
      const sharedIndex = sharedIndexes[column]?.get(pivotValueKey(value));
      records.push(sharedIndex === undefined ? pivotCacheValueXml(value) : `<x v="${sharedIndex}"/>`);
    }
    records.push('</r>');
  }
  return [
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
    `<pivotCacheRecords xmlns="${SPREADSHEET_NAMESPACE}" count="${recordCount}">`,
    ...records,
    '</pivotCacheRecords>',
  ].join('');
}

function pivotSourceValue(cell: Cell | null | undefined): PivotSourceValue {
  const value: unknown = cell?.v ?? cell?.m ?? null;
  if (value === null || value === undefined || value === '') return { value: null, type: 'blank' };
  if (cell?.ct?.t === 'e') return { value: String(cell.m ?? cell.v ?? '#VALUE!'), type: 'error' };
  if (value instanceof Date) return { value, type: 'date' };
  if (typeof value === 'number' && Number.isFinite(value)) return { value, type: 'number' };
  if (typeof value === 'boolean') return { value, type: 'boolean' };
  return { value: String(value), type: 'string' };
}

function uniqueSortedPivotValues(values: PivotSourceValue[]): PivotSourceValue[] {
  const unique = new Map(values.map((value) => [pivotValueKey(value), value]));
  return Array.from(unique.values()).sort((left, right) =>
    pivotValueDisplay(left).localeCompare(pivotValueDisplay(right), 'zh-CN', {
      numeric: true,
      sensitivity: 'base',
    })
  );
}

function pivotCacheValueXml(value: PivotSourceValue): string {
  if (value.type === 'blank') return '<m/>';
  if (value.type === 'number') return `<n v="${Number(value.value)}"/>`;
  if (value.type === 'boolean') return `<b v="${value.value ? 1 : 0}"/>`;
  if (value.type === 'date') {
    const date = value.value instanceof Date ? value.value.toISOString() : String(value.value);
    return `<d v="${escapeXml(date)}"/>`;
  }
  if (value.type === 'error') return `<e v="${escapeXml(String(value.value))}"/>`;
  return `<s v="${escapeXml(String(value.value))}"/>`;
}

function pivotValueKey(value: PivotSourceValue): string {
  const raw = value.value instanceof Date ? value.value.toISOString() : value.value;
  return `${value.type}:${String(raw ?? '')}`;
}

function pivotSourceFilterKey(value: PivotSourceValue): string {
  if (value.type === 'blank') return 'blank:';
  if (value.type === 'date') {
    return `string:${value.value instanceof Date ? value.value.toISOString() : String(value.value)}`;
  }
  if (value.type === 'boolean') return `boolean:${String(Boolean(value.value))}`;
  if (value.type === 'number') return `number:${String(Number(value.value))}`;
  return `string:${String(value.value ?? '')}`;
}

function pivotValueDisplay(value: PivotSourceValue): string {
  if (value.value instanceof Date) return value.value.toISOString();
  return String(value.value ?? '');
}

function pivotValueStatistics(values: PivotSourceValue[]) {
  const types = new Set(values.filter((value) => value.type !== 'blank').map((value) => value.type));
  return {
    blank: values.some((value) => value.type === 'blank'),
    string: types.has('string') || types.has('error'),
    number: types.has('number'),
    integer:
      types.has('number') &&
      values.filter((value) => value.type === 'number').every((value) => Number.isInteger(Number(value.value))),
    date: types.has('date'),
    typeCount: types.size,
  };
}

function escapeXml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&apos;');
}
