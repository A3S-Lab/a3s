import type { ParagraphChild } from 'docx';
import { docxDocumentFieldKind } from './work-document-fields';

export function docxDocumentFieldRun(element: HTMLElement, docx: typeof import('docx')): ParagraphChild {
  const instruction = element.dataset.fieldInstruction?.trim() ?? '';
  const display = element.dataset.fieldDisplay?.trim() || element.textContent?.trim() || '';
  if (!docxDocumentFieldKind(instruction)) return new docx.TextRun(display);
  return new docx.SimpleField(instruction, display);
}
