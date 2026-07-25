import { FolderInput, LibraryBig, LoaderCircle, Plus, RefreshCw } from 'lucide-react';
import { ProductSidebar, SidebarNavIcon } from '../../../components/product-sidebar';
import { Button } from '../../../design-system/primitives';

interface KnowledgeSidebarProps {
  count: number;
  libraryActive: boolean;
  refreshing: boolean;
  onShowLibrary: () => void;
  onCollapse: () => void;
  onCreate: () => void;
  onImport: () => void;
  onRefresh: () => void;
}

export function KnowledgeSidebar({
  count,
  libraryActive,
  refreshing,
  onShowLibrary,
  onCollapse,
  onCreate,
  onImport,
  onRefresh,
}: KnowledgeSidebarProps) {
  return (
    <ProductSidebar className='work-sidebar knowledge-sidebar' label='知识导航' title='知识' onCollapse={onCollapse}>
      <nav className='sidebar-nav-list' aria-label='知识库范围'>
        <span className='sidebar-section-label'>知识库</span>
        <button
          type='button'
          className={`sidebar-nav-item ${libraryActive ? 'active' : ''}`}
          aria-label={`我的知识库 ${count}`}
          aria-current={libraryActive ? 'page' : undefined}
          onClick={onShowLibrary}
        >
          <SidebarNavIcon>
            <LibraryBig size={15} />
          </SidebarNavIcon>
          <span className='sidebar-nav-label'>我的知识库</span>
          <small className='sidebar-nav-count'>{count}</small>
        </button>
      </nav>

      <section
        className='sidebar-nav-list sidebar-action-group work-sidebar-create knowledge-sidebar-quick-actions'
        aria-label='快速操作'
      >
        <span className='sidebar-section-label'>快速操作</span>
        <Button className='sidebar-nav-item' tone='quiet' onClick={onCreate}>
          <SidebarNavIcon tone='blue'>
            <Plus size={15} />
          </SidebarNavIcon>
          <span className='sidebar-nav-label'>新建</span>
        </Button>
        <Button className='sidebar-nav-item' tone='quiet' onClick={onImport}>
          <SidebarNavIcon tone='green'>
            <FolderInput size={15} />
          </SidebarNavIcon>
          <span className='sidebar-nav-label'>导入</span>
        </Button>
        <Button
          className='sidebar-nav-item'
          tone='quiet'
          disabled={refreshing}
          aria-busy={refreshing || undefined}
          onClick={onRefresh}
        >
          <SidebarNavIcon tone='orange'>
            {refreshing ? <LoaderCircle className='spin' size={15} /> : <RefreshCw size={15} />}
          </SidebarNavIcon>
          <span className='sidebar-nav-label'>{refreshing ? '刷新中…' : '刷新'}</span>
        </Button>
      </section>
    </ProductSidebar>
  );
}
