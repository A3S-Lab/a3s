import { type ExportPlugin, PDFViewer, type PluginRegistry } from '@embedpdf/react-pdf-viewer';
import { AlertCircle, Check, Loader2, Save } from 'lucide-react';
import { type RefObject, useCallback, useEffect, useRef, useState } from 'react';
import { Button, StateView, StatusBadge } from '../../../design-system/primitives';
import { isOfficeShortcutBlocked } from './office-shortcuts';

type PdfSaveState = 'idle' | 'saving' | 'saved' | 'error';

const PDFIUM_WASM_PATH = '/workspace/vendor/embedpdf/pdfium.wasm';

export function PdfViewer({
  fileName = 'document.pdf',
  loadSource,
  onSave,
  saveLabel = '保存',
  sourceKey,
}: {
  fileName?: string;
  loadSource: () => Promise<Blob>;
  onSave?: (pdf: Blob) => Promise<boolean>;
  saveLabel?: string;
  sourceKey?: string;
}) {
  const [sourceUrl, setSourceUrl] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [viewerReady, setViewerReady] = useState(false);
  const [saveState, setSaveState] = useState<PdfSaveState>('idle');
  const [retryCount, setRetryCount] = useState(0);
  const registryRef = useRef<PluginRegistry | null>(null);
  const embedRef = useRef<HTMLDivElement>(null);

  usePdfViewerControls(embedRef, Boolean(sourceUrl && viewerReady));

  useEffect(() => {
    let disposed = false;
    let objectUrl: string | null = null;
    registryRef.current = null;
    setViewerReady(false);
    setSaveState('idle');
    setSourceUrl(null);
    setLoadError(null);

    void loadSource()
      .then((source) => {
        if (disposed) return;
        objectUrl = URL.createObjectURL(
          source.type === 'application/pdf' ? source : new Blob([source], { type: 'application/pdf' })
        );
        setSourceUrl(objectUrl);
      })
      .catch((error: unknown) => {
        if (!disposed) setLoadError(pdfErrorMessage(error));
      });

    return () => {
      disposed = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [loadSource, retryCount, sourceKey]);

  const savePdf = useCallback(async () => {
    const registry = registryRef.current;
    if (!registry || !onSave || saveState === 'saving') return;
    setSaveState('saving');
    try {
      await registry.pluginsReady();
      const exportPlugin = registry.getPlugin<ExportPlugin>('export');
      if (!exportPlugin) throw new Error('EmbedPDF export plugin is unavailable.');
      const buffer = await exportPlugin.provides().saveAsCopy().toPromise();
      const saved = await onSave(new Blob([buffer], { type: 'application/pdf' }));
      setSaveState(saved ? 'saved' : 'error');
    } catch {
      setSaveState('error');
    }
  }, [onSave, saveState]);

  useEffect(() => {
    if (!onSave) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        event.defaultPrevented ||
        event.repeat ||
        event.altKey ||
        event.shiftKey ||
        isOfficeShortcutBlocked(event.target) ||
        !(event.metaKey || event.ctrlKey) ||
        event.key.toLowerCase() !== 's'
      ) {
        return;
      }
      event.preventDefault();
      event.stopImmediatePropagation();
      void savePdf();
    };
    window.addEventListener('keydown', onKeyDown, { capture: true });
    return () => window.removeEventListener('keydown', onKeyDown, { capture: true });
  }, [onSave, savePdf]);

  if (loadError) {
    return (
      <StateView
        className='work-pdf-state'
        tone='danger'
        role='alert'
        icon={<AlertCircle size={24} />}
        title='无法打开 PDF'
        description='请重试。'
        descriptionTitle={loadError}
        actions={<Button onClick={() => setRetryCount((value) => value + 1)}>重试</Button>}
      />
    );
  }

  if (!sourceUrl) {
    return (
      <StateView
        className='work-pdf-state'
        role='status'
        icon={<Loader2 className='spin' size={22} />}
        title='正在加载 PDF…'
      />
    );
  }

  return (
    <section className='work-pdf-viewer' aria-label={`PDF 编辑器：${fileName}`}>
      {onSave && (
        <div className='work-pdf-integration-bar'>
          <output className={saveState} aria-label='PDF 保存状态'>
            {saveState === 'saving' && <StatusBadge tone='info'>正在保存…</StatusBadge>}
            {saveState === 'saved' && (
              <StatusBadge tone='success'>
                <Check size={13} /> 已保存
              </StatusBadge>
            )}
            {saveState === 'error' && <StatusBadge tone='danger'>保存失败，请重试</StatusBadge>}
          </output>
          <Button
            tone='secondary'
            title={`${saveLabel}（Cmd/Ctrl+S）`}
            disabled={!viewerReady || saveState === 'saving'}
            onClick={() => void savePdf()}
          >
            <Save size={14} />
            {saveLabel}
          </Button>
        </div>
      )}
      <div className='work-pdf-embed' ref={embedRef}>
        <PDFViewer
          key={sourceUrl}
          className='work-pdf-native-viewer'
          style={{ width: '100%', height: '100%' }}
          config={{
            src: sourceUrl,
            // EmbedPDF creates a Blob worker, so a root-relative URL has no
            // usable base inside WorkerGlobalScope. Keep this absolute.
            wasmUrl: new URL(PDFIUM_WASM_PATH, window.location.href).href,
            tabBar: 'never',
            theme: {
              preference: 'system',
              light: { accent: { primary: '#2867d8' } },
              dark: { accent: { primary: '#7da7ff' } },
            },
            i18n: { defaultLocale: 'zh-CN' },
            annotations: { annotationAuthor: 'A3S Work 用户', autoCommit: true },
            export: { defaultFileName: fileName },
            fonts: { ui: null, signature: null },
            disabledCategories: onSave ? undefined : ['annotation', 'redaction', 'form', 'history'],
          }}
          onReady={(registry) => {
            registryRef.current = registry;
            setViewerReady(true);
          }}
        />
      </div>
    </section>
  );
}

