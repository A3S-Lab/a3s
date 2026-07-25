import { Clock3, Files, FolderOpen, HardDrive, Home, Presentation, Sheet, Star, Trash2, X } from 'lucide-react';
import { useState } from 'react';
import { ProductSidebar, SidebarNavIcon } from '../../../components/product-sidebar';
import { Button, IconButton } from '../../../design-system/primitives';
import { hasDraggedWorkspaceFiles } from '../../workspace/workspace-drop-import';
import {
  canMoveLocalPaths,
  hasWorkLocalFileDragData,
  localPathBasename,
  readWorkLocalFileDragData,
  sameLocalPath,
} from '../work-local-files';
import type { WorkFolder, WorkLibraryView } from '../work-types';
import { WorkFileIcon } from './work-file-icon';
import { WorkWorkspaceSwitcher } from './work-workspace-switcher';

interface WorkSidebarProps {
  surface: 'files' | 'library';
  localRootName: string;
  localRootPath: string;
  localCurrentPath: string;
  recentRootPaths: string[];
  localFavoritePaths: string[];
  view: WorkLibraryView;
  totalCount: number;
  favoriteCount: number;
  trashCount: number;
  folders: WorkFolder[];
  activeFolderId: string | null;
  onChangeView: (view: WorkLibraryView) => void;
  onOpenFolder: (id: string) => void;
  onOpenLocalFiles: () => void;
  onSelectWorkspace: (path: string) => Promise<string | null>;
  onPickWorkspace: () => Promise<string | null>;
  onOpenLocalFavorite: (path: string) => void;
  onRemoveLocalFavorite: (path: string) => void;
  onMoveLocalEntries: (paths: string[], destinationDirectory: string) => void | Promise<void>;
  onImportLocalDrop: (dataTransfer: DataTransfer, destinationDirectory: string) => void | Promise<unknown>;
  onCollapse: () => void;
  onCreate: (templateId: string) => void;
  onImport: () => void;
}

