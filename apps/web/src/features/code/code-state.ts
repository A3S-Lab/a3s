import type { HealthResponse } from '../../types/api';

export type ThemePreference = 'system' | 'light' | 'dark';
export type BootPhase = 'loading' | 'ready' | 'error';
export type ServiceStatus = 'connected' | 'checking' | 'disconnected';
export type ProductId = 'work' | 'memory' | 'knowledge' | 'plugin' | 'plugins';
export type TaskView = 'conversation' | 'review' | 'activity';
export type WorkRoute = 'home' | 'conversation';
export interface ToastState {
  id: number;
  tone: 'info' | 'success' | 'error';
  message: string;
}
export interface CodeShellState {
  bootPhase: BootPhase;
  bootError: string | null;
  serviceStatus: ServiceStatus;
  serviceError: string | null;
  health: HealthResponse | null;
  theme: ThemePreference;
  activeProduct: ProductId;
  workRoute: WorkRoute;
  conversationSessionId: string | null;
  sidebarOpen: boolean;
  taskView: TaskView;
  settingsOpen: boolean;
  commandPaletteOpen: boolean;
  fileQuickOpenOpen: boolean;
  toast: ToastState | null;
}

export interface ShellLocation {
  activeProduct: ProductId;
  workRoute: WorkRoute;
  conversationSessionId: string | null;
  pluginKey: string | null;
  settingsOpen: boolean;
  valid: boolean;
}

export function conversationHash(sessionId: string): string {
  if (!sessionId.trim()) throw new Error('Conversation routes require a session id.');
  return `#conversation/${encodeURIComponent(sessionId)}`;
}

export function parseShellLocation(hash: string): ShellLocation {
  const workHome: ShellLocation = {
    activeProduct: 'work',
    workRoute: 'home',
    conversationSessionId: null,
    pluginKey: null,
    settingsOpen: false,
    valid: true,
  };
  if (hash === '#home') return workHome;
  if (hash === '#settings' || hash.startsWith('#settings/')) {
    return { ...workHome, settingsOpen: true };
  }
  if (hash === '#memory') return { ...workHome, activeProduct: 'memory' };
  if (hash === '#knowledge') return { ...workHome, activeProduct: 'knowledge' };
  if (hash === '#plugins') return { ...workHome, activeProduct: 'plugins' };
  if (hash.startsWith('#plugin/')) {
    const pluginKey = decodeRouteValue(hash.slice('#plugin/'.length));
    return pluginKey ? { ...workHome, activeProduct: 'plugin', pluginKey } : { ...workHome, valid: false };
  }
  if (hash.startsWith('#conversation/')) {
    const conversationSessionId = decodeRouteValue(hash.slice('#conversation/'.length));
    return conversationSessionId
      ? { ...workHome, workRoute: 'conversation', conversationSessionId }
      : { ...workHome, valid: false };
  }
  return { ...workHome, valid: false };
}

function decodeRouteValue(value: string): string | null {
  if (!value) return null;
  try {
    const decoded = decodeURIComponent(value);
    return decoded.trim() ? decoded : null;
  } catch {
    return null;
  }
}

function readTheme(): ThemePreference {
  try {
    const value = localStorage.getItem('a3s-code-web.theme');
    return value === 'light' || value === 'dark' || value === 'system' ? value : 'system';
  } catch {
    return 'system';
  }
}
export function createCodeShellState(): CodeShellState {
  const location = parseShellLocation(window.location.hash);
  if (!location.valid) window.history.replaceState(null, '', '#home');
  return {
    bootPhase: 'loading',
    bootError: null,
    serviceStatus: 'checking',
    serviceError: null,
    health: null,
    theme: readTheme(),
    activeProduct: location.activeProduct,
    workRoute: location.workRoute,
    conversationSessionId: location.conversationSessionId,
    sidebarOpen: true,
    taskView: 'conversation',
    settingsOpen: location.settingsOpen,
    commandPaletteOpen: false,
    fileQuickOpenOpen: false,
    toast: null,
  };
}
