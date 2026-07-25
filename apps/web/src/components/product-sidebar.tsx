import { PanelLeftClose, PanelLeftOpen } from 'lucide-react';
import type { ReactNode } from 'react';
import { IconButton } from '../design-system/primitives';

export function ProductSidebar({
  title,
  label,
  className = '',
  headerActions,
  children,
  onCollapse,
}: {
  title: string;
  label: string;
  className?: string;
  headerActions?: ReactNode;
  children?: ReactNode;
  onCollapse: () => void;
}) {
  return (
    <aside className={`product-sidebar${className ? ` ${className}` : ''}`} aria-label={label}>
      <SidebarProductHeader title={title} onCollapse={onCollapse}>
        {headerActions}
      </SidebarProductHeader>
      {children}
    </aside>
  );
}

export function SidebarProductOpenButton({
  title,
  className,
  onOpen,
}: {
  title: string;
  className?: string;
  onOpen: () => void;
}) {
  return (
    <IconButton className={className} label={`展开${title}侧边栏`} onClick={onOpen}>
      <PanelLeftOpen size={16} />
    </IconButton>
  );
}

export function SidebarNavIcon({
  children,
  tone = 'neutral',
}: {
  children: ReactNode;
  tone?: 'neutral' | 'blue' | 'green' | 'orange' | 'purple';
}) {
  return (
    <span className='sidebar-nav-icon' data-tone={tone} aria-hidden='true'>
      {children}
    </span>
  );
}

function SidebarProductHeader({
  title,
  children,
  onCollapse,
}: {
  title: string;
  children?: ReactNode;
  onCollapse: () => void;
}) {
  return (
    <header className='sidebar-product-header'>
      <strong>{title}</strong>
      <div className='sidebar-product-header-actions'>
        {children}
        <IconButton label={`收起${title}侧边栏`} onClick={onCollapse}>
          <PanelLeftClose size={16} />
        </IconButton>
      </div>
    </header>
  );
}
