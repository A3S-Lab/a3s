import { attribute, descendants, directChildren, type OoxmlPackage } from './work-ooxml-package';
import type { WorkCompatibilityIssue } from './work-types';

export async function diagnoseDocxNotes(
  archive: OoxmlPackage,
  document: Document | null
): Promise<WorkCompatibilityIssue[]> {
  const noteParts = ['word/footnotes.xml', 'word/endnotes.xml'].filter((part) => archive.has(part));
  const noteDocuments = await Promise.all(noteParts.map((part) => archive.xml(part)));
  const hasNotes =
    Boolean(
      document &&
        (descendants(document, 'footnoteReference').length || descendants(document, 'endnoteReference').length)
    ) ||
    noteDocuments.some((notes) =>
      [...descendants(notes, 'footnote'), ...descendants(notes, 'endnote')].some(
        (note) => Number(attribute(note, 'id')) > 0
      )
    );
  if (!hasNotes) return [];

  const issues: WorkCompatibilityIssue[] = [
    noteIssue(
      'docx.notes',
      'Footnote and endnote references, editable note text, common inline formatting, preview placement, and native DOCX note parts are preserved.',
      'info'
    ),
  ];
  if (
    noteDocuments.some(
      (notes) =>
        descendants(notes, 'tbl').length > 0 ||
        descendants(notes, 'drawing').length > 0 ||
        descendants(notes, 'pict').length > 0
    )
  ) {
    issues.push(
      noteIssue(
        'docx.notes.rich-content',
        'Tables, drawings, and embedded media inside notes may be flattened or converted to inline content.'
      )
    );
  }

  const settings = archive.has('word/settings.xml') ? await archive.xml('word/settings.xml') : null;
  const noteProperties = [
    ...(document ? descendants(document, 'footnotePr') : []),
    ...(document ? descendants(document, 'endnotePr') : []),
    ...(settings ? descendants(settings, 'footnotePr') : []),
    ...(settings ? descendants(settings, 'endnotePr') : []),
  ];
  if (noteProperties.some((properties) => directChildren(properties).length > 0)) {
    issues.push(
      noteIssue(
        'docx.notes.numbering',
        'Custom note symbols, numbering formats, restart rules, separators, and placement settings normalize to continuous Arabic numbering with footnotes per page and endnotes at document end.'
      )
    );
  }
  return issues;
}

function noteIssue(
  code: string,
  message: string,
  severity: WorkCompatibilityIssue['severity'] = 'warning'
): WorkCompatibilityIssue {
  return {
    code,
    feature: 'Footnotes and endnotes',
    message,
    severity,
  };
}
