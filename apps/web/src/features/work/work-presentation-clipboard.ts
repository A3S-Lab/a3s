import { createWorkId } from './work-templates';
import type { WorkSlide, WorkSlideElement } from './work-types';

export type WorkPresentationClipboardPayload =
  | { kind: 'element'; element: WorkSlideElement }
  | { kind: 'slide'; slide: WorkSlide };

export interface WorkPresentationClipboardRead {
  payload: WorkPresentationClipboardPayload;
  offset: number;
}

let clipboard: { payload: WorkPresentationClipboardPayload; pasteCount: number } | null = null;

export function copyPresentationElement(element: WorkSlideElement): void {
  clipboard = {
    payload: { kind: 'element', element: structuredCopy(element) },
    pasteCount: 0,
  };
  writeSystemClipboardText(presentationElementPlainText(element));
}

export function copyPresentationSlide(slide: WorkSlide): void {
  clipboard = {
    payload: { kind: 'slide', slide: structuredCopy(slide) },
    pasteCount: 0,
  };
  writeSystemClipboardText(presentationSlidePlainText(slide));
}

export function takePresentationClipboard(): WorkPresentationClipboardRead | null {
  if (!clipboard) return null;
  clipboard.pasteCount += 1;
  return {
    payload: structuredCopy(clipboard.payload),
    offset: Math.min(clipboard.pasteCount * 2, 12),
  };
}

export function clearPresentationClipboard(): void {
  clipboard = null;
}

export function clonePresentationElementForPaste(element: WorkSlideElement, offset: number): WorkSlideElement {
  const copy = structuredCopy(element);
  return {
    ...copy,
    id: createWorkId('element'),
    x: clamp(copy.x + offset, 0, Math.max(0, 100 - copy.width)),
    y: clamp(copy.y + offset, 0, Math.max(0, 100 - copy.height)),
    placeholder: undefined,
  };
}

export function clonePresentationSlideForPaste(slide: WorkSlide): WorkSlide {
  const copy = structuredCopy(slide);
  return {
    ...copy,
    id: createWorkId('slide'),
    name: `${slide.name} 副本`,
    elements: copy.elements.map((element) => ({ ...element, id: createWorkId('element') })),
    comments: copy.comments?.map((comment) => ({ ...comment, id: createWorkId('slide-comment') })),
  };
}

function presentationElementPlainText(element: WorkSlideElement): string {
  if (element.table) return element.table.rows.map((row) => row.join('\t')).join('\n');
  if (element.textRuns?.length) return element.textRuns.map((run) => run.text).join('');
  if (element.text.trim()) return element.text;
  if (element.chart?.title?.trim()) return element.chart.title;
  return element.altText?.trim() || (element.type === 'image' ? '图片' : '演示元素');
}

function presentationSlidePlainText(slide: WorkSlide): string {
  return [slide.name, ...slide.elements.map(presentationElementPlainText), slide.notes ?? '']
    .map((value) => value.trim())
    .filter(Boolean)
    .join('\n');
}

function writeSystemClipboardText(value: string): void {
  if (typeof navigator === 'undefined' || !navigator.clipboard?.writeText) return;
  void navigator.clipboard.writeText(value).catch(() => undefined);
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}

function structuredCopy<T>(value: T): T {
  if (typeof structuredClone === 'function') return structuredClone(value);
  return JSON.parse(JSON.stringify(value)) as T;
}
