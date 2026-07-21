import { describe, expect, it } from 'vitest';
import type { WorkSlide, WorkSlideElement } from './work-types';
import {
  clonePresentationElementForPaste,
  clonePresentationSlideForPaste,
  copyPresentationElement,
  takePresentationClipboard,
} from './work-presentation-clipboard';

const element: WorkSlideElement = {
  id: 'element-source',
  type: 'table',
  x: 12,
  y: 18,
  width: 40,
  height: 24,
  text: '',
  fontSize: 14,
  color: '#172033',
  fill: '#ffffff',
  bold: false,
  align: 'left',
  placeholder: { key: 'idx:2', type: 'body' },
  table: { headerRows: 1, rows: [['Quarter', 'Revenue']] },
};

describe('Work presentation clipboard', () => {
  it('clones an element with independent content, a fresh ID, and a visible paste offset', () => {
    const pasted = clonePresentationElementForPaste(element, 4);

    expect(pasted).toMatchObject({
      type: 'table',
      x: 16,
      y: 22,
      placeholder: undefined,
      table: { rows: [['Quarter', 'Revenue']] },
    });
    expect(pasted.id).not.toBe(element.id);
    expect(pasted.table).not.toBe(element.table);
  });

  it('regenerates slide, element, and comment identities while preserving presentation content', () => {
    const slide: WorkSlide = {
      id: 'slide-source',
      name: 'Results',
      background: '#ffffff',
      elements: [element],
      notes: 'Explain the quarter-over-quarter change.',
      comments: [
        {
          id: 'comment-source',
          author: 'Alice',
          date: '2026-07-21T00:00:00.000Z',
          text: 'Verify this number.',
          x: 40,
          y: 30,
        },
      ],
    };

    const pasted = clonePresentationSlideForPaste(slide);

    expect(pasted).toMatchObject({
      name: 'Results 副本',
      notes: slide.notes,
      elements: [{ table: { rows: [['Quarter', 'Revenue']] } }],
      comments: [{ text: 'Verify this number.' }],
    });
    expect(pasted.id).not.toBe(slide.id);
    expect(pasted.elements[0].id).not.toBe(element.id);
    expect(pasted.comments?.[0].id).not.toBe(slide.comments?.[0].id);
  });

  it('keeps full-fidelity element data in the in-app clipboard and cascades paste offsets', () => {
    copyPresentationElement(element);

    const first = takePresentationClipboard();
    const second = takePresentationClipboard();

    expect(first).toMatchObject({ payload: { kind: 'element', element }, offset: 2 });
    expect(second).toMatchObject({ payload: { kind: 'element', element }, offset: 4 });
  });
});