function usePdfViewerControls(containerRef: RefObject<HTMLDivElement | null>, active: boolean): void {
  useEffect(() => {
    if (!active) return;
    const container = containerRef.current;
    if (!container) return;

    let disposed = false;
    let root: ShadowRoot | null = null;
    let rootObserver: MutationObserver | null = null;
    let rootFrame = 0;
    let focusFrame = 0;
    let rootAttempts = 0;
    let focusAttempts = 0;
    let pendingSearchFocus = false;

    const searchInput = () =>
      [...(root?.querySelectorAll<HTMLInputElement>('input[type="text"]:not([name="zoom"])') ?? [])].find(
        (input) => !input.disabled && input.getAttribute('aria-hidden') !== 'true' && !input.closest('[hidden]')
      ) ?? null;

    const searchButton = () => {
      const item = root?.querySelector<HTMLElement>('[data-epdf-i="search-button"]');
      return item instanceof HTMLButtonElement ? item : (item?.querySelector<HTMLButtonElement>('button') ?? null);
    };

    const focusSearchInput = (): boolean => {
      const input = searchInput();
      if (!input) return false;
      input.setAttribute('aria-label', '在 PDF 中搜索');
      input.focus();
      pendingSearchFocus = false;
      return true;
    };

    const scheduleSearchFocus = () => {
      pendingSearchFocus = true;
      focusAttempts = 0;
      cancelAnimationFrame(focusFrame);
      const focusWhenReady = () => {
        if (disposed || focusSearchInput()) return;
        focusAttempts += 1;
        if (focusAttempts < 12) focusFrame = requestAnimationFrame(focusWhenReady);
      };
      focusFrame = requestAnimationFrame(focusWhenReady);
    };

    const enhanceControls = () => {
      if (!root) return;
      const overflowItem = root.querySelector<HTMLElement>('[data-epdf-i="overflow-tabs-button"]');
      const overflowButton =
        overflowItem instanceof HTMLButtonElement
          ? overflowItem
          : overflowItem?.querySelector<HTMLButtonElement>('button');
      if (overflowButton) {
        overflowButton.setAttribute('aria-label', '更多工具');
        overflowButton.setAttribute('title', '更多工具');
      }
      const zoomInput = root.querySelector<HTMLInputElement>('input[name="zoom"]');
      zoomInput?.setAttribute('aria-label', '缩放比例');
      searchInput()?.setAttribute('aria-label', '在 PDF 中搜索');
      if (pendingSearchFocus) focusSearchInput();
    };

    const onShadowClick = (event: Event) => {
      const openedSearch = event
        .composedPath()
        .some((node) => node instanceof Element && node.getAttribute('data-epdf-i') === 'search-button');
      if (openedSearch) scheduleSearchFocus();
    };

    const connectRoot = () => {
      const nextRoot = container.querySelector<HTMLElement>('embedpdf-container')?.shadowRoot ?? null;
      if (!nextRoot) {
        rootAttempts += 1;
        if (!disposed && rootAttempts < 60) rootFrame = requestAnimationFrame(connectRoot);
        return;
      }
      if (root !== nextRoot) {
        root?.removeEventListener('click', onShadowClick);
        rootObserver?.disconnect();
        root = nextRoot;
        root.addEventListener('click', onShadowClick);
        rootObserver = new MutationObserver(enhanceControls);
        rootObserver.observe(root, { childList: true, subtree: true });
      }
      enhanceControls();
    };

    const onKeyDown = (event: KeyboardEvent) => {
      const currentSearchInput = searchInput();
      if (event.key === 'Escape' && currentSearchInput && root?.activeElement === currentSearchInput) {
        const button = searchButton();
        if (!button) return;
        event.preventDefault();
        event.stopImmediatePropagation();
        pendingSearchFocus = false;
        cancelAnimationFrame(focusFrame);
        button.click();
        focusFrame = requestAnimationFrame(() => {
          focusFrame = requestAnimationFrame(() => button.focus());
        });
        return;
      }
      if (
        event.defaultPrevented ||
        event.repeat ||
        event.altKey ||
        event.shiftKey ||
        isOfficeShortcutBlocked(event.target) ||
        !(event.metaKey || event.ctrlKey) ||
        event.key.toLocaleLowerCase() !== 'f'
      ) {
        return;
      }
      connectRoot();
      if (!root) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      if (focusSearchInput()) return;
      const button = searchButton();
      if (!button) return;
      button.click();
      scheduleSearchFocus();
    };

    const containerObserver = new MutationObserver(connectRoot);
    containerObserver.observe(container, { childList: true, subtree: true });
    window.addEventListener('keydown', onKeyDown, { capture: true });
    connectRoot();

    return () => {
      disposed = true;
      cancelAnimationFrame(rootFrame);
      cancelAnimationFrame(focusFrame);
      containerObserver.disconnect();
      rootObserver?.disconnect();
      root?.removeEventListener('click', onShadowClick);
      window.removeEventListener('keydown', onKeyDown, { capture: true });
    };
  }, [active, containerRef]);
}

function pdfErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  return '无法读取这个 PDF 文件。';
}
