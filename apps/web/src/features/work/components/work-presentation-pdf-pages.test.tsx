import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { createWorkArtifact } from '../work-templates';
import type { WorkPresentationPrintLayout } from '../work-types';
import { WorkPdfExportSurface } from './work-pdf-export-surface';

describe('Work presentation PDF layouts', () => {
  afterEach(cleanup);

  it('creates portrait notes pages with the slide and its speaker notes', () => {
    const artifact = createWorkArtifact('strategy-deck');
    if (artifact.content.type !== 'presentation') return;
    artifact.content.slides[0].notes = 'Open with the decision and its evidence.';

    const { container } = render(<WorkPdfExportSurface artifact={artifact} presentationLayout='notes' />);
    const pages = container.querySelectorAll<HTMLElement>('[data-work-pdf-page]');

    expect(pages).toHaveLength(3);
    expect(pages[0]).toHaveAttribute('data-presentation-print-layout', 'notes');
    expect(pages[0]).toHaveAttribute('data-pdf-orientation', 'portrait');
    expect(pages[0]).toHaveTextContent('Open with the decision and its evidence.');
    expect(pages[0].querySelector('.work-slide-canvas')).toBeInTheDocument();
  });

  it.each([
    ['handout-2', 2, 'portrait'],
    ['handout-3', 1, 'portrait'],
    ['handout-6', 1, 'landscape'],
  ] as const)('paginates the %s presentation handout', (layout, pageCount, orientation) => {
    const artifact = createWorkArtifact('strategy-deck');
    const { container } = render(
      <WorkPdfExportSurface artifact={artifact} presentationLayout={layout as WorkPresentationPrintLayout} />
    );
    const pages = container.querySelectorAll<HTMLElement>('[data-work-pdf-page]');

    expect(pages).toHaveLength(pageCount);
    expect(pages[0]).toHaveAttribute('data-presentation-print-layout', layout);
    expect(pages[0]).toHaveAttribute('data-pdf-orientation', orientation);
    expect(container.querySelectorAll('[data-presentation-slide-number]')).toHaveLength(3);
  });

  it('keeps slide review comments out of PDF and handout output', () => {
    const artifact = createWorkArtifact('strategy-deck');
    if (artifact.content.type !== 'presentation') return;
    artifact.content.slides[0].comments = [
      {
        id: 'pdf-review-comment',
        author: 'Alice',
        date: '2026-07-21T00:00:00.000Z',
        text: 'Internal review only',
        x: 50,
        y: 50,
      },
    ];

    const { container, rerender } = render(<WorkPdfExportSurface artifact={artifact} presentationLayout='slides' />);
    expect(container).not.toHaveTextContent('Internal review only');
    expect(container.querySelector('.work-presentation-comment-pin')).not.toBeInTheDocument();

    rerender(<WorkPdfExportSurface artifact={artifact} presentationLayout='handout-3' />);
    expect(container).not.toHaveTextContent('Internal review only');
    expect(container.querySelector('.work-presentation-comment-pin')).not.toBeInTheDocument();
  });
});
