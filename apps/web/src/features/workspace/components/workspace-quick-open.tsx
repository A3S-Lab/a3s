import { LoaderCircle, RotateCcw, Search } from 'lucide-react';
import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { formatApiError } from '../../../state/app-state';
import type { WorkspaceFileCatalog, WorkspaceFileCatalogItem } from '../../../types/api';
import { normalizePath, workspaceRelativePath } from '../workspace-state';
import { WorkspaceFileIcon } from './workspace-file-icon';

const QUICK_OPEN_RESULT_LIMIT = 120;
const QUERY_DELAY_MS = 90;

interface WorkspaceQuickOpenProps {
  rootPath: string;
  generation: number;
  openFiles: ReadonlyArray<{ path: string; isBinary: boolean }>;
  activePath: string | null;
  findFiles: (query: string, maxResults: number) => Promise<WorkspaceFileCatalog>;
  openFile: (item: WorkspaceFileCatalogItem) => Promise<boolean>;
  onClose: () => void;
}

export function WorkspaceQuickOpen({
  rootPath,
  generation,
  openFiles,
  activePath,
  findFiles,
  openFile,
  onClose,
}: WorkspaceQuickOpenProps) {
  const [query, setQuery] = useState('');
  const [catalog, setCatalog] = useState<WorkspaceFileCatalog | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [openingPath, setOpeningPath] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const restoreFocusRef = useRef<HTMLElement | null>(null);
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const selectedIndexRef = useRef(0);
  const requestIdRef = useRef(0);
  const mountedRef = useRef(true);
  const resultId = useId();
  const normalizedQuery = query.trim();

  useEffect(() => {
    mountedRef.current = true;
    restoreFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    inputRef.current?.focus();
    return () => {
      mountedRef.current = false;
      const previous = restoreFocusRef.current;
      if (previous?.isConnected) previous.focus();
    };
  }, []);

  useEffect(() => {
    const requestId = ++requestIdRef.current;
    setCatalog(null);
    setError(null);
    setLoading(Boolean(rootPath));
    selectedIndexRef.current = 0;
    setSelectedIndex(0);
    if (!rootPath) {
      setError('当前没有可用的工作区。');
      return;
    }
    const timer = window.setTimeout(
      () => {
        void findFiles(normalizedQuery, QUICK_OPEN_RESULT_LIMIT)
          .then((result) => {
            if (requestId !== requestIdRef.current) return;
            setCatalog(result);
          })
          .catch((reason: unknown) => {
            if (requestId !== requestIdRef.current) return;
            setError(formatApiError(reason));
          })
          .finally(() => {
            if (requestId === requestIdRef.current) setLoading(false);
          });
      },
      normalizedQuery ? QUERY_DELAY_MS : 0
    );
    return () => window.clearTimeout(timer);
  }, [findFiles, generation, normalizedQuery, reloadToken, rootPath]);

  const options = useMemo(
    () => quickOpenOptions(catalog?.items ?? [], normalizedQuery, openFiles, activePath, rootPath),
    [activePath, catalog?.items, normalizedQuery, openFiles, rootPath]
  );
  const openPaths = useMemo(
    () => new Set(openFiles.map((file) => comparablePath(file.path, rootPath))),
    [openFiles, rootPath]
  );

  useLayoutEffect(() => {
    const index = Math.min(selectedIndexRef.current, Math.max(0, options.length - 1));
    selectedIndexRef.current = index;
    setSelectedIndex(index);
  }, [options.length]);
  useEffect(() => {
    optionRefs.current[selectedIndex]?.scrollIntoView?.({ block: 'nearest' });
  }, [selectedIndex]);

  const moveSelection = (offset: number) => {
    if (!options.length) return;
    const index = (selectedIndexRef.current + offset + options.length) % options.length;
    selectedIndexRef.current = index;
    setSelectedIndex(index);
  };
  const selectIndex = (index: number) => {
    selectedIndexRef.current = index;
    setSelectedIndex(index);
  };
  const choose = async (item: WorkspaceFileCatalogItem) => {
    if (openingPath) return;
    setOpeningPath(item.path);
    setError(null);
    try {
      const opened = await openFile(item);
      if (opened) onClose();
      else if (mountedRef.current) setError(`无法打开 ${item.name}。`);
    } catch (reason) {
      if (mountedRef.current) setError(formatApiError(reason));
    } finally {
      if (mountedRef.current) setOpeningPath(null);
    }
  };

  return (
    <dialog
      ref={dialogRef}
      open
      className='palette-overlay workspace-quick-open-overlay'
      aria-modal='true'
      aria-label='快速打开文件'
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault();
          event.stopPropagation();
          onClose();
          return;
        }
        if (event.key !== 'Tab') return;
        const focusable = [
          ...(dialogRef.current?.querySelectorAll<HTMLElement>('input, button:not(:disabled), [tabindex="0"]') ?? []),
        ];
        if (!focusable.length) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }}
    >
      <section className='command-palette workspace-quick-open'>
        <label>
          <Search size={16} />
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder='按文件名或路径搜索'
            aria-label='按文件名或路径搜索'
            role='combobox'
            aria-autocomplete='list'
            aria-controls={resultId}
            aria-expanded='true'
            aria-activedescendant={options[selectedIndex] ? `${resultId}-${selectedIndex}` : undefined}
            onKeyDown={(event) => {
              if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'p') {
                event.preventDefault();
                moveSelection(1);
              } else if (event.key === 'ArrowDown') {
                event.preventDefault();
                moveSelection(1);
              } else if (event.key === 'ArrowUp') {
                event.preventDefault();
                moveSelection(-1);
              } else if (event.key === 'Home') {
                event.preventDefault();
                selectIndex(0);
              } else if (event.key === 'End') {
                event.preventDefault();
                selectIndex(Math.max(0, options.length - 1));
              } else if (event.key === 'Enter') {
                event.preventDefault();
                const selected = options[selectedIndexRef.current];
                if (selected) void choose(selected);
              }
            }}
          />
          {loading && <LoaderCircle className='spin workspace-quick-open-loader' size={15} aria-label='正在查找文件' />}
        </label>
        <div
          className='palette-results workspace-quick-open-results'
          id={resultId}
          role='listbox'
          aria-label='工作区文件'
          aria-busy={loading}
        >
          <span>FILES · {basename(rootPath)}</span>
          {options.map((item, index) => {
            const selected = index === selectedIndex;
            const open = openPaths.has(comparablePath(item.path, rootPath));
            return (
              <button
                ref={(element) => {
                  optionRefs.current[index] = element;
                }}
                type='button'
                role='option'
                aria-selected={selected}
                className={selected ? 'active' : ''}
                id={`${resultId}-${index}`}
                key={item.path}
                disabled={openingPath !== null}
                onClick={() => void choose(item)}
                onMouseEnter={() => selectIndex(index)}
              >
                <WorkspaceFileIcon path={item.path} size={17} />
                <span>
                  <strong>{item.name}</strong>
                  <small>{directoryLabel(item.relativePath)}</small>
                </span>
                <span className='workspace-quick-open-badges'>
                  {open && <em>已打开</em>}
                  {item.isBinary && <em>二进制</em>}
                  {openingPath === item.path && <LoaderCircle className='spin' size={13} />}
                </span>
              </button>
            );
          })}
          {loading && !options.length && <output className='workspace-quick-open-state'>正在索引工作区文件…</output>}
          {!loading && error && (
            <div className='workspace-quick-open-state error' role='alert'>
              <span>{error}</span>
              <button type='button' onClick={() => setReloadToken((value) => value + 1)}>
                <RotateCcw size={13} />
                重试
              </button>
            </div>
          )}
          {!loading && !error && !options.length && (
            <p>{normalizedQuery ? `没有匹配“${normalizedQuery}”的文件` : '工作区中没有可打开的文件'}</p>
          )}
        </div>
        <footer className='workspace-quick-open-footer'>
          <span>
            <kbd>↑↓</kbd> 选择　<kbd>Enter</kbd> 打开　<kbd>Esc</kbd> 关闭
          </span>
          {catalog?.truncated && (
            <span>
              显示前 {catalog.items.length} / {catalog.total} 个结果，请继续输入以缩小范围
            </span>
          )}
        </footer>
      </section>
    </dialog>
  );
}

