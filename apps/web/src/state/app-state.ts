import { proxy } from 'valtio';
import type { CodeSession } from '../types/api';
import {
  createCodeShellState,
  type TaskView,
  type CodeShellState,
  type ThemePreference,
  type ToastState,
} from '../features/code/code-state';
import {
  createTaskState,
  persistActiveTask,
  persistTaskDrafts,
  taskDraftKey,
  type TaskState,
} from '../features/tasks/task-state';
import { rememberTaskContextFocus, restoreTaskContextFocus } from '../features/tasks/task-context-focus';
import {
  captureWorkspaceTaskSnapshot,
  createWorkspaceState,
  createWorkspaceTaskState,
  normalizePath,
  persistWorkspaceTaskSnapshots,
  restoreWorkspaceTaskState,
  type WorkspaceState,
} from '../features/workspace/workspace-state';
import { createRunsState, type RunsState } from '../features/runs/runs-state';
import { createSettingsState, type SettingsState } from '../features/settings/settings-state';
import type { SettingsTab } from '../features/settings/settings-state';
export type { TaskView, ThemePreference } from '../features/code/code-state';
type AppState = CodeShellState & TaskState & WorkspaceState & RunsState & SettingsState;

const titleStorageKey = 'a3s-code-web.session-titles';
const themeStorageKey = 'a3s-code-web.theme';
let modelChangeNoticeId = 0;

const initialTaskState = createTaskState();
const initialWorkspaceState = createWorkspaceState(taskDraftKey(initialTaskState.activeSessionId));
const initialCodeShellState = createCodeShellState();
const initialWorkspaceSnapshot =
  initialWorkspaceState.workspaceSnapshotsByTask[taskDraftKey(initialTaskState.activeSessionId)];
if (initialWorkspaceSnapshot) initialCodeShellState.taskView = initialWorkspaceSnapshot.taskView;

export const appState = proxy<AppState>({
  ...initialCodeShellState,
  ...initialTaskState,
  ...initialWorkspaceState,
  ...createRunsState(),
  ...createSettingsState(),
});

export function persistSessionTitle(sessionId: string, title: string): boolean {
  const normalized = title.trim().slice(0, 72) || '新任务';
  appState.sessionTitles[sessionId] = normalized;
  try {
    localStorage.setItem(titleStorageKey, JSON.stringify(appState.sessionTitles));
    return true;
  } catch {
    return false;
  }
}
export function removePersistedSessionTitle(sessionId: string): boolean {
  delete appState.sessionTitles[sessionId];
  try {
    localStorage.setItem(titleStorageKey, JSON.stringify(appState.sessionTitles));
    return true;
  } catch {
    return false;
  }
}

export function sessionTitle(
  session: Pick<CodeSession, 'sessionId' | 'title'>,
  titles: Readonly<Record<string, string>> = appState.sessionTitles
): string {
  return titles[session.sessionId] || session.title?.trim() || '新任务';
}

export interface WorkspaceContext {
  generation: number;
  workspaceRoot: string;
}

export function captureWorkspaceContext(): WorkspaceContext {
  return {
    generation: appState.workspaceGeneration,
    workspaceRoot: appState.workspaceRoot,
  };
}

export function isWorkspaceContextCurrent(context: WorkspaceContext): boolean {
  return (
    context.generation === appState.workspaceGeneration &&
    sameWorkspaceRoot(context.workspaceRoot, appState.workspaceRoot)
  );
}

