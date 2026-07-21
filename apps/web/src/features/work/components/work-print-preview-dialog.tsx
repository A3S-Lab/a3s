import { AlertTriangle, ChevronLeft, ChevronRight, FileDown, Minus, Plus, Printer } from 'lucide-react';
import { type CSSProperties, type KeyboardEvent, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Button, Dialog } from '../../../design-system/primitives';
import { parseWorkPrintRange } from '../work-print-range';
import type { WorkArtifact, WorkPresentationPrintLayout } from '../work-types';
import { workArtifactKindLabel } from '../work-types';
import { WorkPdfExportSurface } from './work-pdf-export-surface';

type WorkPrintRangeMode = 'all' | 'current' | 'custom';

const MIN_ZOOM = 40;
const MAX_ZOOM = 120;
const ZOOM_STEP = 10;

export function WorkPrintPreviewDialog({
  artifact,
  presentationLayout,
  exportingPdf,
  onPresentationLayoutChange,
  onClose,
  onExportPdf,
  onPrint,
  onReviewCompatibility,
}: {
  artifact: WorkArtifact;
  presentationLayout: WorkPresentationPrintLayout;
  exportingPdf: boolean;
  onPresentationLayoutChange: (layout: WorkPresentationPrintLayout) => void;
  onClose: () => void;
  onExportPdf: (pageIndexes: number[]) => void | Promise<void>;
  onPrint: (pageIndexes: number[]) => void | Promise<void>;
  onReviewCompatibility?: () => void;
}) {
  const [pageLabels, setPageLabels] = useState<string[]>([]);
  const [currentPage, setCurrentPage] = useState(0);
  const [zoom, setZoom] = useState(70);
  const [rangeMode, setRangeMode] = useState<WorkPrintRangeMode>('all');
  const [customRange, setCustomRange] = useState('');
  const [printing, setPrinting] = useState(false);
  const previewRef = useRef<HTMLDivElement>(null);
  const scrollerRef = useRef<HTMLDivElement>(null);
  const pageCount = pageLabels.length;

  useEffect(() => {
    document.body.classList.add('work-print-preview-open');
    return () => document.body.classList.remove('work-print-preview-open');
  }, []);

  useLayoutEffect(() => {
    const pages = printablePages(previewRef.current);
    setPageLabels(pages.map(printablePageLabel));
    setCurrentPage((page) => Math.min(page, Math.max(0, pages.length - 1)));
  }, [artifact.id, artifact.revision, presentationLayout]);

  const customResult = useMemo(() => parseWorkPrintRange(customRange, pageCount), [customRange, pageCount]);
  const selectedPageIndexes = useMemo(() => {
    if (rangeMode === 'current') return pageCount ? [currentPage] : [];
    if (rangeMode === 'custom') return customResult.error ? [] : customResult.pageIndexes;
    return Array.from({ length: pageCount }, (_, index) => index);
  }, [currentPage, customResult, pageCount, rangeMode]);
  const selectedPages = useMemo(() => new Set(selectedPageIndexes), [selectedPageIndexes]);
  const rangeError = rangeMode === 'custom' ? customResult.error : pageCount ? null : '当前文件没有可打印页面。';
  const outputDisabled = Boolean(rangeError) || selectedPageIndexes.length === 0 || exportingPdf || printing;

  useEffect(() => {
    printablePages(previewRef.current).forEach((page, index) => {
      if (selectedPages.has(index)) page.removeAttribute('data-print-excluded');
      else page.setAttribute('data-print-excluded', 'true');
    });
  }, [pageLabels, selectedPages]);

  const goToPage = (nextPage: number) => {
    if (!pageCount) return;
    const bounded = Math.min(Math.max(nextPage, 0), pageCount - 1);
    setCurrentPage(bounded);
    printablePages(previewRef.current)[bounded]?.scrollIntoView?.({
      behavior: 'smooth',
      block: 'start',
      inline: 'center',
    });
  };
  const adjustZoom = (delta: number) => {
    setZoom((value) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, value + delta)));
  };
  const handlePreviewKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (
      event.target instanceof HTMLInputElement ||
      event.target instanceof HTMLSelectElement ||
      event.target instanceof HTMLTextAreaElement
    ) {
      return;
    }
    if (event.key === 'ArrowLeft' || event.key === 'PageUp') {
      event.preventDefault();
      goToPage(currentPage - 1);
    } else if (event.key === 'ArrowRight' || event.key === 'PageDown') {
      event.preventDefault();
      goToPage(currentPage + 1);
    } else if (event.key === 'Home') {
      event.preventDefault();
      goToPage(0);
    } else if (event.key === 'End') {
      event.preventDefault();
      goToPage(pageCount - 1);
    } else if (event.key === '+' || event.key === '=') {
      event.preventDefault();
      adjustZoom(ZOOM_STEP);
    } else if (event.key === '-') {
      event.preventDefault();
      adjustZoom(-ZOOM_STEP);
    }
  };
  const handlePrint = async () => {
    if (outputDisabled) return;
    setPrinting(true);
    try {
      await onPrint(selectedPageIndexes);
    } finally {
      setPrinting(false);
    }
  };
  const previewStyle = {
    '--work-print-preview-zoom': String(zoom / 100),
  } as CSSProperties;

  const dialog = (
    <Dialog
      className='work-print-preview-dialog'
      title='打印预览'
      description={`检查 ${workArtifactKindLabel(artifact.kind)}最终分页、页序和输出范围。`}
      onClose={onClose}
      closeDisabled={exportingPdf || printing}
      footer={
        <>
          <span className='work-print-preview-output-summary'>
            已选择 {selectedPageIndexes.length} / {pageCount} 页
          </span>
          <Button tone='quiet' disabled={exportingPdf || printing} onClick={onClose}>
            关闭
          </Button>
          <Button
            aria-label='打印所选页面'
            disabled={outputDisabled}
            loading={printing}
            onClick={() => void handlePrint()}
          >
            <Printer size={15} />
            打印
          </Button>
          <Button
            tone='primary'
            aria-label='导出所选页面为 PDF'
            disabled={outputDisabled}
            loading={exportingPdf}
            onClick={() => void onExportPdf(selectedPageIndexes)}
          >
            <FileDown size={15} />
            导出 PDF
          </Button>
        </>
      }
    >
      <section className='work-print-preview-layout'>
        <aside className='work-print-preview-settings' aria-label='打印设置'>
          <section className='work-print-preview-file-summary'>
            <strong title={artifact.title}>{artifact.title}</strong>
            <output aria-label='打印页数'>{pageCount} 页</output>
          </section>

          {artifact.content.type === 'presentation' && (
            <label className='work-print-preview-field'>
              <span>打印版式</span>
              <select
                aria-label='演示打印版式'
                value={presentationLayout}
                onChange={(event) => {
                  setCurrentPage(0);
                  onPresentationLayoutChange(event.target.value as WorkPresentationPrintLayout);
                }}
              >
                <option value='slides'>整页幻灯片</option>
                <option value='notes'>备注页</option>
                <option value='handout-2'>讲义 · 每页 2 张</option>
                <option value='handout-3'>讲义 · 每页 3 张</option>
                <option value='handout-6'>讲义 · 每页 6 张</option>
              </select>
            </label>
          )}

          <fieldset className='work-print-preview-range'>
            <legend>页面范围</legend>
            <label>
              <input
                type='radio'
                name='work-print-range'
                checked={rangeMode === 'all'}
                onChange={() => setRangeMode('all')}
              />
              <span>全部页面</span>
            </label>
            <label>
              <input
                type='radio'
                name='work-print-range'
                checked={rangeMode === 'current'}
                onChange={() => setRangeMode('current')}
              />
              <span>当前页面</span>
            </label>
            <label>
              <input
                type='radio'
                name='work-print-range'
                checked={rangeMode === 'custom'}
                onChange={() => {
                  setRangeMode('custom');
                  if (!customRange) setCustomRange(String(currentPage + 1));
                }}
              />
              <span>自定义范围</span>
            </label>
            <input
              type='text'
              aria-label='自定义页码范围'
              value={customRange}
              disabled={rangeMode !== 'custom'}
              placeholder='例如 1-3, 5'
              onChange={(event) => setCustomRange(event.target.value)}
            />
            {rangeError && (
              <p role='alert' className='work-print-preview-range-error'>
                {rangeError}
              </p>
            )}
          </fieldset>

          {artifact.compatibility?.issues.length ? (
            <button type='button' className='work-print-preview-compatibility' onClick={onReviewCompatibility}>
              <AlertTriangle size={15} />
              <span>
                <strong>{artifact.compatibility.issues.length} 条兼容性提示</strong>
                <small>导出前查看可能变化的内容</small>
              </span>
            </button>
          ) : (
            <p className='work-print-preview-hint'>
              {artifact.content.type === 'spreadsheet'
                ? '打印范围、标题行列、分页符、页面设置和页眉页脚已应用。'
                : artifact.content.type === 'document'
                  ? '分节、纸张、边距、分栏、页眉页脚和显式分页符已应用。'
                  : '版式会同时用于浏览器打印与 PDF 导出。'}
            </p>
          )}

          <nav className='work-print-preview-thumbnails' aria-label='打印页面列表'>
            {pageLabels.map((label, index) => (
              <button
                type='button'
                className={`${index === currentPage ? 'active' : ''}${selectedPages.has(index) ? '' : ' excluded'}`}
                aria-label={`预览第 ${index + 1} 页：${label}`}
                aria-current={index === currentPage ? 'page' : undefined}
                onClick={() => goToPage(index)}
                key={`${index}-${label}`}
              >
                <span aria-hidden='true'>
                  <i />
                  <i />
                  <i />
                  <strong>{index + 1}</strong>
                </span>
                <small>{selectedPages.has(index) ? label : '不输出'}</small>
              </button>
            ))}
          </nav>
        </aside>

        <section className='work-print-preview-stage'>
          <div
            className='work-print-preview-toolbar'
            role='toolbar'
            aria-label='打印预览工具栏'
            onKeyDown={handlePreviewKeyDown}
          >
            <button
              type='button'
              aria-label='上一打印页'
              disabled={currentPage === 0 || pageCount === 0}
              onClick={() => goToPage(currentPage - 1)}
            >
              <ChevronLeft size={15} />
            </button>
            <output aria-label='当前打印页'>
              {pageCount ? currentPage + 1 : 0} / {pageCount}
            </output>
            <button
              type='button'
              aria-label='下一打印页'
              disabled={currentPage >= pageCount - 1}
              onClick={() => goToPage(currentPage + 1)}
            >
              <ChevronRight size={15} />
            </button>
            <span className='work-print-preview-toolbar-spacer' />
            <button
              type='button'
              aria-label='缩小打印预览'
              disabled={zoom <= MIN_ZOOM}
              data-autofocus
              onClick={() => adjustZoom(-ZOOM_STEP)}
            >
              <Minus size={15} />
            </button>
            <output aria-label='打印预览缩放比例'>{zoom}%</output>
            <button
              type='button'
              aria-label='放大打印预览'
              disabled={zoom >= MAX_ZOOM}
              onClick={() => adjustZoom(ZOOM_STEP)}
            >
              <Plus size={15} />
            </button>
          </div>
          <section
            ref={scrollerRef}
            className='work-print-preview-scroll'
            aria-label='打印页面预览'
            onScroll={() => {
              const scroller = scrollerRef.current;
              const pages = printablePages(previewRef.current);
              if (!scroller || !pages.length) return;
              const top = scroller.getBoundingClientRect().top + 32;
              let nearest = 0;
              for (let index = 0; index < pages.length; index += 1) {
                if (pages[index].getBoundingClientRect().top <= top) nearest = index;
                else break;
              }
              setCurrentPage(nearest);
            }}
          >
            <div ref={previewRef} className='work-print-preview-pages' style={previewStyle}>
              <WorkPdfExportSurface artifact={artifact} presentationLayout={presentationLayout} mode='preview' />
            </div>
          </section>
        </section>
      </section>
    </Dialog>
  );

  return typeof document === 'undefined' ? null : createPortal(dialog, document.body);
}

function printablePages(root: HTMLElement | null): HTMLElement[] {
  return root ? Array.from(root.querySelectorAll<HTMLElement>('[data-work-pdf-page]')) : [];
}

function printablePageLabel(page: HTMLElement, index: number): string {
  return (
    page.dataset.pdfPageRange ??
    (page.dataset.documentPageNumber ? `文档页 ${page.dataset.documentPageNumber}` : undefined) ??
    (page.dataset.presentationPrintPage ? `演示输出页 ${page.dataset.presentationPrintPage}` : undefined) ??
    `第 ${index + 1} 页`
  );
}
