import { describe, expect, it } from 'vitest';
import { normalizeDocumentCaptionsHtml } from './work-document-captions';

describe('Work document captions', () => {
  it('renumbers figure and table captions independently and updates references', () => {
    const html = normalizeDocumentCaptionsHtml(
      [
        '<figcaption data-document-caption="true" data-caption-kind="figure" data-caption-id="figure-one">Architecture</figcaption>',
        '<p>See <span data-document-cross-reference="true" data-reference-target-id="figure-two">stale</span>.</p>',
        '<figcaption data-document-caption="true" data-caption-kind="table" data-caption-id="table-one">Metrics</figcaption>',
        '<figcaption data-document-caption="true" data-caption-kind="figure" data-caption-id="figure-two">Runtime</figcaption>',
      ].join('')
    );
    const document = new DOMParser().parseFromString(html, 'text/html');
    const captions = document.querySelectorAll<HTMLElement>('[data-document-caption]');
    const reference = document.querySelector<HTMLElement>('[data-document-cross-reference]');

    expect(captions[0].dataset.captionNumber).toBe('1');
    expect(captions[0].dataset.captionLabel).toBe('图');
    expect(captions[1].dataset.captionNumber).toBe('1');
    expect(captions[1].dataset.captionLabel).toBe('表');
    expect(captions[2].dataset.captionNumber).toBe('2');
    expect(reference?.dataset.captionNumber).toBe('2');
    expect(reference?.textContent).toBe('图 2');
  });

  it('marks references whose caption target no longer exists', () => {
    const html = normalizeDocumentCaptionsHtml(
      '<p><span data-document-cross-reference="true" data-reference-target-id="missing">图 9</span></p>'
    );
    const document = new DOMParser().parseFromString(html, 'text/html');
    const reference = document.querySelector<HTMLElement>('[data-document-cross-reference]');

    expect(reference?.dataset.referenceOrphaned).toBe('true');
    expect(reference?.textContent).toBe('引用缺失');
  });
});