export function switchActiveTask(sessionId: string | null, workspaceRoot?: string): boolean {
  if (sessionId === appState.activeSessionId) {
    const resolvedRoot = resolveWorkspaceRoot(sessionId, workspaceRoot);
    if (resolvedRoot && !sameWorkspaceRoot(resolvedRoot, appState.workspaceRoot)) {
      replaceActiveWorkspace(resolvedRoot);
    }
    return true;
  }

  const currentKey = taskDraftKey(appState.activeSessionId);
  appState.draftsByTask[currentKey] = {
    content: appState.composerValue,
    contextFiles: [...appState.composerContextFiles],
    skillNames: [...appState.composerSkills],
  };
  reportTaskPersistenceResult(persistTaskDrafts(appState.draftsByTask));
  appState.workspaceSnapshotsByTask[currentKey] = captureWorkspaceTaskSnapshot(appState, appState.taskView);
  appState.activeSessionId = sessionId;
  reportTaskPersistenceResult(persistActiveTask(sessionId));
  const nextKey = taskDraftKey(sessionId);
  const resolvedRoot = resolveWorkspaceRoot(sessionId, workspaceRoot);
  const stored = appState.workspaceSnapshotsByTask[nextKey];
  const nextWorkspace =
    stored && sameWorkspaceRoot(stored.state.workspaceRoot, resolvedRoot)
      ? stored
      : { taskView: 'conversation' as const, state: createWorkspaceTaskState(resolvedRoot) };
  if (stored && stored !== nextWorkspace) delete appState.workspaceSnapshotsByTask[nextKey];
  restoreWorkspaceTaskState(appState, nextWorkspace.state);
  appState.taskView = nextWorkspace.taskView;
  appState.workspaceGeneration += 1;
  appState.fileQuickOpenOpen = false;
  appState.commandPaletteOpen = false;
  const nextDraft = appState.draftsByTask[taskDraftKey(sessionId)];
  appState.composerValue = nextDraft?.content ?? '';
  appState.composerContextFiles = [...(nextDraft?.contextFiles ?? [])];
  appState.composerSkills = [...(nextDraft?.skillNames ?? [])];
  appState.modelChangeNotice = null;
  reportTaskPersistenceResult(persistActiveWorkspaceTask());
  return true;
}

export function promoteActiveTask(sessionId: string, workspaceRoot: string): void {
  if (appState.activeSessionId) {
    switchActiveTask(sessionId, workspaceRoot);
    return;
  }
  const preparedDraftKey = taskDraftKey(null);
  appState.draftsByTask[taskDraftKey(sessionId)] = {
    content: appState.composerValue,
    contextFiles: [...appState.composerContextFiles],
    skillNames: [...appState.composerSkills],
  };
  delete appState.draftsByTask[preparedDraftKey];
  reportTaskPersistenceResult(persistTaskDrafts(appState.draftsByTask));
  const rootChanged = !sameWorkspaceRoot(appState.workspaceRoot, workspaceRoot);
  if (rootChanged) {
    restoreWorkspaceTaskState(appState, createWorkspaceTaskState(workspaceRoot));
    appState.taskView = 'conversation';
    appState.workspaceGeneration += 1;
  } else {
    appState.workspaceRoot = workspaceRoot;
  }
  delete appState.workspaceSnapshotsByTask[preparedDraftKey];
  appState.activeSessionId = sessionId;
  reportTaskPersistenceResult(persistActiveTask(sessionId));
  appState.modelChangeNotice = null;
  reportTaskPersistenceResult(persistActiveWorkspaceTask());
}

export function replaceActiveWorkspace(workspaceRoot: string): void {
  if (sameWorkspaceRoot(workspaceRoot, appState.workspaceRoot)) {
    appState.workspaceRoot = workspaceRoot;
    reportTaskPersistenceResult(persistActiveWorkspaceTask());
    return;
  }
  const key = taskDraftKey(appState.activeSessionId);
  delete appState.workspaceSnapshotsByTask[key];
  restoreWorkspaceTaskState(appState, createWorkspaceTaskState(workspaceRoot));
  appState.taskView = 'conversation';
  appState.workspaceGeneration += 1;
  appState.fileQuickOpenOpen = false;
  reportTaskPersistenceResult(persistActiveWorkspaceTask());
}

export function removeWorkspaceTaskSnapshot(sessionId: string): void {
  delete appState.workspaceSnapshotsByTask[taskDraftKey(sessionId)];
  reportTaskPersistenceResult(persistActiveWorkspaceTask());
}

