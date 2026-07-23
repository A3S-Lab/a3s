import { proxy } from 'valtio';
import {
  type CodeShellState,
  createCodeShellState,
  type ProductId,
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
  settingsHashForTab,
} from '../features/settings/settings-state';
import {
  createTaskState,
  createTaskDraft,
  persistActiveTask,
  persistTaskDrafts,
  readActiveTask,
  type TaskProduct,
  type TaskState,
  taskDraftKey,
} from '../features/tasks/task-state';
import { createWeixinRemoteState, type WeixinRemoteState } from '../features/weixin-remote/weixin-remote-state';
import { createWorkspaceState, type WorkspaceState } from '../features/workspace/workspace-state';
import type { CodeSession } from '../types/api';

export type { ProductId, TaskView, ThemePreference } from '../features/code/code-state';

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
const initialTaskProduct: TaskProduct = initialShellState.activeProduct === 'work' ? 'work' : 'code';

export const appState = proxy<AppState>({
  ...initialShellState,
  ...createTaskState(initialTaskProduct),
  ...createWorkspaceState(),
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

export function switchActiveTask(sessionId: string | null): void {
  const product = activeTaskProduct();
  const currentKey = taskDraftKey(appState.activeSessionId, product);
  appState.draftsByTask[currentKey] = createTaskDraft(
    appState.composerValue,
    appState.composerContextFiles,
    appState.composerSkills,
    appState.composerMode
  );
  reportTaskPersistenceResult(persistTaskDrafts(appState.draftsByTask));
  appState.activeSessionId = sessionId;
  reportTaskPersistenceResult(persistActiveTask(sessionId, product));
  const next = appState.draftsByTask[taskDraftKey(sessionId, product)];
  appState.composerValue = next?.content ?? '';
  appState.composerContextFiles = [...(next?.contextFiles ?? [])];
  appState.composerSkills = [...(next?.skillNames ?? [])];
  appState.composerMode = next?.mode === 'deepResearch' && product === 'code' ? 'deepResearch' : 'standard';
  appState.modelChangeNotice = null;
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
  activateTaskProduct('code');
  appState.settingsOpen = false;
  appState.activeProduct = 'code';
  appState.codeSurface = 'tasks';
  appState.taskView = view;
  window.history.replaceState(null, '', `#code/${view}`);
}

export function navigateMemory(): void {
  activateTaskProduct('code');
  appState.settingsOpen = false;
  appState.activeProduct = 'code';
  appState.codeSurface = 'memory';
  window.history.replaceState(null, '', '#code/memory');
}

export function navigateProduct(product: ProductId): void {
  if (product === 'code') {
    navigateTask(appState.taskView);
    return;
  }
  if (product === 'work') activateTaskProduct('work');
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
  appState.settingsOpen = false;
  appState.commandPaletteOpen = false;
  appState.activeProduct = product;
  window.history.replaceState(null, '', '#work/home');
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

function activeTaskProduct(): TaskProduct {
  return appState.activeProduct === 'work' ? 'work' : 'code';
}

function activateTaskProduct(product: TaskProduct): void {
  const currentProduct = activeTaskProduct();
  if (currentProduct === product) return;
  const currentKey = taskDraftKey(appState.activeSessionId, currentProduct);
  appState.draftsByTask[currentKey] = createTaskDraft(
    appState.composerValue,
    appState.composerContextFiles,
    appState.composerSkills,
    appState.composerMode
  );
  reportTaskPersistenceResult(persistTaskDrafts(appState.draftsByTask));
  reportTaskPersistenceResult(persistActiveTask(appState.activeSessionId, currentProduct));

  const sessionId = readActiveTask(product);
  const draft = appState.draftsByTask[taskDraftKey(sessionId, product)];
  appState.activeSessionId = sessionId;
  appState.composerValue = draft?.content ?? '';
  appState.composerContextFiles = [...(draft?.contextFiles ?? [])];
  appState.composerSkills = [...(draft?.skillNames ?? [])];
  appState.composerMode = draft?.mode === 'deepResearch' && product === 'code' ? 'deepResearch' : 'standard';
  appState.streamEvents = [];
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
          : appState.activeProduct === 'work'
            ? '#work/home'
            : appState.codeSurface === 'memory'
              ? '#code/memory'
              : `#code/${appState.taskView}`
  );
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
