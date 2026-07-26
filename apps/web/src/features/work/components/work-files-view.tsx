import {
  ArrowDown,
  ArrowUp,
  BookPlus,
  Check,
  ClipboardCopy,
  ClipboardPaste,
  Copy,
  Eye,
  FileInput,
  FileText,
  FolderSearch,
  FolderOpen,
  FolderPlus,
  Grid2X2,
  List,
  MessageSquareText,
  Pencil,
  Presentation,
  RefreshCw,
  Scissors,
  Sheet,
  Sparkles,
  Star,
  Tags,
  TextSearch,
  Trash2,
  Upload,
} from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { Button, StateView } from '../../../design-system/primitives';
import { codeApi } from '../../../lib/api';
import { showToast } from '../../../state/app-state';
import type { WorkspaceEntry } from '../../../types/api';
import { WorkspaceContextMenu, type WorkspaceContextMenuItem } from '../../workspace/components/workspace-context-menu';
import { hasDraggedWorkspaceFiles } from '../../workspace/workspace-drop-import';
import type { WorkFilesActions } from '../use-work-files-controller';
import type { WorkAgentRequest } from '../work-agent-request';
import {
  canMoveLocalPaths,
  formatWorkFileDate,
  formatWorkFileSize,
  isWorkOpenableEntry,
  localPathBasename,
  localPathParent,
  readWorkLocalFileDragData,
  relativeLocalPath,
  sameLocalPath,
  workFileKindLabel,
  writeWorkLocalFileDragData,
  type WorkFilesSortKey,
} from '../work-local-files';
import {
  useWorkFileInlineOperation,
  type WorkFileCreateArtifactRequest,
  type WorkFileCreateArtifactResult,
} from './use-work-file-inline-operation';
import { WorkFileDeleteDialog } from './work-file-delete-dialog';
import { WorkFileIcon } from './work-file-icon';
import { WorkFileInlineEditor } from './work-file-inline-editor';
import { useWorkFileMarquee } from './work-file-marquee';
import { WorkFileSelectionControl, WorkFilesSelectAllControl } from './work-file-selection-controls';
import { WorkFilesSelectionToolbar } from './work-files-selection-toolbar';
import { WorkKnowledgeBasePanel } from './work-knowledge-base-panel';

interface ContextMenuState {
  entry: WorkspaceEntry | null;
  x: number;
  y: number;
}

