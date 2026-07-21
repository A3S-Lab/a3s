export type WorkDocumentNoteKind = 'footnote' | 'endnote';

export interface WorkDocumentNote {
  id: string;
  kind: WorkDocumentNoteKind;
  number: number;
  html: string;
}

export interface WorkDocumentNoteCollection {
  html: string;
  notes: WorkDocumentNote[];
}

const NOTE_REFERENCE_SELECTOR = 'sup[data-document-note-reference]';
const NOTE_DEFINITION_SELECTOR = 'aside[data-document-note]';

export function normalizeDocumentNotesHtml(source: string): string {
  const document = new DOMParser().parseFromString(source, 'text/html');
  const definitions = new Map<string, HTMLElement>();
  for (const element of Array.from(document.body.querySelectorAll<HTMLElement>(NOTE_DEFINITION_SELECTOR))) {
    const kind = documentNoteKind(element.dataset.noteKind);
    const id = element.dataset.noteId?.trim();
    if (!kind || !id) {
      element.remove();
      continue;
    }
    const key = documentNoteKey(kind, id);
    if (definitions.has(key)) {
      element.remove();
      continue;
    }
    definitions.set(key, element);
  }

  const counters: Record<WorkDocumentNoteKind, number> = { footnote: 0, endnote: 0 };
  const numbers = new Map<string, number>();
  const references = new Map<string, HTMLElement>();
  for (const element of Array.from(document.body.querySelectorAll<HTMLElement>(NOTE_REFERENCE_SELECTOR))) {
    const kind = documentNoteKind(element.dataset.noteKind) ?? 'footnote';
    const id = element.dataset.noteId?.trim() || nextDocumentNoteId(kind, counters[kind] + 1, numbers);
    const key = documentNoteKey(kind, id);
    let number = numbers.get(key);
    if (!number) {
      counters[kind] += 1;
      number = counters[kind];
      numbers.set(key, number);
      references.set(key, element);
    }
    applyDocumentNoteAttributes(element, kind, id, number, true);
    element.textContent = String(number);
  }

  for (const [key, element] of definitions) {
    const number = numbers.get(key);
    if (!number) {
      element.remove();
      continue;
    }
    const { kind, id } = splitDocumentNoteKey(key);
    applyDocumentNoteAttributes(element, kind, id, number, false);
    ensureDocumentNoteBlocks(element);
  }

  for (const [key, number] of numbers) {
    if (definitions.has(key)) continue;
    const { kind, id } = splitDocumentNoteKey(key);
    const note = createDocumentNoteElement(document, { id, kind, number, html: '<p></p>' });
    const reference = references.get(key);
    const target =
      kind === 'footnote'
        ? reference?.closest('section[data-document-section]')
        : document.body.querySelector('section[data-document-section]:last-of-type');
    (target ?? document.body).append(note);
  }
  return document.body.innerHTML;
}

export function collectDocumentNotes(source: string): WorkDocumentNoteCollection {
  const html = normalizeDocumentNotesHtml(source);
  const document = new DOMParser().parseFromString(html, 'text/html');
  const definitions = new Map<string, HTMLElement>();
  for (const element of Array.from(document.body.querySelectorAll<HTMLElement>(NOTE_DEFINITION_SELECTOR))) {
    const kind = documentNoteKind(element.dataset.noteKind);
    const id = element.dataset.noteId?.trim();
    if (kind && id) definitions.set(documentNoteKey(kind, id), element);
  }
  const seen = new Set<string>();
  const notes: WorkDocumentNote[] = [];
  for (const reference of Array.from(document.body.querySelectorAll<HTMLElement>(NOTE_REFERENCE_SELECTOR))) {
    const kind = documentNoteKind(reference.dataset.noteKind);
    const id = reference.dataset.noteId?.trim();
    if (!kind || !id) continue;
    const key = documentNoteKey(kind, id);
    if (seen.has(key)) continue;
    const definition = definitions.get(key);
    if (!definition) continue;
    seen.add(key);
    notes.push({
      id,
      kind,
      number: positiveInteger(reference.dataset.noteNumber, notes.length + 1),
      html: definition.innerHTML || '<p></p>',
    });
  }
  return { html, notes };
}

export function documentNoteReferenceKeys(source: string): string[] {
  const document = new DOMParser().parseFromString(source, 'text/html');
  const keys: string[] = [];
  for (const reference of Array.from(document.body.querySelectorAll<HTMLElement>(NOTE_REFERENCE_SELECTOR))) {
    const kind = documentNoteKind(reference.dataset.noteKind);
    const id = reference.dataset.noteId?.trim();
    if (kind && id) keys.push(documentNoteKey(kind, id));
  }
  return keys;
}

export function removeDocumentNoteDefinitions(source: string): string {
  const document = new DOMParser().parseFromString(source, 'text/html');
  for (const note of Array.from(document.body.querySelectorAll(NOTE_DEFINITION_SELECTOR))) note.remove();
  return document.body.innerHTML;
}

export function createDocumentNoteElement(document: Document, note: WorkDocumentNote): HTMLElement {
  const element = document.createElement('aside');
  applyDocumentNoteAttributes(element, note.kind, note.id, note.number, false);
  element.innerHTML = note.html || '<p></p>';
  ensureDocumentNoteBlocks(element);
  return element;
}

export function documentNoteKey(kind: WorkDocumentNoteKind, id: string): string {
  return `${kind}:${id}`;
}

export function documentNoteKind(value: string | undefined): WorkDocumentNoteKind | null {
  if (value === 'footnote' || value === 'endnote') return value;
  return null;
}

function applyDocumentNoteAttributes(
  element: HTMLElement,
  kind: WorkDocumentNoteKind,
  id: string,
  number: number,
  reference: boolean
) {
  element.setAttribute(reference ? 'data-document-note-reference' : 'data-document-note', 'true');
  element.setAttribute('data-note-kind', kind);
  element.setAttribute('data-note-id', id);
  element.setAttribute('data-note-number', String(number));
}

function splitDocumentNoteKey(key: string): { kind: WorkDocumentNoteKind; id: string } {
  const separator = key.indexOf(':');
  return {
    kind: key.slice(0, separator) as WorkDocumentNoteKind,
    id: key.slice(separator + 1),
  };
}

function nextDocumentNoteId(kind: WorkDocumentNoteKind, seed: number, existing: Map<string, number>): string {
  let suffix = seed;
  while (existing.has(documentNoteKey(kind, `document-${kind}-${suffix}`))) suffix += 1;
  return `document-${kind}-${suffix}`;
}

function ensureDocumentNoteBlocks(element: HTMLElement) {
  if (element.children.length) return;
  const text = element.textContent ?? '';
  element.replaceChildren();
  const paragraph = element.ownerDocument.createElement('p');
  paragraph.textContent = text;
  element.append(paragraph);
}

function positiveInteger(value: string | undefined, fallback: number): number {
  const number = Number(value);
  return Number.isSafeInteger(number) && number > 0 ? number : fallback;
}
