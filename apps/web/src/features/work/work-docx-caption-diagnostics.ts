import { docxCaptionBookmark, docxCaptionReferenceTarget, docxCaptionSequenceKind } from './work-docx-caption-fields';
import { docxFieldOccurrences, type DocxFieldOccurrence } from './work-docx-field-instructions';
import { documentCitationTagsFromInstruction } from './work-document-citations';
import { docxDocumentFieldKind } from './work-document-fields';
import { attribute } from './work-ooxml-package';
import type { WorkCompatibilityIssue } from './work-types';

export interface DocxCaptionDiagnostics {
  issues: WorkCompatibilityIssue[];
  hasUnsupportedFields: boolean;
}

export function diagnoseDocxCaptions(document: Document): DocxCaptionDiagnostics {
  const fields = docxFieldOccurrences(document);
  const sequences = fields.filter((field) => docxCaptionSequenceKind(field.instruction));
  const bodyFields = fields.filter((field) => docxDocumentFieldKind(field.instruction));
  const bookmarkNames = captionBookmarkNames(sequences);
  return {
    issues: [
      ...(sequences.length
        ? [
            {
              code: 'docx.captions',
              feature: 'Captions and cross-references',
              message:
                'Figure and table SEQ fields, independent numbering, caption bookmarks, and caption REF fields are preserved, editable, and updated in native DOCX output.',
              severity: 'info',
            } satisfies WorkCompatibilityIssue,
          ]
        : []),
      ...(bodyFields.length
        ? [
            {
              code: 'docx.fields.body',
              feature: 'Body fields',
              message:
                'PAGE, NUMPAGES, SECTION, SECTIONPAGES, DATE, and TIME fields remain live, editable body fields and update in preview, PDF, and native DOCX output.',
              severity: 'info',
            } satisfies WorkCompatibilityIssue,
          ]
        : []),
    ],
    hasUnsupportedFields: fields.some((field) => !isSupportedCaptionField(field.instruction, bookmarkNames)),
  };
}

function captionBookmarkNames(fields: DocxFieldOccurrence[]): Set<string> {
  const names = new Set<string>();
  for (const field of fields) {
    const paragraph = closestAncestor(field.start, 'p');
    if (!paragraph) continue;
    const bookmark = docxCaptionBookmark(paragraph, field);
    const name = attribute(bookmark ?? paragraph, 'name')?.trim();
    if (name) names.add(name);
  }
  return names;
}

function isSupportedCaptionField(instruction: string, bookmarkNames: Set<string>): boolean {
  if (documentCitationTagsFromInstruction(instruction).length || /^\s*BIBLIOGRAPHY\b/i.test(instruction)) {
    return true;
  }
  if (docxDocumentFieldKind(instruction)) return true;
  if (docxCaptionSequenceKind(instruction)) return true;
  const target = docxCaptionReferenceTarget(instruction);
  return Boolean(target && bookmarkNames.has(target));
}

function closestAncestor(element: Element, localName: string): Element | null {
  let current: Element | null = element;
  while (current) {
    if (current.localName === localName) return current;
    current = current.parentElement;
  }
  return null;
}
