import { proxy } from 'valtio';
import {
  conversationHash,
  type CodeShellState,
  createCodeShellState,
  parseShellLocation,
  type ProductId,
  type ShellLocation,
  type TaskView,
  type ThemePreference,
  type ToastState,
} from '../features/code/code-state';
import { createKnowledgeState, type KnowledgeState } from '../features/knowledge/knowledge-state';
import { createMemoryState, type MemoryState } from '../features/memory/memory-state';
import { createPluginsState, type PluginsState } from '../features/plugins/plugin-state';
import { createRunsState, type RunsState } from '../features/runs/runs-state';
import {
  type ChannelSettingsTab,
  createSettingsState,
  type SettingsState,
  type SettingsTab,
  settingsChannelFromHash,
  settingsHashForTab,
  settingsTabFromHash,
} from '../features/settings/settings-state';
import { rememberTaskContextFocus, restoreTaskContextFocus } from '../features/tasks/task-context-focus';
import {
  createTaskDraft,
  createTaskState,
  persistActiveTask,
  persistTaskDrafts,
  readActiveTask,
  type TaskState,
  taskDraftKey,
} from '../features/tasks/task-state';
import { createWeixinRemoteState, type WeixinRemoteState } from '../features/weixin-remote/weixin-remote-state';
import {
  captureWorkspaceTaskSnapshot,
  createWorkspaceState,
  createWorkspaceTaskState,
  normalizePath,
  persistWorkspaceTaskSnapshots,
  restoreWorkspaceTaskState,
  type WorkspaceState,
} from '../features/workspace/workspace-state';
import type { CodeSession } from '../types/api';

export type { ProductId, TaskView, ThemePreference, WorkRoute } from '../features/code/code-state';

type AppState = CodeShellState &
  TaskState &
  WorkspaceState &
  RunsState &
  SettingsState &
  MemoryState &
  PluginsState &
  KnowledgeState &
  WeixinRemoteState;

const titleStorageKey = 'a3s-code-web.session-titles';
const themeStorageKey = 'a3s-code-web.theme';
let modelChangeNoticeId = 0;

const initialShellState = createCodeShellState();
const initialTaskState = createTaskState(
  initialShellState.workRoute === 'conversation' ? initialShellState.conversationSessionId : undefined
);
const initialTaskKey = taskDraftKey(initialTaskState.activeSessionId);
const initialWorkspaceState = createWorkspaceState(initialTaskKey);
const initialWorkspaceSnapshot = initialWorkspaceState.workspaceSnapshotsByTask[initialTaskKey];
if (initialWorkspaceSnapshot) initialShellState.taskView = initialWorkspaceSnapshot.taskView;

