import { Minus, Plus } from 'lucide-react';
import { type ButtonHTMLAttributes, Fragment, type ReactNode, useId, useRef, useState } from 'react';
import { Popover, Tabs } from '../../../design-system/primitives';
import { OfficeSlider } from './office-controls';

export interface WorkOfficeRibbonTab<T extends string> {
  id: T;
  label: string;
}

export interface WorkOfficeFileAction {
  id: string;
  label: string;
  icon?: ReactNode;
  shortcut?: string;
  disabled?: boolean;
  separatorBefore?: boolean;
  onSelect: () => void | Promise<void>;
}

export function WorkOfficeRibbon<T extends string>({
  ariaLabel,
  tabs,
  defaultTab,
  activeTab,
  onTabChange,
  panels,
  fileActions,
  className = '',
  toolbarClassName = '',
}: {
  ariaLabel: string;
  tabs: readonly WorkOfficeRibbonTab<T>[];
  defaultTab: T;
  activeTab?: T;
  onTabChange?: (tab: T) => void;
  panels: Record<T, ReactNode>;
  fileActions?: readonly WorkOfficeFileAction[];
  className?: string;
  toolbarClassName?: string;
}) {
  const reactId = useId().replaceAll(':', '');
  const [internalTab, setInternalTab] = useState(defaultTab);
  const selectedTab = activeTab ?? internalTab;
  const selectedLabel = tabs.find((tab) => tab.id === selectedTab)?.label ?? tabs[0]?.label ?? '';
  const selectedPanel = panels[selectedTab];
  const selectTab = (tab: T) => {
    if (activeTab === undefined) setInternalTab(tab);
    onTabChange?.(tab);
  };

  return (
    <section className={`work-office-ribbon ${className}`.trim()} aria-label={ariaLabel}>
      <div className='work-office-ribbon-tabs-row'>
        {fileActions?.length ? <WorkOfficeFileMenu actions={fileActions} /> : null}
        <Tabs
          ariaLabel={ariaLabel}
          value={selectedTab}
          variant='line'
          size='compact'
          className='work-office-ribbon-tabs'
          items={tabs.map((tab) => ({
            ...tab,
            tabId: `${reactId}-tab-${tab.id}`,
            panelId: `${reactId}-panel`,
          }))}
          onChange={selectTab}
        />
      </div>
      <div
        id={`${reactId}-panel`}
        className='work-office-ribbon-panel'
        data-empty={selectedPanel === null || selectedPanel === undefined ? 'true' : undefined}
        role='tabpanel'
        aria-labelledby={`${reactId}-tab-${selectedTab}`}
      >
        {selectedPanel !== null && selectedPanel !== undefined && (
          <div
            className={`work-office-toolbar ${toolbarClassName}`.trim()}
            role='toolbar'
            aria-label={`${selectedLabel}工具栏`}
          >
            {selectedPanel}
          </div>
        )}
      </div>
    </section>
  );
}

function WorkOfficeFileMenu({ actions }: { actions: readonly WorkOfficeFileAction[] }) {
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLElement>(null);
  const focusEdgeRef = useRef<'first' | 'last'>('first');

  const focusRequestedAction = () =>
    requestAnimationFrame(() => {
      const buttons = [...(menuRef.current?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? [])];
      const button = focusEdgeRef.current === 'last' ? buttons.at(-1) : buttons[0];
      button?.focus();
    });

  return (
    <Popover
      label='文件'
      panelLabel='文件菜单'
      panelRole='menu'
      className='work-office-file-menu'
      panelClassName='work-office-file-popover'
      panelRef={menuRef}
      onPanelKeyDown={(event) => moveFileMenuFocus(event, triggerRef)}
      onOpenChange={(open) => {
        if (open) focusRequestedAction();
      }}
      trigger={(triggerProps, { open }) => (
        <button
          {...triggerProps}
          ref={(element) => {
            triggerProps.ref(element);
            triggerRef.current = element;
          }}
          className='work-office-file-trigger'
          onKeyDown={(event) => {
            if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
            event.preventDefault();
            focusEdgeRef.current = event.key === 'ArrowUp' ? 'last' : 'first';
            if (!open) event.currentTarget.click();
            else focusRequestedAction();
          }}
        >
          文件
        </button>
      )}
    >
      {(close) => (
        <>
          {actions.map((action) => (
            <Fragment key={action.id}>
              {action.separatorBefore && <hr className='work-office-file-separator' />}
              <button
                type='button'
                role='menuitem'
                disabled={action.disabled}
                onClick={() => {
                  close();
                  void action.onSelect();
                }}
              >
                <span className='work-office-file-action-icon' aria-hidden='true'>
                  {action.icon}
                </span>
                <span>{action.label}</span>
                {action.shortcut && <kbd>{action.shortcut}</kbd>}
              </button>
            </Fragment>
          ))}
        </>
      )}
    </Popover>
  );
}

