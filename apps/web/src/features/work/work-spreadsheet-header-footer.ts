import type { WorkSpreadsheetHeaderFooterSections } from './work-types';

export type EffectiveSpreadsheetHeaderFooterSections = Required<WorkSpreadsheetHeaderFooterSections>;

export const SPREADSHEET_HEADER_FOOTER_TOKENS = [
  '{page}',
  '{pages}',
  '{sheet}',
  '{file}',
  '{path}',
  '{date}',
  '{time}',
] as const;

export interface SpreadsheetHeaderFooterTokenContext {
  page: number;
  pages: number;
  sheetName: string;
  fileName: string;
  filePath?: string;
  now?: Date;
}

export function effectiveSpreadsheetHeaderFooterSections(
  sections: WorkSpreadsheetHeaderFooterSections | undefined
): EffectiveSpreadsheetHeaderFooterSections {
  return {
    left: sections?.left ?? '',
    center: sections?.center ?? '',
    right: sections?.right ?? '',
  };
}

export function hasSpreadsheetHeaderFooterSections(sections: WorkSpreadsheetHeaderFooterSections | undefined): boolean {
  return Boolean(sections && Object.values(sections).some((value) => value));
}

export function resolveSpreadsheetHeaderFooterTemplate(
  template: string,
  context: SpreadsheetHeaderFooterTokenContext
): string {
  const now = context.now ?? new Date();
  const values: Record<(typeof SPREADSHEET_HEADER_FOOTER_TOKENS)[number], string> = {
    '{page}': String(context.page),
    '{pages}': String(context.pages),
    '{sheet}': context.sheetName,
    '{file}': context.fileName,
    '{path}': context.filePath ?? '',
    '{date}': formatDate(now),
    '{time}': formatTime(now),
  };
  return template.replace(
    /\{(?:page|pages|sheet|file|path|date|time)\}/g,
    (token) => values[token as keyof typeof values]
  );
}

function formatDate(value: Date): string {
  return `${value.getFullYear()}-${twoDigits(value.getMonth() + 1)}-${twoDigits(value.getDate())}`;
}

function formatTime(value: Date): string {
  return `${twoDigits(value.getHours())}:${twoDigits(value.getMinutes())}`;
}

function twoDigits(value: number): string {
  return String(value).padStart(2, '0');
}
