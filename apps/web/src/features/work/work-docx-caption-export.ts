import type { ParagraphChild } from 'docx';
import { documentCaptionKind, documentCaptionLabel } from './work-document-captions';

export function docxCaptionParagraph(
  element: HTMLElement,
  titleChildren: ParagraphChild[],
  docx: typeof import('docx')
): InstanceType<typeof docx.Paragraph> {
  const kind = documentCaptionKind(element.dataset.captionKind) ?? 'figure';
  const number = positiveInteger(element.dataset.captionNumber);
  const bookmarkId = docxCaptionBookmarkId(element.dataset.captionId ?? `${kind}-${number}`);
  return new docx.Paragraph({
    alignment: docx.AlignmentType.CENTER,
    spacing: { after: 160, line: 300 },
    children: [
      new docx.TextRun(`${documentCaptionLabel(kind)} `),
      new docx.Bookmark({
        id: bookmarkId,
        children: [new docx.SequentialIdentifier(kind === 'table' ? 'Table' : 'Figure')],
      }),
      new docx.TextRun('　'),
      ...titleChildren,
    ],
  });
}

export function docxCrossReferenceRuns(element: HTMLElement, docx: typeof import('docx')): ParagraphChild[] {
  if (element.dataset.referenceOrphaned === 'true') return [new docx.TextRun('引用缺失')];
  const kind = documentCaptionKind(element.dataset.captionKind) ?? 'figure';
  const number = positiveInteger(element.dataset.captionNumber);
  const targetId = element.dataset.referenceTargetId?.trim();
  if (!targetId) return [new docx.TextRun('引用缺失')];
  return [
    new docx.TextRun(`${documentCaptionLabel(kind)} `),
    new docx.NumberedItemReference(docxCaptionBookmarkId(targetId), String(number), {
      hyperlink: true,
      referenceFormat: docx.NumberedItemReferenceFormat.NONE,
    }),
  ];
}

export function docxCaptionBookmarkId(source: string): string {
  const safe = source
    .replace(/[^a-z0-9_]/gi, '_')
    .replace(/^([^a-z_])/i, '_$1')
    .slice(0, 18);
  return `A3SCaption_${safe || 'item'}_${stableHash(source)}`.slice(0, 40);
}

function stableHash(source: string): string {
  let hash = 2_166_136_261;
  for (let index = 0; index < source.length; index += 1) {
    hash ^= source.charCodeAt(index);
    hash = Math.imul(hash, 16_777_619);
  }
  return (hash >>> 0).toString(36);
}

function positiveInteger(value: unknown): number {
  const number = Number(value);
  return Number.isSafeInteger(number) && number > 0 ? number : 1;
}