export const appState = proxy<AppState>({
  ...initialShellState,
  ...initialTaskState,
  ...initialWorkspaceState,
  ...createRunsState(),
  ...createSettingsState(),
  ...createMemoryState(),
  ...createPluginsState(),
  ...createKnowledgeState(),
  ...createWeixinRemoteState(),
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
  const currentSessionId = appState.activeSessionId;
  if (sessionId === currentSessionId) {
    const resolvedRoot = resolveWorkspaceRoot(sessionId, workspaceRoot);
    if (resolvedRoot && !sameWorkspaceRoot(resolvedRoot, appState.workspaceRoot)) {
      replaceActiveWorkspace(resolvedRoot);
    }
    return true;
  }

  const currentKey = taskDraftKey(currentSessionId);
  appState.draftsByTask[currentKey] = createTaskDraft(
    appState.composerValue,
    appState.composerContextFiles,
    appState.composerSkills,
    appState.composerMode
  );
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
  const nextDraft = appState.draftsByTask[nextKey];
  appState.composerValue = nextDraft?.content ?? '';
  appState.composerContextFiles = [...(nextDraft?.contextFiles ?? [])];
  appState.composerSkills = [...(nextDraft?.skillNames ?? [])];
  appState.composerMode = nextDraft?.mode === 'deepResearch' ? 'deepResearch' : 'standard';
  appState.modelChangeNotice = null;
  reportTaskPersistenceResult(persistActiveWorkspaceTask());
  return true;
}

export function promoteActiveTask(
  sessionId: string,
  workspaceRoot: string,
  sourceSessionId: string | null = null
): void {
  const sourceKey = taskDraftKey(sourceSessionId);
  const promotedKey = taskDraftKey(sessionId);
  const sourceIsVisible = appState.activeSessionId === sourceSessionId;

  if (!sourceIsVisible) {
    const preparedDraft = appState.draftsByTask[sourceKey];
    if (preparedDraft) appState.draftsByTask[promotedKey] = { ...preparedDraft };
    if (sourceKey !== promotedKey) delete appState.draftsByTask[sourceKey];
    reportTaskPersistenceResult(persistTaskDrafts(appState.draftsByTask));

    const preparedWorkspace = appState.workspaceSnapshotsByTask[sourceKey];
    if (preparedWorkspace) appState.workspaceSnapshotsByTask[promotedKey] = preparedWorkspace;
    if (sourceKey !== promotedKey) delete appState.workspaceSnapshotsByTask[sourceKey];
    if (readActiveTask() === sourceSessionId) {
      reportTaskPersistenceResult(persistActiveTask(sessionId));
    }
    reportTaskPersistenceResult(persistActiveWorkspaceTask());
    return;
  }

  if (sourceSessionId) {
    switchActiveTask(sessionId, workspaceRoot);
    return;
  }
  appState.draftsByTask[promotedKey] = createTaskDraft(
    appState.composerValue,
    appState.composerContextFiles,
    appState.composerSkills,
    appState.composerMode
  );
  delete appState.draftsByTask[sourceKey];
  reportTaskPersistenceResult(persistTaskDrafts(appState.draftsByTask));
  const rootChanged = !sameWorkspaceRoot(appState.workspaceRoot, workspaceRoot);
  if (rootChanged) {
    restoreWorkspaceTaskState(appState, createWorkspaceTaskState(workspaceRoot));
    appState.taskView = 'conversation';
    appState.workspaceGeneration += 1;
  } else {
    appState.workspaceRoot = workspaceRoot;
  }
  delete appState.workspaceSnapshotsByTask[sourceKey];
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
    : '';
  if (sessionWorkspace) return sessionWorkspace;
  const storedWorkspace = appState.workspaceSnapshotsByTask[taskDraftKey(sessionId)]?.state.workspaceRoot;
  return (
    storedWorkspace ||
    appState.workspaceRoot ||
    appState.newTaskConfig.workspace.trim() ||
    appState.health?.workspace.trim() ||
    ''
  );
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

type WorkNavigationHistory = 'push' | 'replace';

export function navigateConversation(
  sessionId: string,
  { history = 'push' }: { history?: WorkNavigationHistory } = {}
): void {
  appState.settingsOpen = false;
  appState.commandPaletteOpen = false;
  appState.fileQuickOpenOpen = false;
  appState.activeProduct = 'work';
  appState.workRoute = 'conversation';
  appState.conversationSessionId = sessionId;
  const hash = conversationHash(sessionId);
  writeRoute(hash, history === 'push' && window.location.hash === hash ? 'replace' : history);
}

export function navigateWorkHome({ history = 'replace' }: { history?: WorkNavigationHistory } = {}): void {
  appState.settingsOpen = false;
  appState.commandPaletteOpen = false;
  appState.fileQuickOpenOpen = false;
  appState.activeProduct = 'work';
  appState.workRoute = 'home';
  appState.conversationSessionId = null;
  writeRoute('#home', history);
}

export function syncShellRouteFromLocation(): ShellLocation {
  const requested = parseShellLocation(window.location.hash);
  const route = requested.valid ? requested : parseShellLocation('#home');
  if (!requested.valid) window.history.replaceState(null, '', '#home');

  if (route.settingsOpen) {
    appState.settingsOpen = true;
    const settingsTab = settingsTabFromHash(window.location.hash);
    const settingsChannel = settingsChannelFromHash(window.location.hash);
    if (settingsTab) appState.settingsTab = settingsTab;
    if (settingsChannel) appState.settingsChannel = settingsChannel;
    return route;
  }

  appState.settingsOpen = false;
  appState.commandPaletteOpen = false;
  appState.fileQuickOpenOpen = false;
  appState.activeProduct = route.activeProduct;
  if (route.activeProduct === 'work') {
    appState.workRoute = route.workRoute;
    appState.conversationSessionId = route.conversationSessionId;
  }
  if (route.activeProduct === 'plugin' && route.pluginKey) appState.activePluginKey = route.pluginKey;
  return route;
}

function writeRoute(hash: string, history: WorkNavigationHistory): void {
  if (history === 'push') window.history.pushState(null, '', hash);
  else window.history.replaceState(null, '', hash);
}

export function navigateTask(view: TaskView): void {
  const previousView = appState.taskView;
  if (previousView === 'conversation' && view !== 'conversation') rememberTaskContextFocus(view);
  if (view === 'conversation') appState.workspacePresentation = 'docked';
  appState.taskView = view;
  if (view === 'conversation' && appState.activeSessionId) {
    navigateConversation(appState.activeSessionId, { history: 'replace' });
  } else {
    navigateWorkHome({ history: 'replace' });
  }
  if (previousView !== 'conversation' && view === 'conversation') restoreTaskContextFocus(previousView);
}

export function navigateMemory(): void {
  appState.settingsOpen = false;
  appState.commandPaletteOpen = false;
  appState.fileQuickOpenOpen = false;
  appState.activeProduct = 'memory';
  window.history.replaceState(null, '', '#memory');
}

export function navigateProduct(product: ProductId): void {
  if (product === 'memory') {
    navigateMemory();
    return;
  }
  if (product === 'plugin') {
    if (appState.activePluginKey) navigatePlugin(appState.activePluginKey);
    else navigateTask('conversation');
    return;
  }
  if (product === 'plugins') {
    navigatePlugins();
    return;
  }
  if (product === 'knowledge') {
    navigateKnowledge();
    return;
  }
  if (product === 'work') {
    navigateWorkHome({ history: 'replace' });
    return;
  }
  appState.settingsOpen = false;
  appState.commandPaletteOpen = false;
  appState.activeProduct = product;
  window.history.replaceState(null, '', '#home');
}

export function navigatePlugin(key: string): void {
  const contribution = appState.pluginCatalog.items.find((item) => item.key === key && item.enabled);
  if (!contribution) {
    navigateTask('conversation');
    return;
  }
  appState.settingsOpen = false;
  appState.commandPaletteOpen = false;
  appState.activeProduct = 'plugin';
  appState.activePluginKey = contribution.key;
  appState.pluginRuntimeError = null;
  window.history.replaceState(null, '', `#plugin/${encodeURIComponent(contribution.key)}`);
}

export function navigatePlugins(): void {
  appState.settingsOpen = false;
  appState.commandPaletteOpen = false;
  appState.activeProduct = 'plugins';
  window.history.replaceState(null, '', '#plugins');
}

export function navigateKnowledge(): void {
  appState.settingsOpen = false;
  appState.commandPaletteOpen = false;
  appState.activeProduct = 'knowledge';
  window.history.replaceState(null, '', '#knowledge');
}

export function navigateKnowledgeBase(id: string, workspace?: string): void {
  appState.requestedKnowledgeBaseId = id;
  const normalizedWorkspace = workspace?.trim();
  if (normalizedWorkspace) {
    appState.knowledgeWorkspace = normalizedWorkspace;
    if (
      appState.personalKnowledgeBases &&
      !sameWorkspaceRoot(appState.personalKnowledgeBases.workspaceRoot, normalizedWorkspace)
    ) {
      appState.personalKnowledgeBases = null;
      appState.knowledgeStatus = 'idle';
      appState.knowledgeError = null;
    }
  }
  navigateKnowledge();
}

export function navigateSettings(tab: SettingsTab): void {
  appState.settingsOpen = true;
  appState.settingsTab = tab;
  window.history.replaceState(null, '', settingsHashForTab(tab, appState.settingsChannel));
}

export function navigateSettingsChannel(channel: ChannelSettingsTab): void {
  appState.settingsOpen = true;
  appState.settingsTab = 'channels';
  appState.settingsChannel = channel;
  window.history.replaceState(null, '', settingsHashForTab('channels', channel));
}

export function closeSettings(): void {
  appState.settingsOpen = false;
  window.history.replaceState(
    null,
    '',
    appState.activeProduct === 'plugin' && appState.activePluginKey
      ? `#plugin/${encodeURIComponent(appState.activePluginKey)}`
      : appState.activeProduct === 'plugins'
        ? '#plugins'
        : appState.activeProduct === 'knowledge'
          ? '#knowledge'
          : appState.activeProduct === 'memory'
            ? '#memory'
            : appState.workRoute === 'conversation' && appState.conversationSessionId
              ? conversationHash(appState.conversationSessionId)
              : '#home'
  );
}

export function clearToast(id: number): void {
  if (appState.toast?.id === id) appState.toast = null;
}

export function formatApiError(error: unknown): string {
  if (!(error instanceof Error)) return '发生了未知错误';
  if (error.message === 'Failed to fetch' || error.message.includes('NetworkError')) return '无法访问本地 A3S 服务。';
  return error.message;
}
