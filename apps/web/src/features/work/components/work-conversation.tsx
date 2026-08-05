import {
  ArrowLeft,
  CircleAlert,
  CircleCheck,
  CirclePause,
  FolderOpen,
  LoaderCircle,
  MessageSquareText,
  Plus,
  WifiOff,
} from 'lucide-react';
import { useSnapshot } from 'valtio';
import { Button, IconButton, StateView, StatusBadge } from '../../../design-system/primitives';
import { appState, sessionTitle } from '../../../state/app-state';
import type { AgentEvent, ChatMessage, CodeSession, TurnQueue } from '../../../types/api';
import type { TaskExecutionTiming } from '../../tasks/task-state';
import type { CodeActions } from '../../code/use-code-controller';
import { ExecutionStream } from '../../tasks/components/execution-stream';
import { TaskComposer } from '../../tasks/components/task-composer';
import { TaskRuntimeFloatingPanel } from '../../tasks/components/task-runtime-floating-panel';
import { ConversationSidebarOpenButton } from './conversation-sidebar-open-button';

interface WorkConversationProps {
  actions: CodeActions;
  sessionId: string | null;
  sidebarOpen: boolean;
  onOpenSidebar: () => void;
  onHome: () => void;
  onNewTask: () => void;
  onOpenWorkspace: () => void;
}

export function WorkConversation({
  actions,
  sessionId,
  sidebarOpen,
  onOpenSidebar,
  onHome,
  onNewTask,
  onOpenWorkspace,
}: WorkConversationProps) {
  const state = useSnapshot(appState);
  const session = sessionId ? state.sessions.find((item) => item.sessionId === sessionId) : undefined;
  const title = session ? sessionTitle(session, state.sessionTitles) : '任务不可用';
  const messages = sessionId
    ? (state.messagesBySession[sessionId] as unknown as readonly ChatMessage[] | undefined)
    : undefined;
  const status = taskConversationStatus({
    execution: sessionId ? state.executionTimings[sessionId] : undefined,
    messageError: sessionId ? state.messageErrors[sessionId] : undefined,
    messages,
    queue: sessionId ? (state.turnQueues[sessionId] as TurnQueue | undefined) : undefined,
    serviceStatus: state.serviceStatus,
    session: session as CodeSession | undefined,
    sessionId,
    streaming: Boolean(sessionId && state.streamingSessionId === sessionId),
  });
  const StatusIcon = status.icon;
  const active = Boolean(sessionId && session && state.activeSessionId === sessionId);

  return (
    <section
      className='work-conversation task-conversation-pane'
      aria-label={session ? `任务对话：${title}` : '任务对话'}
      data-session-id={sessionId ?? undefined}
    >
      <header className='work-conversation-header'>
        <div className='work-conversation-leading'>
          {!sidebarOpen && <ConversationSidebarOpenButton onOpen={onOpenSidebar} />}
          <IconButton label='返回首页' className='work-conversation-back' onClick={onHome}>
            <ArrowLeft size={16} />
          </IconButton>
          <span className='work-conversation-mark' aria-hidden='true'>
            <MessageSquareText size={16} />
          </span>
          <div className='work-conversation-identity'>
            <h1 title={title}>{title}</h1>
            <p title={session?.workspace || state.workspaceRoot || '未记录工作区'}>
              <FolderOpen size={12} aria-hidden='true' />
              <span>{session?.workspace || state.workspaceRoot || '未记录工作区'}</span>
            </p>
          </div>
        </div>
        <div className='work-conversation-actions'>
          <StatusBadge
            tone={status.tone}
            className='work-conversation-status'
            role='status'
            aria-live='polite'
            aria-atomic='true'
          >
            <StatusIcon className={status.spinning ? 'spin' : undefined} size={12} aria-hidden='true' />
            {status.label}
          </StatusBadge>
          <Button aria-label='打开工作区' size='compact' tone='quiet' onClick={onOpenWorkspace}>
            <FolderOpen size={14} />
            <span>打开工作区</span>
          </Button>
          <Button aria-label='新建任务' size='compact' onClick={onNewTask}>
            <Plus size={14} />
            <span>新建任务</span>
          </Button>
        </div>
      </header>

      <div className='work-conversation-body'>
        {active ? (
          <>
            <TaskRuntimeFloatingPanel />
            <ExecutionStream actions={actions} />
          </>
        ) : (
          <ConversationRouteState
            bootPhase={state.bootPhase}
            requestedSessionExists={Boolean(session)}
            sessionId={sessionId}
          />
        )}
      </div>
      {active && <TaskComposer actions={actions} />}
    </section>
  );
}

