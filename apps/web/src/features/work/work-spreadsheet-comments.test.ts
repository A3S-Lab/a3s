import type { Comments } from 'xlsx';
import { describe, expect, it } from 'vitest';
import { exportXlsxCellComment, importXlsxCellComment } from './work-spreadsheet-comments';

describe('Work spreadsheet comments', () => {
  it('maps a legacy XLSX comment to FortuneSheet geometry and preserves its author', () => {
    const source = [{ a: 'Reviewer', t: 'Confirm the launch date.' }] as Comments;
    source.hidden = false;

    expect(importXlsxCellComment(source)).toEqual({
      left: null,
      top: null,
      width: null,
      height: null,
      value: 'Confirm the launch date.',
      isShow: true,
      author: 'Reviewer',
    });
  });

  it('flattens multiple or threaded source entries into one editable legacy comment', () => {
    const source = [
      { a: 'Reviewer', t: 'First note', T: true },
      { a: 'Owner', t: 'Reply', T: true },
    ] as Comments;

    expect(importXlsxCellComment(source)).toMatchObject({
      value: 'Reviewer: First note\n\nOwner: Reply',
      author: 'Reviewer, Owner',
      isShow: false,
    });
  });

  it('exports edited FortuneSheet comment HTML as plain XLSX text', () => {
    const comments = exportXlsxCellComment({
      left: null,
      top: null,
      width: null,
      height: null,
      value: '<div>Confirm&nbsp;owner<br>before launch</div>',
      isShow: false,
      author: 'A3S Reviewer',
    });

    expect(Array.from(comments ?? [])).toEqual([{ a: 'A3S Reviewer', t: 'Confirm owner\nbefore launch' }]);
    expect(comments?.hidden).toBe(true);
  });
});
