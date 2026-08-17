import {
  BarChart3,
  ArrowRight,
  ChevronDown,
  FileText,
  FolderCog,
  FolderOpen,
  FolderTree,
  Presentation,
  MessageSquareText,
  Plus,
  Sheet,
  ShieldCheck,
} from 'lucide-react';
import { type ComponentType, useEffect, useId, useRef, useState } from 'react';
import { appState } from '../../../state/app-state';
import { TaskComposer } from '../../tasks/components/task-composer';
import type { TaskActions } from '../../tasks/task-actions';
import { WORK_TEMPLATES } from '../work-templates';

interface WorkHomeHeroProps {
  taskActions: TaskActions;
  activeSessionTitle?: string | null;
  onContinueSession?: () => void;
  onNewTask?: () => void;
  onTaskSubmit: (content: string) => void;
  onCreate: (templateId: string) => void;
  onImport: () => void;
  onOpenWorkspace: () => void;
}

interface WorkHomeCapability {
  id: string;
  label: string;
  icon: ComponentType<{ size?: string | number }>;
  run: () => void;
}

const DATA_ANALYSIS_PROMPT = '分析我选择的数据文件，识别关键趋势、异常和相互关系，并给出有证据支持的结论与下一步建议。';
const FILE_ORGANIZATION_PROMPT =
  '检查当前工作区的文件结构，提出清晰、安全且可回退的整理方案；先展示计划，得到确认后再执行移动或重命名。';

export function WorkHomeHero({
  taskActions,
  activeSessionTitle,
  onContinueSession,
  onNewTask,
  onTaskSubmit,
  onCreate,
  onImport,
  onOpenWorkspace,
}: WorkHomeHeroProps) {
  const createMenuId = useId();
  const createMenuRef = useRef<HTMLDivElement>(null);
  const createMenuButtonRef = useRef<HTMLButtonElement>(null);
  const [createMenuOpen, setCreateMenuOpen] = useState(false);
  const setTaskPrompt = (prompt: string) => {
    appState.composerValue = prompt;
  };
  useEffect(() => {
    if (!createMenuOpen) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!createMenuRef.current?.contains(event.target as Node)) setCreateMenuOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      setCreateMenuOpen(false);
      createMenuButtonRef.current?.focus();
    };
    document.addEventListener('pointerdown', onPointerDown);
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('pointerdown', onPointerDown);
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [createMenuOpen]);
  const capabilities: WorkHomeCapability[] = [
    {
      id: 'workspace',
      label: '浏览工作区',
      icon: FolderTree,
      run: onOpenWorkspace,
    },
    {
      id: 'open-file',
      label: '打开文件',
      icon: FolderOpen,
      run: onImport,
    },
    {
      id: 'analyze-data',
      label: '分析数据',
      icon: BarChart3,
      run: () => setTaskPrompt(DATA_ANALYSIS_PROMPT),
    },
    {
      id: 'organize-files',
      label: '整理文件',
      icon: FolderCog,
      run: () => setTaskPrompt(FILE_ORGANIZATION_PROMPT),
    },
  ];

  return (
    <section className='work-home-hero' aria-labelledby='work-home-hero-title'>
      <header className='work-home-hero-intro'>
        <h1 id='work-home-hero-title'>今天想完成什么？</h1>
        <p className='work-home-hero-description'>
          描述你要的结果，A3S 会结合当前工作区完成任务，并留下可审阅的本地产物。
        </p>
      </header>
      {activeSessionTitle && (
        <div className='work-home-active-session'>
          <button
            type='button'
            className='work-home-active-session-link'
            aria-label={`打开当前任务：${activeSessionTitle}`}
            onClick={onContinueSession}
          >
            <span aria-hidden='true'>
              <MessageSquareText size={17} />
            </span>
            <span>
              <small>当前任务</small>
              <strong>{activeSessionTitle}</strong>
            </span>
            <span>
              打开任务
              <ArrowRight size={15} />
            </span>
          </button>
          <button type='button' className='work-home-new-task' onClick={onNewTask}>
            <Plus size={14} />
            新建任务
          </button>
        </div>
      )}
      <div className='work-home-composer'>
        <TaskComposer actions={taskActions} variant='preparation' onSubmitStart={onTaskSubmit} />
      </div>
      <nav className='work-home-actions' aria-label='常用操作'>
        {capabilities.slice(0, 2).map((capability) => {
          const Icon = capability.icon;
          return (
            <button type='button' key={capability.id} onClick={capability.run}>
              <Icon size={15} aria-hidden='true' />
              {capability.label}
            </button>
          );
        })}
        <div ref={createMenuRef} className={`work-home-create-menu ${createMenuOpen ? 'open' : ''}`}>
          <button
            ref={createMenuButtonRef}
            type='button'
            aria-controls={createMenuId}
            aria-expanded={createMenuOpen}
            onClick={() => setCreateMenuOpen((open) => !open)}
          >
            <Plus size={15} aria-hidden='true' />
            <span>新建</span>
            <ChevronDown className='work-home-create-chevron' size={13} aria-hidden='true' />
          </button>
          {createMenuOpen && (
            <fieldset id={createMenuId} className='work-home-create-panel' aria-label='新建文件与模板'>
              {WORK_TEMPLATES.map((template) => {
                const Icon =
                  template.kind === 'document' ? FileText : template.kind === 'spreadsheet' ? Sheet : Presentation;
                return (
                  <button
                    type='button'
                    key={template.id}
                    onClick={() => {
                      onCreate(template.id);
                      setCreateMenuOpen(false);
                    }}
                  >
                    <Icon size={16} aria-hidden='true' />
                    <span>
                      <strong>{template.name}</strong>
                      <small>{template.description}</small>
                    </span>
                  </button>
                );
              })}
            </fieldset>
          )}
        </div>
        {capabilities.slice(2).map((capability) => {
          const Icon = capability.icon;
          return (
            <button type='button' key={capability.id} onClick={capability.run}>
              <Icon size={15} aria-hidden='true' />
              {capability.label}
            </button>
          );
        })}
      </nav>
      <p className='work-home-assurance'>
        <ShieldCheck size={13} aria-hidden='true' />
        任务记录与产物由本地服务保存；文件更改会先说明影响并请求确认。
      </p>
    </section>
  );
}
