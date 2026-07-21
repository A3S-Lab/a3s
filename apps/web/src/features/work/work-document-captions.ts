export type WorkDocumentCaptionKind = 'figure' | 'table';

export interface WorkDocumentCaptionTarget {
  id: string;
  kind: WorkDocumentCaptionKind;
  number: number;
  label: string;
  title: string;
  display: string;
}

const CAPTION_SELECTOR = 'figcaption[data-document-caption]';
const REFERENCE_SELECTOR = 'span[data-document-cross-reference]';

export function normalizeDocumentCaptionsHtml(source: string): string {
  const document = new DOMParser().parseFromString(source, 'text/html');
  const targets = normalizeCaptions(document);
  normalizeReferences(document, new Map(targets.map((target) => [target.id, target] as const)));
  return document.body.innerHTML;
}

export function collectDocumentCaptionTargets(source: string): WorkDocumentCaptionTarget[] {
  const document = new DOMParser().parseFromString(normalizeDocumentCaptionsHtml(source), 'text/html');
  return Array.from(document.body.querySelectorAll<HTMLElement>(CAPTION_SELECTOR)).flatMap((element) => {
    const target = documentCaptionTarget(element);
    return target ? [target] : [];
  });
}

export function documentCaptionKind(value: string | undefined): WorkDocumentCaptionKind | null {
  return value === 'figure' || value === 'table' ? value : null;
}

export function documentCaptionLabel(kind: WorkDocumentCaptionKind): string {
  return kind === 'table' ? '表' : '图';
}

export function documentCaptionDisplay(kind: WorkDocumentCaptionKind, number: number): string {
  return `${documentCaptionLabel(kind)} ${positiveInteger(number)}`;
}

function normalizeCaptions(document: Document): WorkDocumentCaptionTarget[] {
  const counters: Record<WorkDocumentCaptionKind, number> = { figure: 0, table: 0 };
  const usedIds = new Set<string>();
  return Array.from(document.body.querySelectorAll<HTMLElement>(CAPTION_SELECTOR)).map((element, index) => {
    const kind = documentCaptionKind(element.dataset.captionKind) ?? 'figure';
    counters[kind] += 1;
    const number = counters[kind];
    const id = uniqueCaptionId(element.dataset.captionId, kind, index + 1, usedIds);
    const label = documentCaptionLabel(kind);
    element.dataset.documentCaption = 'true';
    element.dataset.captionId = id;
    element.dataset.captionKind = kind;
    element.dataset.captionNumber = String(number);
    element.dataset.captionLabel = label;
    element.classList.add('work-document-caption');
    const title = element.textContent?.replace(/\s+/g, ' ').trim() ?? '';
    return {
      id,
      kind,
      number,
      label,
      title,
      display: documentCaptionDisplay(kind, number),
    };
  });
}

function normalizeReferences(document: Document, targets: Map<string, WorkDocumentCaptionTarget>): void {
  for (const element of Array.from(document.body.querySelectorAll<HTMLElement>(REFERENCE_SELECTOR))) {
    const id = element.dataset.referenceTargetId?.trim() ?? '';
    const target = targets.get(id);
    element.dataset.documentCrossReference = 'true';
    element.dataset.referenceTargetId = id;
    element.classList.add('work-document-cross-reference');
    if (!target) {
      element.dataset.referenceOrphaned = 'true';
      element.textContent = '引用缺失';
      continue;
    }
    delete element.dataset.referenceOrphaned;
    element.dataset.captionKind = target.kind;
    element.dataset.captionNumber = String(target.number);
    element.dataset.captionLabel = target.label;
    element.textContent = target.display;
  }
}

function documentCaptionTarget(element: HTMLElement): WorkDocumentCaptionTarget | null {
  const id = element.dataset.captionId?.trim();
  const kind = documentCaptionKind(element.dataset.captionKind);
  if (!id || !kind) return null;
  const number = positiveInteger(element.dataset.captionNumber);
  return {
    id,
    kind,
    number,
    label: documentCaptionLabel(kind),
    title: element.textContent?.replace(/\s+/g, ' ').trim() ?? '',
    display: documentCaptionDisplay(kind, number),
  };
}

function uniqueCaptionId(
  source: string | undefined,
  kind: WorkDocumentCaptionKind,
  index: number,
  usedIds: Set<string>
): string {
  const candidate = source?.trim();
  if (candidate && !usedIds.has(candidate)) {
    usedIds.add(candidate);
    return candidate;
  }
  let suffix = index;
  while (usedIds.has(`document-${kind}-caption-${suffix}`)) suffix += 1;
  const id = `document-${kind}-caption-${suffix}`;
  usedIds.add(id);
  return id;
}

function positiveInteger(value: unknown): number {
  const number = Number(value);
  return Number.isSafeInteger(number) && number > 0 ? number : 1;
}
