export type WorkDocumentFieldKind = 'page' | 'numPages' | 'section' | 'sectionPages' | 'date' | 'time';

export interface WorkDocumentFieldContext {
  pageNumber: number;
  totalPages: number;
  sectionNumber: number;
  sectionPages: number;
  now?: Date;
}

const FIELD_SELECTOR = 'span[data-document-field]';

const FIELD_COMMANDS: Record<WorkDocumentFieldKind, string> = {
  page: 'PAGE',
  numPages: 'NUMPAGES',
  section: 'SECTION',
  sectionPages: 'SECTIONPAGES',
  date: 'DATE \\@ "yyyy年M月d日"',
  time: 'TIME \\@ "HH:mm"',
};

const FIELD_LABELS: Record<WorkDocumentFieldKind, string> = {
  page: '当前页码',
  numPages: '总页数',
  section: '当前节号',
  sectionPages: '本节页数',
  date: '当前日期',
  time: '当前时间',
};

export function documentFieldKind(value: string | undefined): WorkDocumentFieldKind | null {
  if (
    value === 'page' ||
    value === 'numPages' ||
    value === 'section' ||
    value === 'sectionPages' ||
    value === 'date' ||
    value === 'time'
  ) {
    return value;
  }
  return null;
}

export function docxDocumentFieldKind(instruction: string): WorkDocumentFieldKind | null {
  const command = /^\s*([a-z][a-z0-9]*)\b/i.exec(instruction)?.[1]?.toUpperCase();
  if (command === 'PAGE') return 'page';
  if (command === 'NUMPAGES') return 'numPages';
  if (command === 'SECTION') return 'section';
  if (command === 'SECTIONPAGES') return 'sectionPages';
  if (command === 'DATE') return 'date';
  if (command === 'TIME') return 'time';
  return null;
}

export function documentFieldInstruction(kind: WorkDocumentFieldKind): string {
  return FIELD_COMMANDS[kind];
}

export function documentFieldLabel(kind: WorkDocumentFieldKind): string {
  return FIELD_LABELS[kind];
}

export function documentFieldDisplay(
  kind: WorkDocumentFieldKind,
  context: WorkDocumentFieldContext,
  instruction = documentFieldInstruction(kind),
  cachedValue = ''
): string {
  if (kind === 'page') return String(positiveInteger(context.pageNumber));
  if (kind === 'numPages') return String(positiveInteger(context.totalPages));
  if (kind === 'section') return String(positiveInteger(context.sectionNumber));
  if (kind === 'sectionPages') return String(positiveInteger(context.sectionPages));
  const now = validDate(context.now) ?? new Date();
  const format = dateFormatSwitch(instruction) ?? (kind === 'date' ? 'yyyy年M月d日' : 'HH:mm');
  const display = formatWordDate(now, format);
  return display || cachedValue || documentFieldLabel(kind);
}

export function normalizeDocumentFieldsHtml(source: string): string {
  const document = new DOMParser().parseFromString(source, 'text/html');
  const usedIds = new Set<string>();
  for (const [index, element] of Array.from(document.body.querySelectorAll<HTMLElement>(FIELD_SELECTOR)).entries()) {
    const instruction = element.dataset.fieldInstruction?.trim() ?? '';
    const kind = documentFieldKind(element.dataset.fieldKind) ?? docxDocumentFieldKind(instruction);
    if (!kind) {
      element.replaceWith(document.createTextNode(element.textContent ?? ''));
      continue;
    }
    const display = element.dataset.fieldDisplay?.trim() || element.textContent?.trim() || documentFieldLabel(kind);
    element.dataset.documentField = 'true';
    element.dataset.fieldId = uniqueFieldId(element.dataset.fieldId, index + 1, usedIds);
    element.dataset.fieldKind = kind;
    element.dataset.fieldInstruction = instruction || documentFieldInstruction(kind);
    element.dataset.fieldDisplay = display;
    element.classList.add('work-document-field');
    element.textContent = display;
  }
  return document.body.innerHTML;
}

export function resolveDocumentFieldsHtml(source: string, context: WorkDocumentFieldContext): string {
  const document = new DOMParser().parseFromString(normalizeDocumentFieldsHtml(source), 'text/html');
  for (const element of Array.from(document.body.querySelectorAll<HTMLElement>(FIELD_SELECTOR))) {
    const kind = documentFieldKind(element.dataset.fieldKind);
    if (!kind) continue;
    const display = documentFieldDisplay(kind, context, element.dataset.fieldInstruction, element.dataset.fieldDisplay);
    element.dataset.fieldDisplay = display;
    element.textContent = display;
  }
  return document.body.innerHTML;
}

function uniqueFieldId(source: string | undefined, index: number, usedIds: Set<string>): string {
  const candidate = source?.trim();
  if (candidate && !usedIds.has(candidate)) {
    usedIds.add(candidate);
    return candidate;
  }
  let suffix = index;
  while (usedIds.has(`document-field-${suffix}`)) suffix += 1;
  const id = `document-field-${suffix}`;
  usedIds.add(id);
  return id;
}

function dateFormatSwitch(instruction: string): string | null {
  return /\\@\s+"([^"]+)"/i.exec(instruction)?.[1] ?? null;
}

function formatWordDate(date: Date, format: string): string {
  const hour12 = date.getHours() % 12 || 12;
  const replacements: Record<string, string> = {
    'AM/PM': date.getHours() < 12 ? 'AM' : 'PM',
    'am/pm': date.getHours() < 12 ? 'am' : 'pm',
    yyyy: String(date.getFullYear()).padStart(4, '0'),
    yy: String(date.getFullYear() % 100).padStart(2, '0'),
    MMMM: new Intl.DateTimeFormat('zh-CN', { month: 'long' }).format(date),
    MMM: new Intl.DateTimeFormat('zh-CN', { month: 'short' }).format(date),
    MM: String(date.getMonth() + 1).padStart(2, '0'),
    M: String(date.getMonth() + 1),
    dddd: new Intl.DateTimeFormat('zh-CN', { weekday: 'long' }).format(date),
    ddd: new Intl.DateTimeFormat('zh-CN', { weekday: 'short' }).format(date),
    dd: String(date.getDate()).padStart(2, '0'),
    d: String(date.getDate()),
    HH: String(date.getHours()).padStart(2, '0'),
    H: String(date.getHours()),
    hh: String(hour12).padStart(2, '0'),
    h: String(hour12),
    mm: String(date.getMinutes()).padStart(2, '0'),
    m: String(date.getMinutes()),
    ss: String(date.getSeconds()).padStart(2, '0'),
    s: String(date.getSeconds()),
  };
  return format.replace(
    /AM\/PM|am\/pm|yyyy|MMMM|dddd|MMM|ddd|yy|MM|dd|HH|hh|mm|ss|M|d|H|h|m|s/g,
    (token) => replacements[token] ?? token
  );
}

function positiveInteger(value: number): number {
  return Number.isSafeInteger(value) && value > 0 ? value : 1;
}

function validDate(value: Date | undefined): Date | null {
  return value && Number.isFinite(value.getTime()) ? value : null;
}
