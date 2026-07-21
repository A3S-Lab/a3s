import { readDocxBibliography } from './work-docx-bibliography';
import { docxFieldOccurrences } from './work-docx-field-instructions';
import { documentCitationTagsFromInstruction } from './work-document-citations';
import type { OoxmlPackage } from './work-ooxml-package';
import type { WorkCompatibilityIssue } from './work-types';

export async function diagnoseDocxCitations(
  archive: OoxmlPackage,
  document: Document
): Promise<WorkCompatibilityIssue[]> {
  const fields = docxFieldOccurrences(document);
  const citationFields = fields
    .map((field) => ({
      instruction: field.instruction,
      tags: documentCitationTagsFromInstruction(field.instruction),
    }))
    .filter((field) => field.tags.length);
  const bibliographyFields = fields.filter((field) => /^\s*BIBLIOGRAPHY\b/i.test(field.instruction));
  const result = await readDocxBibliography(archive);
  const sourceTags = new Set(result.bibliography?.sources.map((source) => source.tag) ?? []);
  const citedTags = Array.from(new Set(citationFields.flatMap((field) => field.tags)));
  const missingTags = citedTags.filter((tag) => !sourceTags.has(tag));
  const issues: WorkCompatibilityIssue[] = [];

  if (citationFields.length || result.bibliography?.sources.length) {
    issues.push({
      code: 'docx.citations',
      feature: 'Citations and bibliography sources',
      message: `${citationFields.length} citation field(s) and ${
        result.bibliography?.sources.length ?? 0
      } bibliography source(s) retain tags, authors, source metadata, style selection, cached display values, and native DOCX field/custom-XML structure.`,
      severity: 'info',
    });
  }
  if (bibliographyFields.length) {
    issues.push({
      code: 'docx.bibliography',
      feature: 'Bibliography',
      message: `${bibliographyFields.length} live bibliography field(s) remain editable and regenerate from the document source library.`,
      severity: 'info',
    });
  }
  if (missingTags.length) {
    issues.push({
      code: 'docx.citations.missing-source',
      feature: 'Citations and bibliography sources',
      message: `Citation source${missingTags.length === 1 ? '' : 's'} ${missingTags.join(
        ', '
      )} could not be found in a connected Word bibliography source part and will be shown as missing.`,
      severity: 'warning',
    });
  }
  if (result.unreadablePartCount) {
    issues.push({
      code: 'docx.citations.unreadable',
      feature: 'Citations and bibliography sources',
      message: `${result.unreadablePartCount} bibliography custom-XML part(s) could not be read and remain available only in the original file.`,
      severity: 'warning',
    });
  }
  if (result.duplicateTags.length) {
    issues.push({
      code: 'docx.citations.duplicate-tags',
      feature: 'Citations and bibliography sources',
      message: `Duplicate bibliography tags ${result.duplicateTags.join(
        ', '
      )} are assigned unique editable tags during import.`,
      severity: 'warning',
    });
  }
  if (result.uncommonSourceTypes.length) {
    issues.push({
      code: 'docx.citations.source-types',
      feature: 'Citations and bibliography sources',
      message: `Uncommon source type${result.uncommonSourceTypes.length === 1 ? '' : 's'} ${result.uncommonSourceTypes.join(
        ', '
      )} retain their original type and additional metadata; the editor exposes the common source fields directly.`,
      severity: 'info',
    });
  }
  if (result.uncommonStyle) {
    issues.push({
      code: 'docx.citations.style',
      feature: 'Citations and bibliography sources',
      message: `Citation style ${result.uncommonStyle} is retained for DOCX output; Work preview uses its editable APA fallback until a supported APA, MLA, Chicago, or IEEE style is selected.`,
      severity: 'warning',
    });
  }
  return issues;
}