function moveFileMenuFocus(
  event: React.KeyboardEvent<HTMLElement>,
  triggerRef: React.RefObject<HTMLButtonElement | null>
) {
  if (event.key === 'Tab') return;
  if (event.key === 'Escape') {
    event.preventDefault();
    triggerRef.current?.focus();
    return;
  }
  if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
  const buttons = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
  if (!buttons.length) return;
  event.preventDefault();
  const currentIndex = buttons.indexOf(document.activeElement as HTMLButtonElement);
  const nextIndex =
    event.key === 'Home'
      ? 0
      : event.key === 'End'
        ? buttons.length - 1
        : event.key === 'ArrowDown'
          ? (currentIndex + 1 + buttons.length) % buttons.length
          : (currentIndex - 1 + buttons.length) % buttons.length;
  buttons[nextIndex]?.focus();
}

export function WorkOfficeRibbonGroup({ label, children }: { label: string; children: ReactNode }) {
  return (
    <section className='work-office-ribbon-group' aria-label={label}>
      <div>{children}</div>
      <span aria-hidden='true'>{label}</span>
    </section>
  );
}

export function WorkOfficeRibbonButton({
  label,
  visibleLabel = label,
  badge,
  active,
  displayLabel = true,
  className = '',
  children,
  ...props
}: Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'aria-label' | 'children'> & {
  label: string;
  visibleLabel?: string;
  badge?: ReactNode;
  active?: boolean;
  displayLabel?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type='button'
      aria-label={label}
      aria-pressed={active}
      className={`${displayLabel ? 'with-label' : ''} ${active ? 'active' : ''} ${className}`.trim()}
      {...props}
    >
      {children}
      {displayLabel && <span>{visibleLabel}</span>}
      {badge !== undefined && <small className='work-office-ribbon-badge'>{badge}</small>}
    </button>
  );
}

export function WorkOfficeStatusBar({
  children,
  controls,
  className = '',
}: {
  children: ReactNode;
  controls?: ReactNode;
  className?: string;
}) {
  return (
    <footer className={`work-office-status ${className}`.trim()}>
      <div className='work-office-status-info'>{children}</div>
      {controls && <div className='work-office-status-view'>{controls}</div>}
    </footer>
  );
}

export function WorkOfficeZoomControls({
  zoom,
  minimum = 50,
  maximum = 200,
  step = 10,
  decreaseLabel,
  increaseLabel,
  outputLabel,
  sliderLabel,
  onChange,
}: {
  zoom: number;
  minimum?: number;
  maximum?: number;
  step?: number;
  decreaseLabel: string;
  increaseLabel: string;
  outputLabel: string;
  sliderLabel: string;
  onChange: (zoom: number) => void;
}) {
  const clamp = (value: number) => Math.min(maximum, Math.max(minimum, Math.round(value)));
  return (
    <>
      <button
        type='button'
        aria-label={decreaseLabel}
        title={decreaseLabel}
        disabled={zoom <= minimum}
        onClick={() => onChange(clamp(zoom - step))}
      >
        <Minus size={13} />
      </button>
      <output aria-label={outputLabel}>{zoom}%</output>
      <OfficeSlider
        className='work-office-status-slider'
        ariaLabel={sliderLabel}
        min={minimum}
        max={maximum}
        step={step}
        value={zoom}
        onValueChange={(value) => onChange(clamp(value))}
      />
      <button
        type='button'
        aria-label={increaseLabel}
        title={increaseLabel}
        disabled={zoom >= maximum}
        onClick={() => onChange(clamp(zoom + step))}
      >
        <Plus size={13} />
      </button>
    </>
  );
}
