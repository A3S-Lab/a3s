import { createDocumentNoteElement, type WorkDocumentNote, type WorkDocumentNoteKind } from './work-document-notes';

export function extractMammothDocumentNotes(document: Document): WorkDocumentNote[] {
  const definitions = extractDefinitions(document);
  const notes = new Map<string, WorkDocumentNote>();
  const counters: Record<WorkDocumentNoteKind, number> = { footnote: 0, endnote: 0 };
  const references = Array.from(
    document.body.querySelectorAll<HTMLAnchorElement>('a[href^="#footnote-"], a[href^="#endnote-"]')
  );
  for (const reference of references) {
    const target = reference.getAttribute('href')?.slice(1) ?? '';
    const kind = noteKindFromTarget(target);
    if (!kind) continue;
    let note = notes.get(target);
    if (!note) {
      counters[kind] += 1;
      note = {
        id: `document-${kind}-${counters[kind]}`,
        kind,
        number: counters[kind],
        html: definitions.get(target) ?? '<p></p>',
      };
      notes.set(target, note);
    }
    const marker = document.createElement('sup');
    marker.setAttribute('data-document-note-reference', 'true');
    marker.setAttribute('data-note-kind', note.kind);
    marker.setAttribute('data-note-id', note.id);
    marker.setAttribute('data-note-number', String(note.number));
    marker.textContent = String(note.number);
    const wrapper = reference.parentElement;
    if (wrapper?.tagName.toLowerCase() === 'sup' && wrapper.childNodes.length === 1) wrapper.replaceWith(marker);
    else reference.replaceWith(marker);
  }
  return Array.from(notes.values());
}

export function placeMammothDocumentNotes(document: Document, notes: WorkDocumentNote[]) {
  const sections = Array.from(document.body.querySelectorAll<HTMLElement>('section[data-document-section]'));
  for (const note of notes) {
    const reference = Array.from(document.body.querySelectorAll<HTMLElement>('sup[data-document-note-reference]')).find(
      (element) => element.dataset.noteKind === note.kind && element.dataset.noteId === note.id
    );
    const target =
      note.kind === 'footnote' ? reference?.closest<HTMLElement>('section[data-document-section]') : sections.at(-1);
    (target ?? document.body).append(createDocumentNoteElement(document, note));
  }
}

function extractDefinitions(document: Document): Map<string, string> {
  const definitions = new Map<string, string>();
  const elements = Array.from(document.body.querySelectorAll<HTMLElement>('li[id^="footnote-"], li[id^="endnote-"]'));
  for (const element of elements) {
    const id = element.id;
    const clone = element.cloneNode(true) as HTMLElement;
    for (const backlink of Array.from(
      clone.querySelectorAll<HTMLAnchorElement>('a[href^="#footnote-ref-"], a[href^="#endnote-ref-"]')
    )) {
      backlink.remove();
    }
    definitions.set(id, clone.innerHTML.trim() || '<p></p>');
    const parent = element.parentElement;
    element.remove();
    if (parent && !parent.children.length) parent.remove();
  }
  return definitions;
}

function noteKindFromTarget(target: string): WorkDocumentNoteKind | null {
  if (target.startsWith('footnote-')) return 'footnote';
  if (target.startsWith('endnote-')) return 'endnote';
  return null;
}