function persistActiveWorkspaceTask(): boolean {
  return persistWorkspaceTaskSnapshots(
    appState.workspaceSnapshotsByTask,
    taskDraftKey(appState.activeSessionId),
    appState,
    appState.taskView
  );
}

function resolveWorkspaceRoot(sessionId: string | null, explicit?: string): string {
  const requested = explicit?.trim();
  if (requested) return requested;
  const sessionWorkspace = sessionId
    ? appState.sessions.find((session) => session.sessionId === sessionId)?.workspace.trim()
    : appState.newTaskConfig.workspace.trim() || appState.health?.workspace.trim();
  if (sessionWorkspace) return sessionWorkspace;
  return appState.workspaceSnapshotsByTask[taskDraftKey(sessionId)]?.state.workspaceRoot || appState.workspaceRoot;
}

function sameWorkspaceRoot(left: string, right: string): boolean {
  const normalizedLeft = normalizePath(left).replace(/\/$/, '');
  const normalizedRight = normalizePath(right).replace(/\/$/, '');
  if (/^[A-Za-z]:\//.test(normalizedLeft) || /^[A-Za-z]:\//.test(normalizedRight)) {
    return normalizedLeft.toLowerCase() === normalizedRight.toLowerCase();
  }
  return normalizedLeft === normalizedRight;
}

export function reportTaskPersistenceResult(persisted: boolean): void {
  if (persisted || appState.taskPersistenceWarningShown) return;
  appState.taskPersistenceWarningShown = true;
  showToast('当前页面内容仍保留，但浏览器无法保存本地状态；刷新前请复制重要草稿。', 'error');
}

export function setTheme(theme: ThemePreference): void {
  appState.theme = theme;
  applyTheme(theme);
  try {
    localStorage.setItem(themeStorageKey, theme);
  } catch {
    reportTaskPersistenceResult(false);
  }
}

export function applyTheme(theme = appState.theme): void {
  const dark =
    theme === 'dark' ||
    (theme === 'system' &&
      typeof window.matchMedia === 'function' &&
      window.matchMedia('(prefers-color-scheme: dark)').matches);
  document.documentElement.classList.toggle('dark', dark);
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
}

export function showToast(message: string, tone: ToastState['tone'] = 'info'): void {
  appState.toast = { id: Date.now(), tone, message };
}

export function showModelChangeNotice(sessionId: string | null, previousModel: string, currentModel: string): void {
  if (!currentModel || previousModel === currentModel) return;
  modelChangeNoticeId += 1;
  appState.modelChangeNotice = {
    id: modelChangeNoticeId,
    sessionId,
    previousModel,
    currentModel,
  };
}

export function clearModelChangeNotice(id: number): void {
  if (appState.modelChangeNotice?.id === id) appState.modelChangeNotice = null;
}

export function appendTaskInstruction(content: string): void {
  appState.composerValue = [appState.composerValue.trim(), content.trim()].filter(Boolean).join('\n\n');
}

export function navigateTask(view: TaskView): void {
  const previousView = appState.taskView;
  if (previousView === 'conversation' && view !== 'conversation') rememberTaskContextFocus(view);
  appState.settingsOpen = false;
  if (view === 'conversation') appState.workspacePresentation = 'docked';
  appState.taskView = view;
  window.history.replaceState(null, '', `#code/${view}`);
  if (previousView !== 'conversation' && view === 'conversation') restoreTaskContextFocus(previousView);
}

export function navigateSettings(tab: SettingsTab): void {
  appState.settingsOpen = true;
  appState.settingsTab = tab;
  window.history.replaceState(null, '', `#settings/${tab}`);
}

export function closeSettings(): void {
  appState.settingsOpen = false;
  window.history.replaceState(null, '', `#code/${appState.taskView}`);
}

export function clearToast(id: number): void {
  if (appState.toast?.id === id) appState.toast = null;
}

export function formatApiError(error: unknown): string {
  if (!(error instanceof Error)) return '发生了未知错误';
  if (error.message === 'Failed to fetch' || error.message.includes('NetworkError'))
    return '无法访问本地 A3S Code 服务。';
  return error.message;
}