export function WorkFilesView({
  actions,
  openingPath,
  createFolderRequest,
  onOpenFile,
  onQuickLook,
  onAgentRequest,
  onCreateArtifact,
  createArtifactRequest,
  onCreateArtifactFile,
  onConsumeCreateArtifactRequest,
}: {
  actions: WorkFilesActions;
  openingPath: string | null;
  createFolderRequest: number;
  onOpenFile: (entry: WorkspaceEntry) => void | Promise<void>;
  onQuickLook: (entry: WorkspaceEntry) => void;
  onAgentRequest: (request: WorkAgentRequest) => void | Promise<void>;
  onCreateArtifact?: (templateId: string) => void;
  createArtifactRequest?: WorkFileCreateArtifactRequest | null;
  onCreateArtifactFile?: (
    request: WorkFileCreateArtifactRequest,
    fileName: string
  ) => Promise<WorkFileCreateArtifactResult>;
  onConsumeCreateArtifactRequest?: () => void;
}) {
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [pendingDeleteEntries, setPendingDeleteEntries] = useState<WorkspaceEntry[] | null>(null);
  const [draggedPaths, setDraggedPaths] = useState<string[]>([]);
  const [dropTargetPath, setDropTargetPath] = useState<string | null>(null);
  const [externalDropTargetPath, setExternalDropTargetPath] = useState<string | null>(null);
  const [knowledgeBuilderPaths, setKnowledgeBuilderPaths] = useState<string[] | null>(null);
  const filesContentRef = useRef<HTMLDivElement>(null);
  const workspaceSearching = actions.searchScope === 'workspace' && Boolean(actions.query.trim());
  const marquee = useWorkFileMarquee({
    containerRef: filesContentRef,
    visiblePaths: actions.visibleEntries.map((entry) => entry.path),
    selectedPaths: actions.selectedPaths,
    onSelectionChange: actions.replaceSelection,
  });
  const inlineOperation = useWorkFileInlineOperation(actions, {
    createArtifactRequest,
    onCreateArtifact: onCreateArtifactFile,
    onConsumeCreateArtifactRequest,
  });
  useEffect(() => {
    if (createFolderRequest > 0) inlineOperation.startCreateFolder();
  }, [createFolderRequest, inlineOperation.startCreateFolder]);
  const selectedCount = actions.selectedPaths.size;
  const openEntry = (entry: WorkspaceEntry) => {
    if (entry.isDirectory) {
      actions.navigateTo(entry.path);
      return;
    }
    if (!isWorkOpenableEntry(entry)) {
      showToast('这个文件暂不能直接编辑，可以交给 AI 助手处理。', 'info');
      return;
    }
    void onOpenFile(entry);
  };
  const selectedPathsFor = (entry: WorkspaceEntry): string[] =>
    actions.selectedPaths.has(entry.path) ? actions.selectedEntries.map((item) => item.path) : [entry.path];
  const selectedEntriesFor = (entry: WorkspaceEntry): WorkspaceEntry[] =>
    actions.selectedPaths.has(entry.path) ? actions.selectedEntries : [entry];
  const contextItems = contextMenuItems({
    entry: contextMenu?.entry ?? null,
    currentPath: actions.currentPath,
    selectedPaths: contextMenu?.entry ? selectedPathsFor(contextMenu.entry) : [],
    selectedEntries: contextMenu?.entry ? selectedEntriesFor(contextMenu.entry) : [],
    onOpen: openEntry,
    onQuickLook,
    onCreateFolder: inlineOperation.startCreateFolder,
    onRename: inlineOperation.startRename,
    onDuplicate: inlineOperation.startDuplicate,
    onDelete: (entries) => setPendingDeleteEntries([...entries]),
    clipboard: actions.clipboard,
    onCopy: actions.copyEntries,
    onCut: actions.cutEntries,
    onPaste: actions.pasteEntries,
    onCopyPath: copyPathsToClipboard,
    onReveal: revealPath,
    onCreateKnowledgeBase: (paths) => setKnowledgeBuilderPaths([...paths]),
    visibleCount: actions.visibleEntries.length,
    onSelectAll: actions.selectAll,
    layout: actions.layout,
    onSetLayout: actions.setLayout,
    sort: actions.sort,
    onSetSort: actions.setSort,
    onRefresh: actions.refresh,
    favoritePaths: actions.favoritePaths,
    onToggleFavorite: actions.toggleFavoritePath,
    onAgentRequest,
    onCreateArtifact,
  });

  return (
    <>
      <div
        ref={filesContentRef}
        className={`work-files-content ${actions.layout} ${workspaceSearching ? 'workspace-search' : ''} ${externalDropTargetPath === actions.currentPath ? 'external-drop-target' : ''}`}
        role='listbox'
        aria-label='本地文件'
        aria-multiselectable='true'
        aria-busy={actions.dropImporting || actions.searchLoading}
        tabIndex={0}
        onClick={marquee.onClick}
        onPointerDown={marquee.onPointerDown}
        onPointerMove={marquee.onPointerMove}
        onPointerUp={marquee.onPointerUp}
        onPointerCancel={marquee.onPointerCancel}
        onContextMenu={(event) => {
          if (!isBackgroundContextTarget(event.target)) return;
          event.preventDefault();
          actions.clearSelection();
          event.currentTarget.focus({ preventScroll: true });
          setContextMenu({ entry: null, x: event.clientX, y: event.clientY });
        }}
        onDragOver={(event) => {
          if (!hasDraggedWorkspaceFiles(event.dataTransfer)) return;
          event.preventDefault();
          event.dataTransfer.dropEffect = actions.dropImporting ? 'none' : 'copy';
          setExternalDropTargetPath(actions.currentPath);
        }}
        onDragLeave={(event) => {
          const related = event.relatedTarget;
          if (!(related instanceof Node) || !event.currentTarget.contains(related)) setExternalDropTargetPath(null);
        }}
        onDrop={(event) => {
          if (!hasDraggedWorkspaceFiles(event.dataTransfer)) return;
          event.preventDefault();
          event.stopPropagation();
          setExternalDropTargetPath(null);
          if (actions.dropImporting) return;
          void Promise.resolve(actions.importDroppedItems(event.dataTransfer, actions.currentPath)).catch(
            () => undefined
          );
        }}
        onKeyDown={(event) => {
          const commandKey = event.metaKey || event.ctrlKey;
          const key = event.key.toLocaleLowerCase();
          if (
            (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) &&
            event.target === event.currentTarget
          ) {
            event.preventDefault();
            const bounds = event.currentTarget.getBoundingClientRect();
            setContextMenu({
              entry: null,
              x: bounds.left + 18,
              y: bounds.top + 18,
            });
          } else if (commandKey && key === 'c' && actions.selectedEntries.length > 0) {
            event.preventDefault();
            actions.copyEntries(actions.selectedEntries);
          } else if (commandKey && key === 'x' && actions.selectedEntries.length > 0) {
            event.preventDefault();
            actions.cutEntries(actions.selectedEntries);
          } else if (commandKey && key === 'v' && actions.clipboard) {
            event.preventDefault();
            void actions.pasteEntries(actions.currentPath).catch(() => undefined);
          } else if (commandKey && event.key === 'ArrowUp') {
            event.preventDefault();
            actions.goUp();
          } else if (commandKey && event.key === 'ArrowDown' && actions.selectedEntries.length === 1) {
            event.preventDefault();
            openEntry(actions.selectedEntries[0]);
          } else if (commandKey && key === 'a') {
            event.preventDefault();
            actions.selectAll();
          } else if (event.key === 'Enter' && actions.selectedEntries.length === 1) {
            event.preventDefault();
            openEntry(actions.selectedEntries[0]);
          } else if (event.key === ' ' && actions.selectedEntries.length === 1) {
            event.preventDefault();
            onQuickLook(actions.selectedEntries[0]);
          } else if (event.key === 'F2' && actions.selectedEntries.length === 1) {
            event.preventDefault();
            inlineOperation.startRename(actions.selectedEntries[0]);
          } else if (
            actions.selectedEntries.length > 0 &&
            (event.key === 'Delete' || (commandKey && event.key === 'Backspace'))
          ) {
            event.preventDefault();
            setPendingDeleteEntries([...actions.selectedEntries]);
          } else if (event.key === 'Escape') {
            actions.clearSelection();
          } else {
            const items = Array.from(event.currentTarget.querySelectorAll<HTMLElement>('[data-work-file-index]'));
            const nextIndex = finderKeyboardTargetIndex(
              event.key,
              actions.layout,
              actions.visibleEntries.findIndex((entry) => sameLocalPath(entry.path, actions.selectionFocusPath ?? '')),
              items
            );
            if (nextIndex !== null) {
              event.preventDefault();
              const entry = actions.visibleEntries[nextIndex];
              actions.selectEntry(entry, { range: event.shiftKey });
              items[nextIndex]?.focus();
              items[nextIndex]?.scrollIntoView?.({
                block: 'nearest',
                inline: 'nearest',
              });
            }
          }
        }}
      >
        {actions.layout === 'list' && actions.visibleEntries.length > 0 && (
          <table className='work-files-list-header' aria-label='文件列表列标题'>
            <thead>
              <tr>
                <th
                  className='work-files-list-name-heading'
                  scope='col'
                  aria-sort={actions.sort.key === 'name' ? actions.sort.direction : undefined}
                >
                  <WorkFilesSelectAllControl
                    selectedCount={selectedCount}
                    totalCount={actions.visibleEntries.length}
                    onSelectAll={actions.selectAll}
                    onClear={actions.clearSelection}
                  />
                  <span aria-hidden='true' />
                  <WorkFilesSortHeading label='名称' sortKey='name' actions={actions} />
                </th>
                <th
                  scope='col'
                  aria-sort={
                    !workspaceSearching && actions.sort.key === 'modified' ? actions.sort.direction : undefined
                  }
                >
                  {workspaceSearching ? (
                    '位置'
                  ) : (
                    <WorkFilesSortHeading label='修改日期' sortKey='modified' actions={actions} />
                  )}
                </th>
                <th scope='col' aria-sort={actions.sort.key === 'size' ? actions.sort.direction : undefined}>
                  <WorkFilesSortHeading label='大小' sortKey='size' actions={actions} />
                </th>
                <th scope='col' aria-sort={actions.sort.key === 'kind' ? actions.sort.direction : undefined}>
                  <WorkFilesSortHeading label='种类' sortKey='kind' actions={actions} />
                </th>
              </tr>
            </thead>
          </table>
        )}
        <div className={`work-files-items ${actions.layout}`}>
          {inlineOperation.operation && inlineOperation.operation.kind !== 'rename' && (
            <WorkFileInlineEditor
              key={`${inlineOperation.operation.kind}:${inlineOperation.operation.initialValue}`}
              operation={inlineOperation.operation}
              layout={actions.layout}
              workspaceSearching={workspaceSearching}
              rootPath={actions.rootPath}
              onValueChange={inlineOperation.setValue}
              onSave={inlineOperation.save}
              onCancel={inlineOperation.cancel}
            />
          )}
          {actions.visibleEntries.map((entry, index) => {
            if (
              inlineOperation.operation?.kind === 'rename' &&
              sameLocalPath(inlineOperation.operation.entry.path, entry.path)
            ) {
              return (
                <WorkFileInlineEditor
                  key={`rename:${entry.path}`}
                  operation={inlineOperation.operation}
                  layout={actions.layout}
                  index={index}
                  workspaceSearching={workspaceSearching}
                  rootPath={actions.rootPath}
                  onValueChange={inlineOperation.setValue}
                  onSave={inlineOperation.save}
                  onCancel={inlineOperation.cancel}
                />
              );
            }
            const entryDragPaths = actions.selectedPaths.has(entry.path)
              ? actions.selectedEntries.map((item) => item.path)
              : [entry.path];
            const moving = actions.operationPaths.has(entry.path);
            return (
              <button
                type='button'
                role='option'
                data-work-file-index={index}
                aria-selected={actions.selectedPaths.has(entry.path)}
                aria-label={`${entry.name}，${workFileKindLabel(entry)}`}
                className={`work-file-item ${actions.selectedPaths.has(entry.path) ? 'selected' : ''} ${actions.clipboard?.mode === 'cut' && actions.clipboard.entries.some((item) => sameLocalPath(item.path, entry.path)) ? 'cut' : ''} ${draggedPaths.some((path) => sameLocalPath(path, entry.path)) ? 'dragging' : ''} ${dropTargetPath === entry.path ? 'drop-target' : ''} ${externalDropTargetPath === entry.path ? 'external-drop-target' : ''}`}
                disabled={openingPath === entry.path || moving}
                draggable={openingPath !== entry.path && !moving}
                key={entry.path}
                onClick={(event) => {
                  const selectionControl = isSelectionControlTarget(event.target);
                  const commandKey = event.metaKey || event.ctrlKey;
                  actions.selectEntry(entry, {
                    toggle: selectionControl || commandKey,
                    range: !selectionControl && event.shiftKey,
                    additive: !selectionControl && commandKey && event.shiftKey,
                  });
                }}
                onDoubleClick={(event) => {
                  if (!isSelectionControlTarget(event.target)) openEntry(entry);
                }}
                onDragStart={(event) => {
                  if (!actions.selectedPaths.has(entry.path)) actions.selectEntry(entry);
                  writeWorkLocalFileDragData(event.dataTransfer, entryDragPaths);
                  setDraggedPaths(entryDragPaths);
                }}
                onDragEnd={() => {
                  setDraggedPaths([]);
                  setDropTargetPath(null);
                  setExternalDropTargetPath(null);
                }}
                onDragOver={(event) => {
                  if (!entry.isDirectory) return;
                  if (hasDraggedWorkspaceFiles(event.dataTransfer)) {
                    event.preventDefault();
                    event.stopPropagation();
                    event.dataTransfer.dropEffect = actions.dropImporting ? 'none' : 'copy';
                    setDropTargetPath(null);
                    setExternalDropTargetPath(entry.path);
                    return;
                  }
                  const paths = draggedPaths.length ? draggedPaths : readWorkLocalFileDragData(event.dataTransfer);
                  if (!canMoveLocalPaths(paths, entry.path)) return;
                  event.preventDefault();
                  event.dataTransfer.dropEffect = 'move';
                  setExternalDropTargetPath(null);
                  setDropTargetPath(entry.path);
                }}
                onDragLeave={(event) => {
                  const related = event.relatedTarget;
                  if (!(related instanceof Node) || !event.currentTarget.contains(related)) {
                    setDropTargetPath(null);
                    setExternalDropTargetPath(null);
                  }
                }}
                onDrop={(event) => {
                  if (!entry.isDirectory) return;
                  if (hasDraggedWorkspaceFiles(event.dataTransfer)) {
                    event.preventDefault();
                    event.stopPropagation();
                    setDraggedPaths([]);
                    setDropTargetPath(null);
                    setExternalDropTargetPath(null);
                    if (actions.dropImporting) return;
                    void Promise.resolve(actions.importDroppedItems(event.dataTransfer, entry.path)).catch(
                      () => undefined
                    );
                    return;
                  }
                  const paths = readWorkLocalFileDragData(event.dataTransfer);
                  if (!canMoveLocalPaths(paths, entry.path)) return;
                  event.preventDefault();
                  event.stopPropagation();
                  setDraggedPaths([]);
                  setDropTargetPath(null);
                  void Promise.resolve(actions.moveEntries(paths, entry.path)).catch(() => undefined);
                }}
                onContextMenu={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  if (!actions.selectedPaths.has(entry.path)) actions.selectEntry(entry);
                  event.currentTarget.focus();
                  setContextMenu({ entry, x: event.clientX, y: event.clientY });
                }}
                onKeyDown={(event) => {
                  if (event.key === 'F2') {
                    event.preventDefault();
                    inlineOperation.startRename(entry);
                  } else if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) {
                    event.preventDefault();
                    const bounds = event.currentTarget.getBoundingClientRect();
                    setContextMenu({
                      entry,
                      x: bounds.left + Math.min(bounds.width, 180),
                      y: bounds.bottom,
                    });
                  }
                }}
              >
                <WorkFileSelectionControl selected={actions.selectedPaths.has(entry.path)} />
                <span className='work-file-visual'>
                  <WorkFileIcon
                    path={entry.path}
                    directory={entry.isDirectory}
                    size={actions.layout === 'grid' ? 42 : 18}
                  />
                  {openingPath === entry.path && <span className='work-file-opening-spinner' aria-hidden='true' />}
                </span>
                <strong title={entry.name}>{entry.name}</strong>
                <span
                  className='work-file-modified'
                  title={workspaceSearching ? localPathParent(entry.path) : undefined}
                >
                  {workspaceSearching
                    ? relativeLocalPath(localPathParent(entry.path), actions.rootPath) ||
                      localPathBasename(actions.rootPath)
                    : formatWorkFileDate(entry.mtimeMs)}
                </span>
                <span className='work-file-size'>{formatWorkFileSize(entry.size, entry.isDirectory)}</span>
                <span className='work-file-kind'>{workFileKindLabel(entry)}</span>
              </button>
            );
          })}
        </div>
        {!actions.visibleEntries.length &&
          !inlineOperation.operation &&
          !actions.loading &&
          !actions.searchLoading &&
          !actions.error &&
          !actions.searchError && (
            <StateView
              className='work-files-empty-state'
              size='compact'
              icon={<FolderOpen size={24} />}
              title={actions.query ? '没有匹配的文件' : '这个文件夹是空的'}
              description={actions.query ? '尝试缩短搜索词。' : '你可以直接在这里创建第一个文件夹。'}
              actions={
                !actions.query && (
                  <Button tone='primary' onClick={inlineOperation.startCreateFolder}>
                    <FolderPlus size={14} />
                    新建文件夹
                  </Button>
                )
              }
            />
          )}
        {externalDropTargetPath === actions.currentPath && (
          <output className='work-files-drop-hint'>
            <span>
              <Upload size={20} />
            </span>
            <strong>{actions.dropImporting ? '正在复制拖入项目…' : '松开放入当前文件夹'}</strong>
            <small>文件夹会连同其中的内容一起复制</small>
          </output>
        )}
        {marquee.rectangle && (
          <span
            className='work-files-marquee'
            aria-hidden='true'
            style={{
              left: marquee.rectangle.left,
              top: marquee.rectangle.top,
              width: marquee.rectangle.width,
              height: marquee.rectangle.height,
            }}
          />
        )}
      </div>
      {knowledgeBuilderPaths && (
        <WorkKnowledgeBasePanel
          workspaceRoot={actions.rootPath}
          paths={knowledgeBuilderPaths}
          onClose={() => setKnowledgeBuilderPaths(null)}
        />
      )}
      {!knowledgeBuilderPaths && !marquee.deferSelectionToolbar && (
        <WorkFilesSelectionToolbar
          selectedEntries={actions.selectedEntries}
          totalCount={actions.visibleEntries.length}
          onSelectAll={actions.selectAll}
          onQuickLook={onQuickLook}
          onRename={inlineOperation.startRename}
          onCopy={actions.copyEntries}
          onCut={actions.cutEntries}
          onCreateKnowledgeBase={(entries) => setKnowledgeBuilderPaths(entries.map((entry) => entry.path))}
          onAskAssistant={(entries) =>
            void onAgentRequest({
              workspaceRoot: '',
              paths: entries.map((entry) => entry.path),
              instruction: '请查看已选文件或文件夹，并围绕它们回答我的问题：\n\n问题：',
            })
          }
          onDelete={(entries) => setPendingDeleteEntries([...entries])}
          onClear={actions.clearSelection}
        />
      )}
      <footer className='work-files-status'>
        <span>
          {selectedCount > 0
            ? `已选择 ${selectedCount} 项`
            : workspaceSearching
              ? `在“${localPathBasename(actions.rootPath)}”中找到 ${actions.visibleEntries.length} 项${actions.searchTruncated ? ' · 部分结果' : ''}`
              : `${actions.visibleEntries.length} 项${actions.query ? ` · 搜索“${actions.query}”` : ''}`}
        </span>
        {actions.selectedEntries.length === 1 && (
          <span>
            {workFileKindLabel(actions.selectedEntries[0])} ·{' '}
            {formatWorkFileSize(actions.selectedEntries[0].size, actions.selectedEntries[0].isDirectory)}
          </span>
        )}
      </footer>
      {contextMenu && (
        <WorkspaceContextMenu
          label={contextMenu.entry ? `${contextMenu.entry.name} 操作` : '当前文件夹操作'}
          x={contextMenu.x}
          y={contextMenu.y}
          items={contextItems}
          onClose={() => setContextMenu(null)}
        />
      )}
      {pendingDeleteEntries && (
        <WorkFileDeleteDialog
          entries={pendingDeleteEntries}
          actions={actions}
          onClose={() => setPendingDeleteEntries(null)}
        />
      )}
    </>
  );
}

