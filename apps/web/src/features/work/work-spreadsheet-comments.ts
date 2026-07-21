import type { Cell } from '@fortune-sheet/core';
import type { Comments } from 'xlsx';

export interface WorkSpreadsheetCellComment extends NonNullable<Cell['ps']> {
  author?: string;
}

export function importXlsxCellComment(source: Comments | undefined): WorkSpreadsheetCellComment | undefined {
  const comments = (source ?? []).filter((comment) => comment.t.trim());
  if (!comments.length) return undefined;
  const authors = Array.from(
    new Set(comments.map((comment) => comment.a?.trim()).filter((author): author is string => Boolean(author)))
  );
  const value =
    comments.length === 1
      ? comments[0].t
      : comments.map((comment) => `${comment.a?.trim() ? `${comment.a.trim()}: ` : ''}${comment.t}`).join('\n\n');
  return {
    left: null,
    top: null,
    width: null,
    height: null,
    value,
    isShow: source?.hidden === false,
    author: authors.join(', ') || undefined,
  };
}

export function exportXlsxCellComment(source: WorkSpreadsheetCellComment | undefined): Comments | undefined {
  if (!source) return undefined;
  const text = commentPlainText(source.value);
  if (!text) return undefined;
  const author =
    typeof (source as WorkSpreadsheetCellComment).author === 'string'
      ? (source as WorkSpreadsheetCellComment).author!.trim()
      : '';
  const comments = [{ a: author.slice(0, 54) || 'A3S Work', t: text }] as Comments;
  comments.hidden = !source.isShow;
  return comments;
}

function commentPlainText(value: string): string {
  if (!/[<&]/.test(value)) return value.trim();
  const document = new DOMParser().parseFromString(value, 'text/html');
  for (const lineBreak of Array.from(document.body.querySelectorAll('br'))) lineBreak.replaceWith('\n');
  for (const block of Array.from(document.body.querySelectorAll('div, p'))) block.append('\n');
  return (document.body.textContent ?? '')
    .replace(/\u00a0/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}
