import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { documentSectionDomAttributes } from '../work-document-section';
import type { WorkArtifact, WorkDocumentSectionLayout } from '../work-types';
import { WorkDocumentPreview } from './work-document-pages';
import { WorkPdfExportSurface } from './work-pdf-export-surface';

describe('Work document section pages', () => {
  afterEach(cleanup);

  it('keeps continuous sections on one page and honors odd-page section breaks', () => {
    const first = layout({
      columns: { count: 2, spacing: 9, separator: true },
      breakAfter: 'continuous',
      headerText: 'Section one',
      showPageNumbers: true,
      pageNumberStart: 5,
    });
    const second = layout({
      columns: { count: 3, spacing: 7, separator: false },
      breakAfter: 'oddPage',
      headerText: 'Section two',
      showPageNumbers: true,
    });
    const third = layout({
      pageSize: 'letter',
      orientation: 'landscape',
      breakAfter: 'nextPage',
      headerText: 'Section three',
      showPageNumbers: true,
    });
    const artifact: WorkArtifact = {
      id: 'document-sections',
      kind: 'document',
      title: 'Section pages',
      favorite: false,
      createdAt: 1,
      updatedAt: 1,
      lastOpenedAt: 1,
      revision: 1,
      content: {
        type: 'document',
        pageSize: first.pageSize,
        orientation: first.orientation,
        margins: first.margins,
        columns: first.columns,
        headerText: first.headerText,
        showPageNumbers: true,
        pageNumberStart: 5,
        html: [
          sectionHtml(first, 'first', '<p>Alpha</p>'),
          sectionHtml(second, 'second', '<p>Beta</p>'),
          sectionHtml(third, 'third', '<p>Gamma</p>'),
        ].join(''),
      },
    };

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    const pages = container.querySelectorAll<HTMLElement>('[data-work-pdf-page]');

    expect(pages).toHaveLength(3);
    expect(pages[0].querySelectorAll('[data-document-print-section]')).toHaveLength(2);
    expect(pages[0].querySelector('[data-document-print-section="first"]')).toHaveAttribute(
      'data-document-column-count',
      '2'
    );
    expect(pages[0].querySelector('[data-document-print-section="second"]')).toHaveAttribute(
      'data-document-column-count',
      '3'
    );
    expect(pages[1]).toHaveAttribute('data-document-blank-page', 'true');
    expect(pages[2]).toHaveAttribute('data-document-page-number', '7');
    expect(pages[2]).toHaveAttribute('data-pdf-page-size', 'letter');
    expect(pages[2]).toHaveAttribute('data-pdf-orientation', 'landscape');
    expect(pages[2]).toHaveTextContent('Gamma');
  });

  it('uses the same section pagination for read-only preview', () => {
    const first = layout({ breakAfter: 'nextPage' });
    const second = layout({ pageSize: 'letter', orientation: 'landscape' });
    const content: WorkArtifact['content'] = {
      type: 'document',
      pageSize: 'a4',
      html: `${sectionHtml(first, 'first', '<p>First preview page</p>')}${sectionHtml(
        second,
        'second',
        '<p>Second preview page</p>'
      )}`,
    };
    if (content.type !== 'document') return;

    const { container } = render(<WorkDocumentPreview content={content} />);
    const pages = container.querySelectorAll<HTMLElement>('.work-document-preview-page');
    expect(pages).toHaveLength(2);
    expect(pages[0]).toHaveTextContent('First preview page');
    expect(pages[1]).toHaveClass('letter', 'landscape');
    expect(pages[1]).toHaveTextContent('Second preview page');
  });

  it('keeps tracked insertions and deletions visible in preview and PDF output', () => {
    const content: WorkArtifact['content'] = {
      type: 'document',
      pageSize: 'a4',
      trackChanges: true,
      html: [
        '<p>Review ',
        '<ins data-document-change="true" data-change-kind="insertion" data-change-id="preview-add"',
        ' data-change-author="Alice" data-change-date="2026-07-20T00:00:00.000Z">new copy</ins>',
        ' and ',
        '<del data-document-change="true" data-change-kind="deletion" data-change-id="preview-delete"',
        ' data-change-author="Bob" data-change-date="2026-07-19T00:00:00.000Z">old copy</del>',
        '.</p>',
      ].join(''),
    };
    if (content.type !== 'document') return;
    const artifact: WorkArtifact = {
      id: 'document-revisions',
      kind: 'document',
      title: 'Document revisions',
      favorite: false,
      createdAt: 1,
      updatedAt: 1,
      lastOpenedAt: 1,
      revision: 1,
      content,
    };

    const { container, rerender } = render(<WorkDocumentPreview content={content} />);
    let insertion = container.querySelector('ins[data-document-change]');
    let deletion = container.querySelector('del[data-document-change]');
    expect(insertion).toHaveTextContent('new copy');
    expect(deletion).toHaveTextContent('old copy');

    rerender(<WorkPdfExportSurface artifact={artifact} />);
    insertion = container.querySelector('ins[data-document-change]');
    deletion = container.querySelector('del[data-document-change]');
    expect(insertion).toHaveTextContent('new copy');
    expect(deletion).toHaveTextContent('old copy');
  });

  it('keeps comment anchors highlighted in preview and visually plain in PDF output', () => {
    const content: WorkArtifact['content'] = {
      type: 'document',
      pageSize: 'a4',
      html: [
        '<p>Review ',
        '<span data-document-comment="true" data-comment-id="preview-comment">this claim</span>',
        ' before publishing.</p>',
      ].join(''),
      comments: [
        {
          id: 'preview-comment',
          author: 'Alice',
          date: '2026-07-20T00:00:00.000Z',
          text: 'Please verify the source.',
          resolved: false,
        },
      ],
    };
    if (content.type !== 'document') return;
    const artifact: WorkArtifact = {
      id: 'document-comments-preview',
      kind: 'document',
      title: 'Document comments',
      favorite: false,
      createdAt: 1,
      updatedAt: 1,
      lastOpenedAt: 1,
      revision: 1,
      content,
    };

    const { container, rerender } = render(<WorkDocumentPreview content={content} />);
    let page = container.querySelector<HTMLElement>('.work-document-preview-page');
    let anchor = page?.querySelector<HTMLElement>('[data-document-comment]');
    expect(page).toHaveAttribute('data-document-comment-appearance', 'highlighted');
    expect(anchor).toHaveAttribute('data-comment-id', 'preview-comment');
    expect(anchor).toHaveTextContent('this claim');

    rerender(<WorkPdfExportSurface artifact={artifact} />);
    page = container.querySelector<HTMLElement>('[data-work-pdf-page]');
    anchor = page?.querySelector<HTMLElement>('[data-document-comment]');
    expect(page).toHaveAttribute('data-document-comment-appearance', 'plain');
    expect(anchor).toHaveAttribute('data-comment-id', 'preview-comment');
    expect(page).toHaveTextContent('Review this claim before publishing.');
  });

  it('uses proportional custom columns in preview and PDF output', () => {
    const customLayout = layout({
      columns: {
        count: 2,
        spacing: 12,
        separator: true,
        custom: [
          { widthPercent: 65, spacing: 8 },
          { widthPercent: 35, spacing: 0 },
        ],
      },
    });
    const content: WorkArtifact['content'] = {
      type: 'document',
      pageSize: 'a4',
      columns: customLayout.columns,
      html: sectionHtml(
        customLayout,
        'custom-columns',
        '<p>Alpha block</p><p>Beta block</p><p>Gamma block</p><p>Delta block</p>'
      ),
    };
    if (content.type !== 'document') return;
    const artifact: WorkArtifact = {
      id: 'custom-document-columns',
      kind: 'document',
      title: 'Custom columns',
      favorite: false,
      createdAt: 1,
      updatedAt: 1,
      lastOpenedAt: 1,
      revision: 1,
      content,
    };

    const { container, rerender } = render(<WorkPdfExportSurface artifact={artifact} />);
    let customColumns = container.querySelector<HTMLElement>('[data-document-column-layout="custom"]');
    expect(customColumns).toHaveStyle({
      gridTemplateColumns: 'minmax(0, 65fr) 8mm minmax(0, 35fr)',
    });
    expect(customColumns?.querySelectorAll('[data-document-column-index]')).toHaveLength(2);

    rerender(<WorkDocumentPreview content={content} />);
    customColumns = container.querySelector<HTMLElement>('[data-document-column-layout="custom"]');
    expect(customColumns?.querySelectorAll('[data-document-column-index]')).toHaveLength(2);
    expect(customColumns).toHaveTextContent('Alpha block');
    expect(customColumns).toHaveTextContent('Delta block');
  });

  it('places footnotes on their reference page and endnotes at the document end', () => {
    const content: WorkArtifact['content'] = {
      type: 'document',
      pageSize: 'a4',
      html: [
        '<p>First page',
        '<sup data-document-note-reference="true" data-note-kind="footnote" data-note-id="foot-one" data-note-number="1">1</sup>',
        '</p>',
        '<div data-page-break="true"></div>',
        '<p>Last page',
        '<sup data-document-note-reference="true" data-note-kind="endnote" data-note-id="end-one" data-note-number="1">1</sup>',
        '</p>',
        '<aside data-document-note="true" data-note-kind="footnote" data-note-id="foot-one" data-note-number="1">',
        '<p>Page-specific note <a href="javascript:alert(1)" onclick="alert(1)">source</a></p>',
        '<script data-note-script>window.noteAttack = true</script>',
        '</aside>',
        '<aside data-document-note="true" data-note-kind="endnote" data-note-id="end-one" data-note-number="1">',
        '<p>Document-end note</p>',
        '</aside>',
      ].join(''),
    };
    if (content.type !== 'document') return;

    const artifact: WorkArtifact = {
      id: 'document-notes',
      kind: 'document',
      title: 'Document notes',
      favorite: false,
      createdAt: 1,
      updatedAt: 1,
      lastOpenedAt: 1,
      revision: 1,
      content,
    };
    const { container, rerender } = render(<WorkPdfExportSurface artifact={artifact} />);
    let pages = container.querySelectorAll<HTMLElement>('[data-work-pdf-page]');
    expect(pages).toHaveLength(2);
    expect(pages[0].querySelector('[data-document-page-note-kind="footnote"]')).toHaveTextContent('Page-specific note');
    expect(pages[0].querySelector('[data-note-script]')).not.toBeInTheDocument();
    expect(pages[0].querySelector('[data-document-page-note-kind="footnote"] a')).not.toHaveAttribute('href');
    expect(pages[0].querySelector('[data-document-page-note-kind="footnote"] a')).not.toHaveAttribute('onclick');
    expect(pages[0]).not.toHaveTextContent('Document-end note');
    expect(pages[1].querySelector('[data-document-page-note-kind="endnote"]')).toHaveTextContent('Document-end note');

    rerender(<WorkDocumentPreview content={content} />);
    pages = container.querySelectorAll<HTMLElement>('.work-document-preview-page');
    expect(pages[0].querySelector('[data-document-page-note-kind="footnote"]')).toHaveTextContent('Page-specific note');
    expect(pages[1].querySelector('[data-document-page-note-kind="endnote"]')).toHaveTextContent('Document-end note');
  });

  it('normalizes captions and cross-references in preview and PDF output', () => {
    const content: WorkArtifact['content'] = {
      type: 'document',
      pageSize: 'a4',
      html: [
        '<p>Architecture overview</p>',
        '<figcaption data-document-caption="true" data-caption-kind="figure" data-caption-id="architecture">',
        'System architecture',
        '</figcaption>',
        '<p>See <span data-document-cross-reference="true" data-reference-target-id="architecture">stale</span>.</p>',
      ].join(''),
    };
    if (content.type !== 'document') return;
    const artifact: WorkArtifact = {
      id: 'document-captions',
      kind: 'document',
      title: 'Captions',
      favorite: false,
      createdAt: 1,
      updatedAt: 1,
      lastOpenedAt: 1,
      revision: 1,
      content,
    };

    const { container, rerender } = render(<WorkPdfExportSurface artifact={artifact} />);
    let caption = container.querySelector<HTMLElement>('[data-document-caption]');
    let reference = container.querySelector<HTMLElement>('[data-document-cross-reference]');
    expect(caption).toHaveAttribute('data-caption-number', '1');
    expect(caption).toHaveAttribute('data-caption-label', '图');
    expect(caption).toHaveTextContent('System architecture');
    expect(reference).toHaveTextContent('图 1');

    rerender(<WorkDocumentPreview content={content} />);
    caption = container.querySelector<HTMLElement>('[data-document-caption]');
    reference = container.querySelector<HTMLElement>('[data-document-cross-reference]');
    expect(caption).toHaveAttribute('data-caption-number', '1');
    expect(reference).toHaveTextContent('图 1');
  });

  it('resolves first, even, and default rich page chrome in preview and PDF', () => {
    const content: WorkArtifact['content'] = {
      type: 'document',
      pageSize: 'a4',
      pageChrome: {
        differentFirstPage: true,
        differentOddEvenPages: true,
        default: {
          headerHtml: '<p><strong>Odd header</strong></p>',
          footerHtml: '<p>Odd footer</p>',
          showPageNumber: true,
        },
        first: {
          headerHtml: '<p><em>First header</em></p>',
          footerHtml: '<p>First footer</p>',
          showPageNumber: false,
        },
        even: {
          headerHtml: '<p><u>Even header</u></p>',
          footerHtml: '<p>Even footer</p>',
          showPageNumber: true,
        },
      },
      html: [
        '<p>Page one</p>',
        '<div data-page-break="true"></div>',
        '<p>Page two</p>',
        '<div data-page-break="true"></div>',
        '<p>Page three</p>',
      ].join(''),
    };
    if (content.type !== 'document') return;
    const artifact: WorkArtifact = {
      id: 'rich-page-chrome',
      kind: 'document',
      title: 'Rich page chrome',
      favorite: false,
      createdAt: 1,
      updatedAt: 1,
      lastOpenedAt: 1,
      revision: 1,
      content,
    };

    const { container, rerender } = render(<WorkPdfExportSurface artifact={artifact} />);
    let pages = container.querySelectorAll<HTMLElement>('[data-work-pdf-page]');
    expect(pages).toHaveLength(3);
    expect(pages[0]).toHaveAttribute('data-document-page-chrome', 'first');
    expect(pages[0].querySelector('header')).toHaveTextContent('First header');
    expect(pages[0].querySelector('footer')).toHaveTextContent('First footer');
    expect(pages[1]).toHaveAttribute('data-document-page-chrome', 'even');
    expect(pages[1].querySelector('header')).toHaveTextContent('Even header');
    expect(pages[1].querySelector('footer')).toHaveTextContent('Even footer');
    expect(pages[1].querySelector('footer')).toHaveTextContent('2');
    expect(pages[2]).toHaveAttribute('data-document-page-chrome', 'default');
    expect(pages[2].querySelector('header')).toHaveTextContent('Odd header');

    rerender(<WorkDocumentPreview content={content} />);
    pages = container.querySelectorAll<HTMLElement>('.work-document-preview-page');
    expect(pages[0]).toHaveAttribute('data-document-page-chrome', 'first');
    expect(pages[1]).toHaveAttribute('data-document-page-chrome', 'even');
    expect(pages[2]).toHaveAttribute('data-document-page-chrome', 'default');
  });
});

function layout(patch: Partial<WorkDocumentSectionLayout>): WorkDocumentSectionLayout {
  return {
    pageSize: 'a4',
    orientation: 'portrait',
    margins: { top: 20, right: 20, bottom: 20, left: 20 },
    columns: { count: 1, spacing: 12, separator: false },
    breakAfter: 'nextPage',
    ...patch,
  };
}

function sectionHtml(layout: WorkDocumentSectionLayout, id: string, html: string): string {
  const section = document.createElement('section');
  for (const [name, value] of Object.entries(documentSectionDomAttributes(layout, id))) {
    section.setAttribute(name, value);
  }
  section.innerHTML = html;
  return section.outerHTML;
}