function ConversationRouteState({
  bootPhase,
  requestedSessionExists,
  sessionId,
}: {
  bootPhase: 'loading' | 'ready' | 'error';
  requestedSessionExists: boolean;
  sessionId: string | null;
}) {
  if (bootPhase === 'loading' || requestedSessionExists) {
    return (
      <div className='work-conversation-state-shell'>
        <StateView
          tone='info'
          role='status'
          icon={<LoaderCircle className='spin' size={24} />}
          title='正在打开任务'
          description='正在恢复这条任务的对话、运行参数和工作区上下文。'
        />
      </div>
    );
  }
  if (bootPhase === 'error') {
    return (
      <div className='work-conversation-state-shell'>
        <StateView
          tone='warning'
          role='alert'
          icon={<WifiOff size={24} />}
          title='暂时无法确认这个任务'
          description='本地 A3S 服务尚未恢复。重新连接后，这条链接会再次尝试加载对应任务。'
        />
      </div>
    );
  }
  return (
    <div className='work-conversation-state-shell'>
      <StateView
        tone='danger'
        role='alert'
        icon={<CircleAlert size={24} />}
        title={sessionId ? '找不到这个任务' : '没有指定任务'}
        description={
          sessionId
            ? '这条任务可能已被删除，或当前服务没有返回这条任务记录。请返回首页选择其他任务，或新建一个任务。'
            : '请返回首页新建任务，或从任务列表选择一条已有任务。'
        }
      />
    </div>
  );
}

interface TaskConversationStatusInput {
  execution?: TaskExecutionTiming;
  messageError?: string;
  messages?: readonly ChatMessage[];
  queue?: TurnQueue;
  serviceStatus: 'connected' | 'checking' | 'disconnected';
  session?: CodeSession;
  sessionId: string | null;
  streaming: boolean;
}

function taskConversationStatus({
  execution,
  messageError,
  messages,
  queue,
  serviceStatus,
  session,
  sessionId,
  streaming,
}: TaskConversationStatusInput): {
  icon: typeof CircleCheck;
  label: string;
  spinning?: boolean;
  tone: 'neutral' | 'info' | 'success' | 'warning' | 'danger';
} {
  if (serviceStatus !== 'connected') {
    return {
      icon: serviceStatus === 'checking' ? LoaderCircle : WifiOff,
      label: serviceStatus === 'checking' ? '正在重连' : '连接中断',
      spinning: serviceStatus === 'checking',
      tone: 'warning',
    };
  }
  if (sessionId && messageError) {
    return { icon: CircleAlert, label: '记录异常', tone: 'danger' };
  }
  if (sessionId && (streaming || execution?.status === 'running' || queue?.status === 'running' || queue?.active)) {
    return { icon: LoaderCircle, label: '执行中', spinning: true, tone: 'info' };
  }
  if (sessionId && queue?.paused) {
    return { icon: CirclePause, label: '已暂停', tone: 'warning' };
  }
  if (sessionId && (queue?.status === 'pending' || queue?.items.length)) {
    return { icon: LoaderCircle, label: '等待执行', tone: 'info' };
  }
  if (execution?.status === 'failed') {
    return { icon: CircleAlert, label: '执行失败', tone: 'danger' };
  }
  if (execution?.status === 'cancelled') {
    return { icon: CirclePause, label: '已停止', tone: 'warning' };
  }
  if (execution?.status === 'completed') {
    return { icon: CircleCheck, label: '已完成', tone: 'success' };
  }
  const messageStatus = latestTerminalMessageStatus(messages ?? []);
  if (messageStatus === 'failed') {
    return { icon: CircleAlert, label: '执行失败', tone: 'danger' };
  }
  if (messageStatus === 'cancelled') {
    return { icon: CirclePause, label: '已停止', tone: 'warning' };
  }
  if (messageStatus === 'completed') {
    return { icon: CircleCheck, label: '已完成', tone: 'success' };
  }
  const serviceState = session?.state.toLowerCase();
  if (serviceState === 'running') return { icon: LoaderCircle, label: '执行中', spinning: true, tone: 'info' };
  if (serviceState === 'failed' || serviceState === 'error') {
    return { icon: CircleAlert, label: '执行失败', tone: 'danger' };
  }
  if (serviceState === 'completed' || serviceState === 'done') {
    return { icon: CircleCheck, label: '已完成', tone: 'success' };
  }
  if (!session) return { icon: CircleAlert, label: '任务不可用', tone: 'warning' };
  return { icon: CircleCheck, label: '可继续', tone: 'neutral' };
}

function latestTerminalMessageStatus(messages: readonly ChatMessage[]): TaskExecutionTiming['status'] | null {
  for (let messageIndex = messages.length - 1; messageIndex >= 0; messageIndex -= 1) {
    const message = messages[messageIndex];
    if (message?.role !== 'assistant') continue;
    const events = (message.events ?? []) as readonly AgentEvent[];
    for (let eventIndex = events.length - 1; eventIndex >= 0; eventIndex -= 1) {
      const event = events[eventIndex];
      if (event?.type === 'error') return 'failed';
      if (event?.type === 'cancelled') return 'cancelled';
      if (event?.type === 'agent_end') return event.success === false ? 'failed' : 'completed';
    }
    return null;
  }
  return null;
}
