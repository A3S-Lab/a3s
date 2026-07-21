import type { ParagraphChild } from 'docx';
import { documentCitationInstruction, documentCitationTags } from './work-document-citations';

export function docxCitationRun(element: HTMLElement, docx: typeof import('docx')): ParagraphChild {
  const tags = documentCitationTags(element.dataset.citationTags);
  const instruction = element.dataset.citationInstruction?.trim() || documentCitationInstruction(tags);
  const display = element.dataset.citationDisplay?.trim() || element.textContent?.trim() || '缺失引文';
  if (!tags.length || !/^\s*CITATION\b/i.test(instruction)) {
    return new docx.TextRun(display);
  }
  return new docx.SimpleField(instruction, display);
}

export function docxBibliographyParagraph(
  element: HTMLElement,
  docx: typeof import('docx')
): InstanceType<typeof docx.Paragraph> {
  const instruction = element.dataset.bibliographyInstruction?.trim() || 'BIBLIOGRAPHY \\l 2052';
  const cached = Array.from(element.querySelectorAll<HTMLElement>('[data-bibliography-entry]'))
    .map((entry) => entry.textContent?.trim() ?? '')
    .filter(Boolean)
    .join('\n');
  return new docx.Paragraph({
    children: [new docx.SimpleField(instruction, cached || '参考文献')],
    spacing: { before: 240, after: 120, line: 320 },
  });
}
