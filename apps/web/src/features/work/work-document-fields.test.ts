import { describe, expect, it } from 'vitest';
import { documentPageDescriptors } from './work-document-pages';
import { documentFieldDisplay, normalizeDocumentFieldsHtml, resolveDocumentFieldsHtml } from './work-document-fields';

describe('Work document body fields', () => {
  it('normalizes field metadata and assigns stable unique identifiers', () => {
    const html = normalizeDocumentFieldsHtml(
      [
        '<p><span data-document-field="true" data-field-id="field" data-field-kind="page">4</span>',
        '<span data-document-field="true" data-field-id="field" data-field-instruction="NUMPAGES">9</span></p>',
      ].join('')
    );
    const document = new DOMParser().parseFromString(html, 'text/html');
    const fields = document.querySelectorAll<HTMLElement>('[data-document-field]');

    expect(fields).toHaveLength(2);
    expect(fields[0].dataset.fieldId).toBe('field');
    expect(fields[0].dataset.fieldInstruction).toBe('PAGE');
    expect(fields[1].dataset.fieldId).not.toBe('field');
    expect(fields[1].dataset.fieldKind).toBe('numPages');
    expect(fields[1].dataset.fieldDisplay).toBe('9');
  });

  it('resolves page and section fields independently on each explicit page', () => {
    const pages = documentPageDescriptors({
      type: 'document',
      pageSize: 'a4',
      html: [
        '<p>Page <span data-document-field="true" data-field-kind="page">0</span>',
        ' of <span data-document-field="true" data-field-kind="numPages">0</span></p>',
        '<div data-page-break="true"></div>',
        '<p>Section pages <span data-document-field="true" data-field-kind="sectionPages">0</span>',
        ' · page <span data-document-field="true" data-field-kind="page">0</span></p>',
      ].join(''),
    });

    expect(pages).toHaveLength(2);
    expect(pages[0].segments[0].html).toContain('data-field-display="1"');
    expect(pages[0].segments[0].html).toContain('data-field-display="2"');
    expect(pages[1].segments[0].html).toContain('Section pages');
    expect(pages[1].segments[0].html.match(/data-field-display="2"/g)).toHaveLength(2);
  });

  it('formats editable DATE and TIME switches with one deterministic timestamp', () => {
    const context = {
      pageNumber: 1,
      totalPages: 1,
      sectionNumber: 1,
      sectionPages: 1,
      now: new Date('2026-07-21T14:05:09'),
    };

    expect(documentFieldDisplay('date', context, 'DATE \\@ "yyyy-MM-dd"')).toBe('2026-07-21');
    expect(documentFieldDisplay('time', context, 'TIME \\@ "HH:mm:ss"')).toBe('14:05:09');
    expect(
      resolveDocumentFieldsHtml(
        '<span data-document-field="true" data-field-kind="date" data-field-instruction=\'DATE \\@ "yyyy年M月d日"\'>old</span>',
        context
      )
    ).toContain('2026年7月21日');
  });
});