function WorkFilesSortHeading({
  label,
  sortKey,
  actions,
}: {
  label: string;
  sortKey: WorkFilesSortKey;
  actions: WorkFilesActions;
}) {
  const active = actions.sort.key === sortKey;
  const currentDirection = actions.sort.direction === 'ascending' ? '升序' : '降序';
  const nextDirection = actions.sort.direction === 'ascending' ? '降序' : '升序';
  return (
    <button
      type='button'
      className={`work-files-sort-heading ${active ? 'active' : ''}`}
      aria-label={active ? `${label}，当前${currentDirection}，切换为${nextDirection}` : `${label}，点击排序`}
      onClick={() =>
        actions.setSort({
          key: sortKey,
          direction:
            active && actions.sort.direction === 'ascending'
              ? 'descending'
              : active
                ? 'ascending'
                : actions.sort.direction,
        })
      }
    >
      <span>{label}</span>
      {active &&
        (actions.sort.direction === 'ascending' ? (
          <ArrowUp size={11} aria-hidden='true' />
        ) : (
          <ArrowDown size={11} aria-hidden='true' />
        ))}
    </button>
  );
}

function contextMenuItems({
  entry,
  currentPath,
  selectedPaths,
  selectedEntries,
  onOpen,
  onQuickLook,
  onCreateFolder,
  onRename,
  onDuplicate,
  onDelete,
  clipboard,
  onCopy,
  onCut,
  onPaste,
  onCopyPath,
  onReveal,
  onCreateKnowledgeBase,
  visibleCount,
  onSelectAll,
  layout,
  onSetLayout,
  sort,
  onSetSort,
  onRefresh,
  favoritePaths,
  onToggleFavorite,
  onAgentRequest,
  onCreateArtifact,
}: {
  entry: WorkspaceEntry | null;
  currentPath: string;
  selectedPaths: string[];
  selectedEntries: WorkspaceEntry[];
  onOpen: (entry: WorkspaceEntry) => void;
  onQuickLook: (entry: WorkspaceEntry) => void;
  onCreateFolder: () => void;
  onRename: (entry: WorkspaceEntry) => void;
  onDuplicate: (entry: WorkspaceEntry) => void;
  onDelete: (entries: WorkspaceEntry[]) => void;
  clipboard: WorkFilesActions['clipboard'];
  onCopy: (entries: readonly WorkspaceEntry[]) => void;
  onCut: (entries: readonly WorkspaceEntry[]) => void;
  onPaste: (destinationDirectory?: string) => Promise<void>;
  onCopyPath: (paths: readonly string[]) => void;
  onReveal: (path: string) => void;
  onCreateKnowledgeBase: (paths: readonly string[]) => void;
  visibleCount: number;
  onSelectAll: () => void;
  layout: WorkFilesActions['layout'];
  onSetLayout: WorkFilesActions['setLayout'];
  sort: WorkFilesActions['sort'];
  onSetSort: WorkFilesActions['setSort'];
  onRefresh: () => Promise<void>;
  favoritePaths: string[];
  onToggleFavorite: (path: string) => void;
  onAgentRequest: (request: WorkAgentRequest) => void | Promise<void>;
  onCreateArtifact?: (templateId: string) => void;
}): WorkspaceContextMenuItem[] {
  if (!entry) {
    const items: WorkspaceContextMenuItem[] = [
      {
        id: 'new-folder',
        label: '新建文件夹',
        icon: <FolderPlus size={14} />,
        onSelect: onCreateFolder,
      },
    ];
    if (onCreateArtifact) {
      items.push(
        {
          id: 'new-document',
          label: '新建文字文档',
          icon: <FileText size={14} />,
          onSelect: () => onCreateArtifact('blank-document'),
        },
        {
          id: 'new-spreadsheet',
          label: '新建电子表格',
          icon: <Sheet size={14} />,
          onSelect: () => onCreateArtifact('blank-spreadsheet'),
        },
        {
          id: 'new-presentation',
          label: '新建演示文稿',
          icon: <Presentation size={14} />,
          onSelect: () => onCreateArtifact('blank-presentation'),
        }
      );
    }
    items.push(
      {
        id: 'paste',
        label: clipboard ? `粘贴 ${clipboard.entries.length} 项` : '粘贴',
        icon: <ClipboardPaste size={14} />,
        shortcut: '⌘V',
        ariaKeyShortcut: 'Control+V Meta+V',
        disabled: !clipboard,
        separatorBefore: true,
        onSelect: () => void onPaste(currentPath).catch(() => undefined),
      },
      {
        id: 'select-all',
        label: '全选',
        icon: <Check size={14} />,
        shortcut: '⌘A',
        ariaKeyShortcut: 'Control+A Meta+A',
        disabled: visibleCount === 0,
        onSelect: onSelectAll,
      },
      {
        id: 'refresh',
        label: '刷新',
        icon: <RefreshCw size={14} />,
        separatorBefore: true,
        onSelect: () => void onRefresh(),
      },
      {
        id: 'grid-view',
        label: '图标视图',
        icon: <Grid2X2 size={14} />,
        checked: layout === 'grid',
        onSelect: () => onSetLayout('grid'),
      },
      {
        id: 'list-view',
        label: '列表视图',
        icon: <List size={14} />,
        checked: layout === 'list',
        onSelect: () => onSetLayout('list'),
      },
      {
        id: 'sort-name',
        label: '按名称排序',
        icon: <ArrowDown size={14} />,
        checked: sort.key === 'name',
        separatorBefore: true,
        onSelect: () => onSetSort({ ...sort, key: 'name' }),
      },
      {
        id: 'sort-modified',
        label: '按修改日期排序',
        icon: <ArrowDown size={14} />,
        checked: sort.key === 'modified',
        onSelect: () => onSetSort({ ...sort, key: 'modified' }),
      },
      {
        id: 'create-knowledge-base',
        label: '从当前文件夹创建知识库',
        icon: <BookPlus size={14} />,
        separatorBefore: true,
        onSelect: () => onCreateKnowledgeBase([currentPath]),
      },
      {
        id: 'organize',
        label: '用 AI 助手整理当前文件夹',
        icon: <Sparkles size={14} />,
        onSelect: () =>
          void onAgentRequest({
            workspaceRoot: '',
            paths: [currentPath],
            instruction:
              '请分析当前文件夹的内容和结构，提出一份清晰、可执行的整理方案。先只给出建议，不要移动、重命名或删除任何文件，除非我明确确认。',
          }),
      },
      {
        id: 'copy-current-path',
        label: '复制当前文件夹路径',
        icon: <Copy size={14} />,
        separatorBefore: true,
        onSelect: () => onCopyPath([currentPath]),
      },
      {
        id: 'reveal-current',
        label: '在系统文件管理器中显示',
        icon: <FolderSearch size={14} />,
        onSelect: () => onReveal(currentPath),
      }
    );
    return items;
  }

  const items: WorkspaceContextMenuItem[] = [
    {
      id: 'open',
      label: entry.isDirectory ? '打开文件夹' : '打开',
      icon: entry.isDirectory ? <FolderOpen size={14} /> : <FileInput size={14} />,
      disabled: !isWorkOpenableEntry(entry),
      onSelect: () => onOpen(entry),
    },
    {
      id: 'quick-look',
      label: '快速查看',
      icon: <Eye size={14} />,
      shortcut: '空格',
      ariaKeyShortcut: 'Space',
      onSelect: () => onQuickLook(entry),
    },
    {
      id: 'ask',
      label: '询问 AI 助手',
      icon: <MessageSquareText size={14} />,
      separatorBefore: true,
      onSelect: () =>
        void onAgentRequest({
          workspaceRoot: '',
          paths: selectedPaths,
          instruction: '请查看已选文件或文件夹，并围绕它们回答我的问题：\n\n问题：',
        }),
    },
    {
      id: 'create-knowledge-base',
      label: selectedPaths.length > 1 ? `从所选 ${selectedPaths.length} 项创建知识库` : '创建知识库',
      icon: <BookPlus size={14} />,
      onSelect: () => onCreateKnowledgeBase(selectedPaths),
    },
    {
      id: 'copy',
      label: selectedEntries.length > 1 ? `复制 ${selectedEntries.length} 项` : '复制',
      icon: <ClipboardCopy size={14} />,
      shortcut: '⌘C',
      ariaKeyShortcut: 'Control+C Meta+C',
      separatorBefore: true,
      onSelect: () => onCopy(selectedEntries),
    },
    {
      id: 'cut',
      label: selectedEntries.length > 1 ? `剪切 ${selectedEntries.length} 项` : '剪切',
      icon: <Scissors size={14} />,
      shortcut: '⌘X',
      ariaKeyShortcut: 'Control+X Meta+X',
      onSelect: () => onCut(selectedEntries),
    },
    {
      id: 'copy-path',
      label: selectedPaths.length > 1 ? `复制 ${selectedPaths.length} 个路径` : '复制路径',
      icon: <Copy size={14} />,
      onSelect: () => onCopyPath(selectedPaths),
    },
  ];
  if (selectedEntries.length === 1) {
    items.push({
      id: 'reveal',
      label: '在系统文件管理器中显示',
      icon: <FolderSearch size={14} />,
      onSelect: () => onReveal(entry.path),
    });
  }
  if (entry.isDirectory) {
    const favorite = favoritePaths.some((path) => sameLocalPath(path, entry.path));
    items.push(
      {
        id: 'paste-into',
        label: clipboard ? `粘贴 ${clipboard.entries.length} 项到此文件夹` : '粘贴到此文件夹',
        icon: <ClipboardPaste size={14} />,
        disabled: !clipboard,
        onSelect: () => void onPaste(entry.path).catch(() => undefined),
      },
      {
        id: 'favorite',
        label: favorite ? '从侧边栏移除' : '添加到侧边栏',
        icon: <Star size={14} fill={favorite ? 'currentColor' : 'none'} />,
        onSelect: () => onToggleFavorite(entry.path),
      }
    );
  }
  const selectedDirectoryCount = selectedEntries.filter((item) => item.isDirectory).length;
  const selectionIncludesDirectories = selectedDirectoryCount > 0;
  const selectionIncludesFiles = selectedDirectoryCount < selectedEntries.length;
  if (selectionIncludesDirectories) {
    const mixedSelection = selectionIncludesFiles;
    items.push({
      id: 'organize',
      label: mixedSelection
        ? '用 AI 助手整理所选项目'
        : selectedPaths.length > 1
          ? '用 AI 助手整理所选文件夹'
          : '用 AI 助手整理文件夹',
      icon: <Sparkles size={14} />,
      onSelect: () =>
        void onAgentRequest({
          workspaceRoot: '',
          paths: selectedPaths,
          instruction: mixedSelection
            ? '请分析已选文件和文件夹的内容与结构，提出一份清晰、可执行的整理方案。先只给出建议，不要移动、重命名或删除任何文件，除非我明确确认。'
            : '请分析已选文件夹的内容和结构，提出一份清晰、可执行的整理方案。先只给出建议，不要移动、重命名或删除任何文件，除非我明确确认。',
        }),
    });
  } else {
    items.push(
      {
        id: 'summarize',
        label: selectedPaths.length > 1 ? '总结所选文件' : '总结文件',
        icon: <TextSearch size={14} />,
        onSelect: () =>
          void onAgentRequest({
            workspaceRoot: '',
            paths: selectedPaths,
            instruction: '请总结已选文件的核心内容、关键结论和需要注意的事项。先生成摘要，不要修改文件。',
          }),
      },
      {
        id: 'suggest-name',
        label: selectedPaths.length > 1 ? '建议更清晰的文件名' : '建议更清晰的文件名',
        icon: <Tags size={14} />,
        onSelect: () =>
          void onAgentRequest({
            workspaceRoot: '',
            paths: selectedPaths,
            instruction:
              '请根据已选文件的内容建议更清晰、易检索的文件名，并简要说明命名理由。先只给出建议，不要执行重命名。',
          }),
      }
    );
  }
  if (selectedEntries.length === 1) {
    items.push(
      {
        id: 'rename',
        label: '重命名',
        icon: <Pencil size={14} />,
        shortcut: 'F2',
        ariaKeyShortcut: 'F2',
        separatorBefore: true,
        onSelect: () => onRename(entry),
      },
      {
        id: 'duplicate',
        label: '创建副本',
        icon: <Copy size={14} />,
        onSelect: () => onDuplicate(entry),
      }
    );
  }
  items.push({
    id: 'delete',
    label: selectedEntries.length > 1 ? `永久删除 ${selectedEntries.length} 项` : '永久删除',
    icon: <Trash2 size={14} />,
    shortcut: 'Delete',
    ariaKeyShortcut: 'Delete',
    separatorBefore: true,
    onSelect: () => onDelete(selectedEntries),
  });
  return items;
}