function quickOpenOptions(
  catalogItems: WorkspaceFileCatalogItem[],
  query: string,
  openFiles: ReadonlyArray<{ path: string; isBinary: boolean }>,
  activePath: string | null,
  workspaceRoot: string
): WorkspaceFileCatalogItem[] {
  if (query) return [...catalogItems];
  const openItems = openFiles
    .map((file) => ({
      path: file.path,
      relativePath: workspaceRelativePath(file.path, workspaceRoot),
      name: basename(file.path),
      isBinary: file.isBinary,
    }))
    .sort((left, right) => Number(right.path === activePath) - Number(left.path === activePath));
  const seen = new Set(openItems.map((item) => comparablePath(item.path, workspaceRoot)));
  return [...openItems, ...catalogItems.filter((item) => !seen.has(comparablePath(item.path, workspaceRoot)))];
}

function comparablePath(path: string, workspaceRoot: string): string {
  const normalized = normalizePath(path);
  return /^[A-Za-z]:[\\/]/.test(workspaceRoot) ? normalized.toLowerCase() : normalized;
}

function directoryLabel(relativePath: string): string {
  const normalized = normalizePath(relativePath);
  const index = normalized.lastIndexOf('/');
  return index < 0 ? '工作区根目录' : normalized.slice(0, index);
}

function basename(path: string): string {
  return normalizePath(path).split('/').filter(Boolean).pop() || path;
}
