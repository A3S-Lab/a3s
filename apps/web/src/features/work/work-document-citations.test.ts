import { describe, expect, it } from 'vitest';
import {
  createDocumentBibliography,
  documentBibliographyEntry,
  documentCitationInstruction,
  documentCitationTagsFromInstruction,
  normalizeDocumentCitationsHtml,
  renameDocumentCitationTagInInstruction,
  renderDocumentBibliographyHtml,
  resolveDocumentCitation,
} from './work-document-citations';
import type { WorkDocumentBibliography, WorkDocumentCitationSource } from './work-types';

const sources: WorkDocumentCitationSource[] = [
  {
    id: 'source-smith',
    tag: 'Smith2026',
    sourceType: 'Book',
    title: 'Agentic Systems',
    year: '2026',
    contributors: {
      Author: {
        people: [
          { first: 'Jane', last: 'Smith' },
          { first: 'Ming', last: 'Li' },
        ],
      },
    },
    publisher: 'A3S Press',
  },
  {
    id: 'source-lab',
    tag: 'A3SLab2025',
    sourceType: 'Report',
    title: 'Work Research',
    year: '2025',
    contributors: { Author: { corporate: 'A3S Lab' } },
    institution: 'A3S Lab',
  },
];

describe('Work document citations', () => {
  it('parses and emits native multi-source CITATION instructions', () => {
    const instruction = documentCitationInstruction(['Smith2026', 'A3SLab2025']);

    expect(instruction).toBe('CITATION Smith2026 \\m A3SLab2025 \\l 2052');
    expect(documentCitationTagsFromInstruction(` ${instruction} \\p "p. 8" `)).toEqual(['Smith2026', 'A3SLab2025']);
    expect(renameDocumentCitationTagInInstruction(`${instruction} \\p "p. 8"`, 'Smith2026', 'Smith2027')).toBe(
      'CITATION Smith2027 \\m A3SLab2025 \\l 2052 \\p "p. 8"'
    );
  });

  it('renders APA, IEEE, and missing-source citations deterministically', () => {
    const apa: WorkDocumentBibliography = { ...createDocumentBibliography('apa'), sources };
    const ieee: WorkDocumentBibliography = { ...createDocumentBibliography('ieee'), sources };

    expect(resolveDocumentCitation(['Smith2026'], apa)).toEqual({
      text: '(Smith & Li, 2026)',
      orphaned: false,
    });
    expect(resolveDocumentCitation(['Smith2026', 'A3SLab2025'], ieee)).toEqual({
      text: '[1, 2]',
      orphaned: false,
    });
    expect(resolveDocumentCitation(['Missing'], apa)).toEqual({
      text: '缺失引文：Missing',
      orphaned: true,
    });
  });

  it('normalizes inline citations and regenerates bibliography blocks', () => {
    const bibliography: WorkDocumentBibliography = {
      ...createDocumentBibliography('apa'),
      sources,
    };
    const html = normalizeDocumentCitationsHtml(
      [
        '<p>See <span data-document-citation="true" data-citation-tags="Smith2026">old</span>.</p>',
        renderDocumentBibliographyHtml(bibliography),
      ].join(''),
      bibliography
    );
    const document = new DOMParser().parseFromString(html, 'text/html');
    const citation = document.querySelector<HTMLElement>('[data-document-citation]');
    const entries = document.querySelectorAll<HTMLElement>('[data-bibliography-entry]');

    expect(citation?.textContent).toBe('(Smith & Li, 2026)');
    expect(citation?.dataset.citationInstruction).toContain('CITATION Smith2026');
    expect(entries).toHaveLength(2);
    expect(entries[0].textContent).toBe(documentBibliographyEntry(sources[0], bibliography, 0));
    expect(entries[1].textContent).toContain('A3S Lab');
  });
});
