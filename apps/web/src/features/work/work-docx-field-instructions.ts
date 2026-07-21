import { attribute, descendants } from './work-ooxml-package';

export interface DocxFieldOccurrence {
  instruction: string;
  start: Element;
  end: Element;
}

export function docxFieldInstructions(root: ParentNode): string[] {
  return docxFieldOccurrences(root).map((field) => field.instruction);
}

export function docxFieldOccurrences(root: ParentNode): DocxFieldOccurrence[] {
  const fields: DocxFieldOccurrence[] = descendants(root, 'fldSimple').map((field) => ({
    instruction: attribute(field, 'instr') ?? '',
    start: field,
    end: field,
  }));
  const stack: Array<{ instruction: string; start: Element }> = [];
  for (const element of Array.from(root.querySelectorAll('*'))) {
    if (element.localName === 'fldChar') {
      const fieldType = attribute(element, 'fldCharType');
      if (fieldType === 'begin') {
        stack.push({ instruction: '', start: element });
      } else if (fieldType === 'end' && stack.length) {
        const field = stack.pop();
        if (field) fields.push({ ...field, end: element });
      }
      continue;
    }
    if (element.localName === 'instrText' && stack.length) {
      stack[stack.length - 1].instruction += element.textContent ?? '';
    } else if (element.localName === 'instrText') {
      fields.push({
        instruction: element.textContent ?? '',
        start: element,
        end: element,
      });
    }
  }
  fields.push(...stack.map((field) => ({ ...field, end: field.start })));
  return fields
    .map((field) => ({ ...field, instruction: field.instruction.trim() }))
    .filter((field) => Boolean(field.instruction));
}

export function docxFieldResultText(field: DocxFieldOccurrence): string {
  if (field.start.localName === 'fldSimple') {
    return descendants(field.start, 't')
      .map((element) => element.textContent ?? '')
      .join('');
  }
  const root = closestAncestor(field.start, 'p') ?? field.start.ownerDocument?.documentElement;
  if (!root) return '';
  let inside = false;
  let separated = false;
  const values: string[] = [];
  for (const element of Array.from(root.querySelectorAll('*'))) {
    if (element === field.start) inside = true;
    if (!inside) continue;
    if (element.localName === 'fldChar' && attribute(element, 'fldCharType') === 'separate') {
      separated = true;
    } else if (separated && (element.localName === 't' || element.localName === 'delText')) {
      values.push(element.textContent ?? '');
    }
    if (element === field.end) break;
  }
  return values.join('');
}

function closestAncestor(element: Element, localName: string): Element | null {
  let current: Element | null = element;
  while (current) {
    if (current.localName === localName) return current;
    current = current.parentElement;
  }
  return null;
}
