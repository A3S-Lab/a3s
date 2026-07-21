import { describe, expect, it } from 'vitest';
import { collectDocumentNotes, normalizeDocumentNotesHtml } from './work-document-notes';

describe('Work document notes', () => {
  it('numbers footnotes and endnotes independently and removes orphan definitions', () => {
    const normalized = normalizeDocumentNotesHtml(
      [
        '<p>First',
        '<sup data-document-note-reference="true" data-note-kind="footnote" data-note-id="foot-a" data-note-number="7">7</sup>',
        ' second',
        '<sup data-document-note-reference="true" data-note-kind="footnote" data-note-id="foot-b" data-note-number="9">9</sup>',
        ' end',
        '<sup data-document-note-reference="true" data-note-kind="endnote" data-note-id="end-a" data-note-number="4">4</sup>',
        '</p>',
        '<aside data-document-note="true" data-note-kind="footnote" data-note-id="foot-a"><p>Alpha</p></aside>',
        '<aside data-document-note="true" data-note-kind="footnote" data-note-id="foot-b"><p>Beta</p></aside>',
        '<aside data-document-note="true" data-note-kind="endnote" data-note-id="end-a"><p>Omega</p></aside>',
        '<aside data-document-note="true" data-note-kind="footnote" data-note-id="orphan"><p>Unused</p></aside>',
      ].join('')
    );
    const document = new DOMParser().parseFromString(normalized, 'text/html');

    expect(
      Array.from(document.querySelectorAll<HTMLElement>('[data-document-note-reference]')).map(
        (reference) => `${reference.dataset.noteKind}:${reference.dataset.noteNumber}`
      )
    ).toEqual(['footnote:1', 'footnote:2', 'endnote:1']);
    expect(document.body.textContent).not.toContain('Unused');
    expect(collectDocumentNotes(normalized).notes.map((note) => `${note.kind}:${note.number}:${note.id}`)).toEqual([
      'footnote:1:foot-a',
      'footnote:2:foot-b',
      'endnote:1:end-a',
    ]);
  });

  it('creates an editable empty definition for a reference whose body is missing', () => {
    const normalized = normalizeDocumentNotesHtml(
      '<p>Reference<sup data-document-note-reference="true" data-note-kind="footnote" data-note-id="missing">8</sup></p>'
    );
    const document = new DOMParser().parseFromString(normalized, 'text/html');
    const definition = document.querySelector<HTMLElement>('[data-document-note][data-note-id="missing"]');

    expect(definition?.dataset.noteNumber).toBe('1');
    expect(definition?.querySelector('p')).not.toBeNull();
  });
});
