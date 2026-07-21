import { ChevronDown, ChevronUp, Download, Minus, Plus } from 'lucide-react';
import type { PDFDocumentProxy, PDFPageProxy, RenderTask } from 'pdfjs-dist';
import { useEffect, useRef, useState } from 'react';

let pdfRuntimePromise: Promise<typeof import('pdfjs-dist')> | null = null;

export function PdfViewer({ loadSource, onDownload }: { loadSource: () => Promise<Blob>; onDownload?: () => void }) {
  const [document, setDocument] = useState<PDFDocumentProxy | null>(null);
  const [zoom, setZoom] = useState(100);
  const [page, setPage] = useState(1);
  const [error, setError] = useState<string | null>(null);
  const scrollerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let current = true;
    let loaded: PDFDocumentProxy | null = null;
    void loadSource()
      .then(async (blob) => {
        const runtime = await loadPdfRuntime();
        loaded = await runtime.getDocument({ data: await blob.arrayBuffer() }).promise;
        if (current) setDocument(loaded);
      })
      .catch((reason: unknown) => {
        if (current) setError(pdfErrorMessage(reason));
      });
    return () => {
      current = false;
      void loaded?.destroy();
    };
  }, [loadSource]);

  const moveToPage = (next: number) => {
    if (!document) return;
    const target = Math.min(Math.max(next, 1), document.numPages);
    setPage(target);
    scrollerRef.current
      ?.querySelector<HTMLElement>(`[data-pdf-page="${target}"]`)
      ?.scrollIntoView({ block: 'start', behavior: 'smooth' });
  };

  if (error) {
    return (
      <section className='work-pdf-state error' role='alert'>
        <strong>无法预览 PDF</strong>
        <span>{error}</span>
        {onDownload && (
          <button type='button' onClick={onDownload}>
            <Download size={14} />
            下载原始文件
          </button>
        )}
      </section>
    );
  }
  if (!document) {
    return (
      <output className='work-pdf-state'>
        <span className='work-state-spinner' />
        正在读取 PDF…
      </output>
    );
  }

  return (
    <section className='work-pdf-viewer'>
      <div className='work-office-toolbar work-pdf-toolbar' role='toolbar' aria-label='PDF 预览工具栏'>
        <button type='button' aria-label='上一页' disabled={page === 1} onClick={() => moveToPage(page - 1)}>
          <ChevronUp size={15} />
        </button>
        <label>
          <span>页码</span>
          <input
            type='number'
            min={1}
            max={document.numPages}
            value={page}
            aria-label='PDF 页码'
            onChange={(event) => moveToPage(Number(event.target.value) || 1)}
          />
          <i>/ {document.numPages}</i>
        </label>
        <button
          type='button'
          aria-label='下一页'
          disabled={page === document.numPages}
          onClick={() => moveToPage(page + 1)}
        >
          <ChevronDown size={15} />
        </button>
        <span className='work-toolbar-divider' />
        <button type='button' aria-label='缩小' disabled={zoom <= 50} onClick={() => setZoom((value) => value - 10)}>
          <Minus size={15} />
        </button>
        <output aria-label='PDF 缩放比例'>{zoom}%</output>
        <button type='button' aria-label='放大' disabled={zoom >= 200} onClick={() => setZoom((value) => value + 10)}>
          <Plus size={15} />
        </button>
        {onDownload && (
          <>
            <span className='work-toolbar-spacer' />
            <button type='button' onClick={onDownload}>
              <Download size={14} />
              下载
            </button>
          </>
        )}
      </div>
      <div
        ref={scrollerRef}
        className='work-pdf-pages'
        onScroll={(event) => {
          const pages = Array.from(event.currentTarget.querySelectorAll<HTMLElement>('[data-pdf-page]'));
          const top = event.currentTarget.getBoundingClientRect().top + 24;
          const currentPage = pages.findLast((item) => item.getBoundingClientRect().top <= top)?.dataset.pdfPage ?? '1';
          setPage(Number(currentPage));
        }}
      >
        {Array.from({ length: document.numPages }, (_, index) => (
          <PdfPage document={document} pageNumber={index + 1} zoom={zoom} key={index + 1} />
        ))}
      </div>
    </section>
  );
}

function PdfPage({ document, pageNumber, zoom }: { document: PDFDocumentProxy; pageNumber: number; zoom: number }) {
  const hostRef = useRef<HTMLElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const renderTaskRef = useRef<RenderTask | null>(null);
  const [visible, setVisible] = useState(pageNumber === 1);
  const [text, setText] = useState('');

  useEffect(() => {
    const host = hostRef.current;
    if (!host || visible || typeof IntersectionObserver === 'undefined') return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) setVisible(true);
      },
      { rootMargin: '800px 0px' }
    );
    observer.observe(host);
    return () => observer.disconnect();
  }, [visible]);

  useEffect(() => {
    if (!visible) return;
    let current = true;
    let page: PDFPageProxy | null = null;
    const render = async () => {
      page = await document.getPage(pageNumber);
      const canvas = canvasRef.current;
      const host = hostRef.current;
      if (!current || !canvas || !host) return;
      const base = page.getViewport({ scale: 1 });
      const availableWidth = Math.max(320, Math.min(host.clientWidth - 32, 1120));
      const scale = (availableWidth / base.width) * (zoom / 100);
      const viewport = page.getViewport({ scale });
      const pixelRatio = Math.max(1, Math.min(window.devicePixelRatio || 1, 2));
      canvas.width = Math.floor(viewport.width * pixelRatio);
      canvas.height = Math.floor(viewport.height * pixelRatio);
      canvas.style.width = `${viewport.width}px`;
      canvas.style.height = `${viewport.height}px`;
      renderTaskRef.current?.cancel();
      renderTaskRef.current = page.render({
        canvas,
        viewport,
        transform: pixelRatio === 1 ? undefined : [pixelRatio, 0, 0, pixelRatio, 0, 0],
      });
      await renderTaskRef.current.promise;
      const content = await page.getTextContent();
      if (current) {
        setText(
          content.items
            .filter((item): item is typeof item & { str: string } => 'str' in item)
            .map((item) => item.str)
            .join(' ')
        );
      }
    };
    void render().catch((reason: unknown) => {
      if (current && !(reason instanceof Error && reason.name === 'RenderingCancelledException')) {
        setText('此页无法渲染');
      }
    });
    return () => {
      current = false;
      renderTaskRef.current?.cancel();
      page?.cleanup();
    };
  }, [document, pageNumber, visible, zoom]);

  return (
    <article ref={hostRef} className='work-pdf-page' data-pdf-page={pageNumber} aria-label={`PDF 第 ${pageNumber} 页`}>
      <canvas ref={canvasRef} role='img' aria-label={`PDF 第 ${pageNumber} 页预览`} />
      {text && <p className='sr-only'>{text}</p>}
      <footer>{pageNumber}</footer>
    </article>
  );
}

function loadPdfRuntime(): Promise<typeof import('pdfjs-dist')> {
  if (pdfRuntimePromise) return pdfRuntimePromise;
  pdfRuntimePromise = import('pdfjs-dist').then((runtime) => {
    runtime.GlobalWorkerOptions.workerSrc = '/vendor/pdf.worker.min.mjs';
    return runtime;
  });
  return pdfRuntimePromise;
}

function pdfErrorMessage(reason: unknown): string {
  if (!(reason instanceof Error)) return 'PDF 文件无法读取';
  if (reason.name === 'PasswordException') return '此 PDF 受密码保护，当前预览器还不能解锁。';
  return reason.message || 'PDF 文件无法读取';
}
