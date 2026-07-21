import { millimetersToPixels } from '../work-document-layout';
import {
  documentColumnGridTemplate,
  documentUnequalColumnGroups,
  normalizeDocumentColumns,
} from '../work-document-columns';
import { resolveDocumentPageChrome } from '../work-document-page-chrome';
import {
  documentPageDescriptors,
  type WorkDocumentPageDescriptor,
  type WorkDocumentPageSegment,
} from '../work-document-pages';
import type { WorkDocumentNote, WorkDocumentNoteKind } from '../work-document-notes';
import type { WorkDocumentContent } from '../work-types';

export function WorkDocumentPdfPages({ content, title }: { content: WorkDocumentContent; title: string }) {
  return documentPageDescriptors(content).map((page) => (
    <DocumentPage key={page.key} page={page} title={title} mode='pdf' />
  ));
}

export function WorkDocumentPreview({ content }: { content: WorkDocumentContent }) {
  return (
    <section className='work-document-preview-pages' aria-label='文字预览'>
      {documentPageDescriptors(content).map((page) => (
        <DocumentPage key={page.key} page={page} mode='preview' />
      ))}
    </section>
  );
}

function DocumentPage({
  page,
  title,
  mode,
}: {
  page: WorkDocumentPageDescriptor;
  title?: string;
  mode: 'pdf' | 'preview';
}) {
  const layout = page.layout;
  const pageChrome = resolveDocumentPageChrome(layout, page.sectionPage, page.physicalPage);
  const pageClass =
    mode === 'pdf'
      ? `work-pdf-export-page document ${layout.pageSize} ${layout.orientation}`
      : `work-document-preview-page ${layout.pageSize} ${layout.orientation}`;
  return (
    <section
      className={pageClass}
      data-work-pdf-page={mode === 'pdf' ? '' : undefined}
      data-pdf-orientation={mode === 'pdf' ? layout.orientation : undefined}
      data-pdf-page-size={mode === 'pdf' ? layout.pageSize : undefined}
      data-document-physical-page={page.physicalPage}
      data-document-page-number={page.pageNumber}
      data-document-blank-page={String(page.blank)}
      data-document-page-chrome={pageChrome.variant}
      data-document-comment-appearance={mode === 'pdf' ? 'plain' : 'highlighted'}
      aria-label={mode === 'preview' ? `文字预览第 ${page.physicalPage} 页` : `文字打印预览第 ${page.physicalPage} 页`}
      style={{
        padding: `${millimetersToPixels(layout.margins.top)}px ${millimetersToPixels(
          layout.margins.right
        )}px ${millimetersToPixels(layout.margins.bottom)}px ${millimetersToPixels(layout.margins.left)}px`,
      }}
    >
      <header>
        {pageChrome.headerHtml ? (
          <div className='work-document-page-chrome-html' dangerouslySetInnerHTML={{ __html: pageChrome.headerHtml }} />
        ) : (
          title || ''
        )}
      </header>
      <div className='work-document-print-body'>
        {page.blank ? (
          <span className='work-document-blank-page-label'>此页按分节设置留空</span>
        ) : (
          page.segments.map((segment, index) => (
            <DocumentPageSegmentContent key={`${segment.sectionId}-${index}`} segment={segment} />
          ))
        )}
      </div>
      {page.footnotes.length > 0 && <DocumentPageNotes kind='footnote' notes={page.footnotes} />}
      {page.endnotes.length > 0 && <DocumentPageNotes kind='endnote' notes={page.endnotes} />}
      {(pageChrome.footerHtml || pageChrome.showPageNumber) && (
        <footer>
          {pageChrome.footerHtml ? (
            <div
              className='work-document-page-chrome-html'
              dangerouslySetInnerHTML={{ __html: pageChrome.footerHtml }}
            />
          ) : (
            <span />
          )}
          {pageChrome.showPageNumber && <span>{page.pageNumber}</span>}
        </footer>
      )}
    </section>
  );
}

function DocumentPageSegmentContent({ segment }: { segment: WorkDocumentPageSegment }) {
  const columns = normalizeDocumentColumns(segment.columns);
  if (!columns.custom) {
    return (
      <article
        data-document-print-section={segment.sectionId}
        data-document-column-count={columns.count}
        data-document-column-layout='equal'
        style={{
          columnCount: columns.count,
          columnGap: `${columns.spacing}mm`,
          columnRule: columns.separator ? '1px solid #cbd0d9' : undefined,
        }}
        dangerouslySetInnerHTML={{ __html: segment.html }}
      />
    );
  }
  const groups = documentUnequalColumnGroups(segment.html, columns);
  return (
    <article
      className='work-document-custom-column-layout'
      data-document-print-section={segment.sectionId}
      data-document-column-count={columns.count}
      data-document-column-layout='custom'
      style={{ gridTemplateColumns: documentColumnGridTemplate(columns) }}
    >
      {groups.map((html, index) => (
        <div
          key={`${segment.sectionId}-column-${index + 1}`}
          data-document-column-index={index + 1}
          style={{
            gridColumn: index * 2 + 1,
            borderRight: columns.separator && index < groups.length - 1 ? '1px solid #cbd0d9' : undefined,
          }}
          dangerouslySetInnerHTML={{ __html: html }}
        />
      ))}
    </article>
  );
}

function DocumentPageNotes({ kind, notes }: { kind: WorkDocumentNoteKind; notes: WorkDocumentNote[] }) {
  return (
    <section
      className={`work-document-page-notes ${kind}`}
      data-document-page-note-kind={kind}
      aria-label={kind === 'footnote' ? '脚注' : '尾注'}
    >
      {kind === 'endnote' && <h3>尾注</h3>}
      <ol>
        {notes.map((note) => (
          <li key={`${note.kind}-${note.id}`} value={note.number} data-document-note-id={note.id}>
            <div dangerouslySetInnerHTML={{ __html: note.html }} />
          </li>
        ))}
      </ol>
    </section>
  );
}
