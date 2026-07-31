import {
  BarChart3,
  ArrowRight,
  FileText,
  FolderCog,
  FolderOpen,
  FolderTree,
  Presentation,
  MessageSquareText,
  Plus,
  Sheet,
  ShieldCheck,
  WandSparkles,
} from 'lucide-react';
import type { ComponentType } from 'react';
import { appState } from '../../../state/app-state';
import { TaskComposer } from '../../tasks/components/task-composer';
import type { TaskActions } from '../../tasks/task-actions';

interface WorkHomeHeroProps {
  taskActions: TaskActions;
  activeSessionTitle?: string | null;
  onContinueSession?: () => void;
  onNewTask?: () => void;
  onTaskSubmit: () => void;
  onCreate: (templateId: string) => void;
  onImport: () => void;
  onOpenWorkspace: () => void;
}

interface WorkHomeCapability {
  id: string;
  label: string;
  icon: ComponentType<{ size?: string | number }>;
  tone: 'blue' | 'green' | 'orange' | 'violet' | 'cyan' | 'yellow' | 'slate';
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
  const setTaskPrompt = (prompt: string) => {
    appState.composerValue = prompt;
  };
  const capabilities: WorkHomeCapability[] = [
    {
      id: 'document',
      label: '新建文档',
      icon: FileText,
      tone: 'blue',
      run: () => onCreate('blank-document'),
    },
    {
      id: 'spreadsheet',
      label: '新建表格',
      icon: Sheet,
      tone: 'green',
      run: () => onCreate('blank-spreadsheet'),
    },
    {
      id: 'presentation',
      label: '新建演示',
      icon: Presentation,
      tone: 'orange',
      run: () => onCreate('blank-presentation'),
    },
    {
      id: 'open-file',
      label: '打开文件',
      icon: FolderOpen,
      tone: 'violet',
      run: onImport,
    },
    {
      id: 'analyze-data',
      label: '分析数据',
      icon: BarChart3,
      tone: 'cyan',
      run: () => setTaskPrompt(DATA_ANALYSIS_PROMPT),
    },
    {
      id: 'organize-files',
      label: '整理文件',
      icon: FolderCog,
      tone: 'yellow',
      run: () => setTaskPrompt(FILE_ORGANIZATION_PROMPT),
    },
    {
      id: 'workspace',
      label: '浏览工作区',
      icon: FolderTree,
      tone: 'slate',
      run: onOpenWorkspace,
    },
  ];

  return (
    <section className='work-home-hero' aria-labelledby='work-home-hero-title'>
      <header className='work-home-hero-intro'>
        <span className='work-home-agent-mark' aria-hidden='true'>
          <WandSparkles size={23} />
        </span>
        <div>
          <p>A3S · 本地智能工作台</p>
          <h1 id='work-home-hero-title'>今天想完成什么？</h1>
        </div>
      </header>
      <p className='work-home-hero-description'>
        选择工作区，描述你想要的结果。A3S 会读取相关文件、完成工作，并把可审阅的产物留在本地。
      </p>
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
      <nav className='work-home-capabilities' aria-label='Work 快捷能力'>
        {capabilities.map((capability) => {
          const Icon = capability.icon;
          return (
            <button type='button' key={capability.id} data-tone={capability.tone} onClick={capability.run}>
              <span aria-hidden='true'>
                <Icon size={16} />
              </span>
              {capability.label}
            </button>
          );
        })}
      </nav>
      <p className='work-home-assurance'>
        <ShieldCheck size={13} aria-hidden='true' />
        当前工作区会持续显示；涉及移动、重命名等更改时，A3S 会先说明影响并请求确认。
      </p>
    </section>
  );
}