function finderKeyboardTargetIndex(
  key: string,
  layout: WorkFilesActions['layout'],
  currentIndex: number,
  items: HTMLElement[]
): number | null {
  if (!items.length) return null;
  if (key === 'Home') return 0;
  if (key === 'End') return items.length - 1;
  if (!['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(key)) return null;
  if (layout === 'list' && (key === 'ArrowLeft' || key === 'ArrowRight')) return null;
  if (currentIndex < 0) return key === 'ArrowUp' || key === 'ArrowLeft' ? items.length - 1 : 0;
  const columns = layout === 'grid' ? finderGridColumnCount(items) : 1;
  const offset = key === 'ArrowUp' ? -columns : key === 'ArrowDown' ? columns : key === 'ArrowLeft' ? -1 : 1;
  return Math.max(0, Math.min(items.length - 1, currentIndex + offset));
}

function finderGridColumnCount(items: HTMLElement[]): number {
  const firstTop = items[0]?.getBoundingClientRect().top;
  if (firstTop === undefined) return 1;
  const nextRow = items.findIndex((item, index) => index > 0 && item.getBoundingClientRect().top > firstTop + 1);
  return nextRow > 0 ? nextRow : 1;
}

function isSelectionControlTarget(target: EventTarget | null): boolean {
  return target instanceof Element && Boolean(target.closest('[data-work-file-selection-control]'));
}

function isBackgroundContextTarget(target: EventTarget | null): boolean {
  return !(
    target instanceof Element &&
    target.closest(
      '[data-work-file-index], [data-work-inline-name-editor], .work-files-list-header, button, input, textarea, select, a, [contenteditable="true"]'
    )
  );
}

async function copyPathsToClipboard(paths: readonly string[]): Promise<void> {
  try {
    if (!navigator.clipboard?.writeText) throw new Error('当前浏览器不支持写入剪贴板。');
    await navigator.clipboard.writeText(paths.join('\n'));
    showToast(paths.length > 1 ? `已复制 ${paths.length} 个路径` : '路径已复制', 'success');
  } catch (error) {
    showToast(error instanceof Error ? error.message : '无法复制路径。', 'error');
  }
}

async function revealPath(path: string): Promise<void> {
  try {
    await codeApi.revealWorkspacePath(path);
  } catch (error) {
    showToast(error instanceof Error ? error.message : '无法打开系统文件管理器。', 'error');
  }
}