export function WorkSidebar({
  surface,
  localRootName,
  localRootPath,
  localCurrentPath,
  recentRootPaths,
  localFavoritePaths,
  view,
  totalCount,
  favoriteCount,
  trashCount,
  folders,
  activeFolderId,
  onChangeView,
  onOpenFolder,
  onOpenLocalFiles,
  onSelectWorkspace,
  onPickWorkspace,
  onOpenLocalFavorite,
  onRemoveLocalFavorite,
  onMoveLocalEntries,
  onImportLocalDrop,
  onCollapse,
  onCreate,
  onImport,
}: WorkSidebarProps) {
  const [dropTargetPath, setDropTargetPath] = useState<string | null>(null);
  const [externalDropTargetPath, setExternalDropTargetPath] = useState<string | null>(null);
  const rootFolders = folders.filter((folder) => !folder.parentId && !folder.trashedAt);
  const acceptDrop = (event: React.DragEvent<HTMLButtonElement>, destinationDirectory: string) => {
    if (!destinationDirectory) return;
    if (hasDraggedWorkspaceFiles(event.dataTransfer)) {
      event.preventDefault();
      event.stopPropagation();
      event.dataTransfer.dropEffect = 'copy';
      setDropTargetPath(null);
      setExternalDropTargetPath(destinationDirectory);
      return;
    }
    if (!hasWorkLocalFileDragData(event.dataTransfer)) return;
    const paths = readWorkLocalFileDragData(event.dataTransfer);
    if (paths.length && !canMoveLocalPaths(paths, destinationDirectory)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
    setExternalDropTargetPath(null);
    setDropTargetPath(destinationDirectory);
  };
  const finishDrop = (event: React.DragEvent<HTMLButtonElement>, destinationDirectory: string) => {
    if (hasDraggedWorkspaceFiles(event.dataTransfer)) {
      event.preventDefault();
      event.stopPropagation();
      setDropTargetPath(null);
      setExternalDropTargetPath(null);
      if (!destinationDirectory) return;
      void Promise.resolve(onImportLocalDrop(event.dataTransfer, destinationDirectory)).catch(() => undefined);
      return;
    }
    const paths = readWorkLocalFileDragData(event.dataTransfer);
    setDropTargetPath(null);
    setExternalDropTargetPath(null);
    if (!canMoveLocalPaths(paths, destinationDirectory)) return;
    event.preventDefault();
    void Promise.resolve(onMoveLocalEntries(paths, destinationDirectory)).catch(() => undefined);
  };
  const clearDropTarget = (event: React.DragEvent<HTMLButtonElement>) => {
    const related = event.relatedTarget;
    if (related instanceof Node && event.currentTarget.contains(related)) return;
    setDropTargetPath(null);
    setExternalDropTargetPath(null);
  };
  return (
    <ProductSidebar className='work-sidebar' label='办公文件导航' title='办公' onCollapse={onCollapse}>
      <WorkWorkspaceSwitcher
        rootPath={localRootPath}
        recentPaths={recentRootPaths}
        onSelect={onSelectWorkspace}
        onPick={onPickWorkspace}
      />

      <nav className='sidebar-nav-list' aria-label='文件范围'>
        <span className='sidebar-section-label'>位置</span>
        <button
          type='button'
          className={`sidebar-nav-item ${surface === 'files' && sameLocalPath(localCurrentPath, localRootPath) ? 'active' : ''} ${dropTargetPath === localRootPath || externalDropTargetPath === localRootPath ? 'drop-target' : ''} ${externalDropTargetPath === localRootPath ? 'external-drop-target' : ''}`}
          onClick={onOpenLocalFiles}
          onDragOver={(event) => acceptDrop(event, localRootPath)}
          onDragLeave={clearDropTarget}
          onDrop={(event) => finishDrop(event, localRootPath)}
        >
          <SidebarNavIcon>
            <HardDrive size={15} />
          </SidebarNavIcon>
          <span className='sidebar-nav-label'>{localRootName ? '全部文件' : '本地文件'}</span>
        </button>
        {localFavoritePaths.length > 0 && (
          <section className='work-sidebar-local-favorites' aria-label='本地收藏文件夹'>
            <span className='sidebar-section-label favorites'>个人收藏</span>
            {localFavoritePaths.map((path) => {
              const label = localPathBasename(path);
              return (
                <div className='work-sidebar-local-favorite' key={path}>
                  <button
                    type='button'
                    title={path}
                    aria-label={`打开收藏文件夹 ${label}`}
                    className={`sidebar-nav-item ${surface === 'files' && sameLocalPath(localCurrentPath, path) ? 'active' : ''} ${dropTargetPath === path || externalDropTargetPath === path ? 'drop-target' : ''} ${externalDropTargetPath === path ? 'external-drop-target' : ''}`}
                    onClick={() => onOpenLocalFavorite(path)}
                    onDragOver={(event) => acceptDrop(event, path)}
                    onDragLeave={clearDropTarget}
                    onDrop={(event) => finishDrop(event, path)}
                  >
                    <WorkFileIcon path={path} directory size={17} />
                    <span className='sidebar-nav-label'>{label}</span>
                  </button>
                  <IconButton
                    className='work-sidebar-favorite-remove'
                    label={`从侧边栏移除 ${label}`}
                    tooltip='从侧边栏移除'
                    onClick={() => onRemoveLocalFavorite(path)}
                  >
                    <X size={12} />
                  </IconButton>
                </div>
              );
            })}
          </section>
        )}
        <span className='sidebar-section-label library'>我的文件</span>
        <button
          type='button'
          className={`sidebar-nav-item ${surface === 'library' && view === 'home' ? 'active' : ''}`}
          onClick={() => onChangeView('home')}
        >
          <SidebarNavIcon>
            <Home size={15} />
          </SidebarNavIcon>
          <span className='sidebar-nav-label'>我的文档</span>
        </button>
        <button
          type='button'
          className={`sidebar-nav-item ${surface === 'library' && view === 'recent' ? 'active' : ''}`}
          onClick={() => onChangeView('recent')}
        >
          <SidebarNavIcon>
            <Clock3 size={15} />
          </SidebarNavIcon>
          <span className='sidebar-nav-label'>最近</span>
          <small className='sidebar-nav-count'>{totalCount}</small>
        </button>
        <button
          type='button'
          className={`sidebar-nav-item ${surface === 'library' && view === 'favorites' ? 'active' : ''}`}
          onClick={() => onChangeView('favorites')}
        >
          <SidebarNavIcon>
            <Star size={15} />
          </SidebarNavIcon>
          <span className='sidebar-nav-label'>收藏</span>
          <small className='sidebar-nav-count'>{favoriteCount}</small>
        </button>
        <button
          type='button'
          className={`sidebar-nav-item ${surface === 'library' && view === 'trash' ? 'active' : ''}`}
          onClick={() => onChangeView('trash')}
        >
          <SidebarNavIcon>
            <Trash2 size={15} />
          </SidebarNavIcon>
          <span className='sidebar-nav-label'>回收站</span>
          <small className='sidebar-nav-count'>{trashCount}</small>
        </button>
      </nav>

      {rootFolders.length > 0 && (
        <section className='sidebar-nav-list work-sidebar-folders' aria-label='文件夹'>
          <span className='sidebar-section-label'>文件夹</span>
          {rootFolders.map((folder) => (
            <button
              type='button'
              key={folder.id}
              className={`sidebar-nav-item ${surface === 'library' && view === 'folder' && activeFolderId === folder.id ? 'active' : ''}`}
              onClick={() => onOpenFolder(folder.id)}
            >
              <WorkFileIcon path={folder.name} directory size={17} />
              <span className='sidebar-nav-label'>{folder.name}</span>
            </button>
          ))}
        </section>
      )}

      <section className='sidebar-nav-list sidebar-action-group work-sidebar-create' aria-label='快速新建'>
        <span className='sidebar-section-label'>快速新建</span>
        <Button className='sidebar-nav-item' tone='quiet' onClick={() => onCreate('blank-document')}>
          <SidebarNavIcon tone='blue'>
            <Files size={15} />
          </SidebarNavIcon>
          <span className='sidebar-nav-label'>文字</span>
        </Button>
        <Button className='sidebar-nav-item' tone='quiet' onClick={() => onCreate('blank-spreadsheet')}>
          <SidebarNavIcon tone='green'>
            <Sheet size={15} />
          </SidebarNavIcon>
          <span className='sidebar-nav-label'>表格</span>
        </Button>
        <Button className='sidebar-nav-item' tone='quiet' onClick={() => onCreate('blank-presentation')}>
          <SidebarNavIcon tone='orange'>
            <Presentation size={15} />
          </SidebarNavIcon>
          <span className='sidebar-nav-label'>演示</span>
        </Button>
      </section>

      <Button tone='secondary' className='sidebar-nav-item work-sidebar-import' onClick={onImport}>
        <SidebarNavIcon>
          <FolderOpen size={15} />
        </SidebarNavIcon>
        <span className='sidebar-nav-label'>导入文件</span>
      </Button>
    </ProductSidebar>
  );
}
