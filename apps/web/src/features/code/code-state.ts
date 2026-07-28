import type { HealthResponse } from '../../types/api';

export type ThemePreference = 'system' | 'light' | 'dark';
export type BootPhase = 'loading' | 'ready' | 'error';
export type ServiceStatus = 'connected' | 'checking' | 'disconnected';
export type ProductId = 'work' | 'memory' | 'knowledge' | 'plugin' | 'plugins';
export type TaskView = 'conversation' | 'review' | 'activity';
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
  sidebarOpen: boolean;
  taskView: TaskView;
  settingsOpen: boolean;
  commandPaletteOpen: boolean;
  fileQuickOpenOpen: boolean;
  toast: ToastState | null;
}

function readActiveProduct(): ProductId {
  if (window.location.hash.startsWith('#plugin/')) return 'plugin';
  if (window.location.hash === '#plugins') return 'plugins';
  if (window.location.hash === '#knowledge') return 'knowledge';
  if (window.location.hash === '#memory') return 'memory';
  if (
    window.location.hash !== '#home' &&
    window.location.hash !== '#settings' &&
    !window.location.hash.startsWith('#settings/')
  ) {
    window.history.replaceState(null, '', '#home');
  }
  return 'work';
}
function readTheme(): ThemePreference {
  try {
    const value = localStorage.getItem('a3s-code-web.theme');
    return value === 'light' || value === 'dark' || value === 'system' ? value : 'system';
  } catch {
    return 'system';
  }
}
function readSettingsOpen() {
  return window.location.hash === '#settings' || window.location.hash.startsWith('#settings/');
}
export function createCodeShellState(): CodeShellState {
  return {
    bootPhase: 'loading',
    bootError: null,
    serviceStatus: 'checking',
    serviceError: null,
    health: null,
    theme: readTheme(),
    activeProduct: readActiveProduct(),
    sidebarOpen: true,
    taskView: 'conversation',
    settingsOpen: readSettingsOpen(),
    commandPaletteOpen: false,
    fileQuickOpenOpen: false,
    toast: null,
  };
}
