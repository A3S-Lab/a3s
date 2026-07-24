import { useMemoizedFn } from 'ahooks';
import { ApiError, codeApi } from '../../lib/api';
import { appState, formatApiError, showToast } from '../../state/app-state';
import type { LlmSettings } from '../../types/api';
import type { AgentSettings, ConfigCategory, ContextSettings, IntegrationsSettings } from '../../types/settings';
import { fallbackModelCatalog } from '../code/use-app-bootstrap';
import type { UpdateStatus } from './settings-state';

export function useSettingsController() {
  const loadSettingsCategory = useMemoizedFn(async (category: ConfigCategory, force = false) => {
    if (appState.settingsCategoryLoading[category]) return;
    if (!force && categoryData(category)) return;
    appState.settingsCategoryLoading[category] = true;
    appState.settingsCategoryErrors[category] = null;
    try {
      if (category === 'llm') appState.llm = await codeApi.llmSettings();
      else if (category === 'agent') appState.agentSettings = await codeApi.agentSettings();
      else if (category === 'context') appState.contextSettings = await codeApi.contextSettings();
      else appState.integrationsSettings = await codeApi.integrationsSettings();
    } catch (error) {
      appState.settingsCategoryErrors[category] = formatApiError(error);
    } finally {
      appState.settingsCategoryLoading[category] = false;
    }
  });
  const saveLlmSettings = useMemoizedFn(async (patch: Partial<LlmSettings>) => {
    if (appState.settingsCategorySaving.llm) throw new Error('模型设置正在保存');
    appState.settingsCategorySaving.llm = true;
    appState.settingsCategoryErrors.llm = null;
    try {
      const settings = await codeApi.updateLlmSettings(patch);
      appState.llm = settings;
      appState.selectedModel = settings.defaultModel;
      appState.modelCatalog = await codeApi.modelCatalog().catch(() => fallbackModelCatalog(settings));
      appState.settingsCategorySavedAt.llm = Date.now();
      return settings;
    } catch (error) {
      const message = formatApiError(error);
      appState.settingsCategoryErrors.llm = message;
      showToast(message, 'error');
      throw error;
    } finally {
      appState.settingsCategorySaving.llm = false;
    }
  });
  const saveAgentSettings = useMemoizedFn(async (patch: Partial<AgentSettings>) => {
    if (appState.settingsCategorySaving.agent) throw new Error('Agent 设置正在保存');
    appState.settingsCategorySaving.agent = true;
    appState.settingsCategoryErrors.agent = null;
    try {
      const settings = await codeApi.updateAgentSettings(patch);
      appState.agentSettings = settings;
      appState.settingsCategorySavedAt.agent = Date.now();
      return settings;
    } catch (error) {
      const message = formatApiError(error);
      appState.settingsCategoryErrors.agent = message;
      showToast(message, 'error');
      throw error;
    } finally {
      appState.settingsCategorySaving.agent = false;
    }
  });
  const saveContextSettings = useMemoizedFn(async (patch: Partial<ContextSettings>) => {
    if (appState.settingsCategorySaving.context) throw new Error('上下文设置正在保存');
    appState.settingsCategorySaving.context = true;
    appState.settingsCategoryErrors.context = null;
    try {
      const settings = await codeApi.updateContextSettings(patch);
      appState.contextSettings = settings;
      appState.settingsCategorySavedAt.context = Date.now();
      return settings;
    } catch (error) {
      const message = formatApiError(error);
      appState.settingsCategoryErrors.context = message;
      showToast(message, 'error');
      throw error;
    } finally {
      appState.settingsCategorySaving.context = false;
    }
  });
  const saveIntegrationsSettings = useMemoizedFn(async (patch: Partial<IntegrationsSettings>) => {
    if (appState.settingsCategorySaving.integrations) throw new Error('集成设置正在保存');
    appState.settingsCategorySaving.integrations = true;
    appState.settingsCategoryErrors.integrations = null;
    try {
      const settings = await codeApi.updateIntegrationsSettings(patch);
      appState.integrationsSettings = settings;
      appState.settingsCategorySavedAt.integrations = Date.now();
      return settings;
    } catch (error) {
      const message = formatApiError(error);
      appState.settingsCategoryErrors.integrations = message;
      showToast(message, 'error');
      throw error;
    } finally {
      appState.settingsCategorySaving.integrations = false;
    }
  });
  const updateDefaultModel = useMemoizedFn(async (model: string) => {
    if (appState.defaultModelSaving || model === appState.llm?.defaultModel) return;
    appState.defaultModelSaving = true;
    try {
      await saveLlmSettings({ defaultModel: model });
    } catch {
      // saveLlmSettings keeps the authoritative value and reports the error.
    } finally {
      appState.defaultModelSaving = false;
    }
  });
  const refreshModelCatalog = useMemoizedFn(async () => {
    if (appState.modelCatalogRefreshing) return;
    appState.modelCatalogRefreshing = true;
    appState.modelCatalogRefreshError = null;
    try {
      const catalog = await codeApi.refreshModelCatalog();
      appState.modelCatalog = catalog;
      const fallbackModel = catalog.defaultModel || appState.llm?.defaultModel || '';
      if (!catalog.items.some((model) => model.id === appState.selectedModel)) {
        appState.selectedModel = fallbackModel;
      }
      if (!catalog.items.some((model) => model.id === appState.newTaskConfig.model)) {
        appState.newTaskConfig.model = fallbackModel;
      }
      appState.modelCatalogRefreshedAt = Date.now();
    } catch (error) {
      appState.modelCatalogRefreshError = formatApiError(error);
    } finally {
      appState.modelCatalogRefreshing = false;
    }
  });
  const loginWithOs = useMemoizedFn(async () => {
    try {
      appState.osAccount = await codeApi.osLogin();
      showToast('A3S OS 授权成功', 'success');
    } catch (error) {
      showToast(formatApiError(error), 'error');
    }
  });
  const logout = useMemoizedFn(async () => {
    try {
      appState.osAccount = await codeApi.osLogout();
      showToast('已退出 A3S OS，本地任务保持打开', 'success');
    } catch (error) {
      showToast(formatApiError(error), 'error');
      throw error;
    }
  });
  const checkForUpdates = useMemoizedFn(async () => {
    if (appState.updateChecking || appState.updateInstalling) return;
    appState.updateChecking = true;
    appState.updateCheckError = null;
    appState.updateInstallError = null;
    try {
      appState.updateStatus = (await codeApi.updateStatus()) as UpdateStatus;
      if (!appState.updateStatus.updateAvailable) appState.updateInstalledVersion = null;
    } catch (error) {
      const message = formatUpdateCheckError(error);
      appState.updateStatus = null;
      appState.updateCheckError = message;
    } finally {
      appState.updateChecking = false;
    }
  });
  const installUpdate = useMemoizedFn(async (version: string) => {
    if (!version || appState.updateInstalling) return;
    appState.updateInstalling = true;
    appState.updateInstallError = null;
    let result: { restartRequired: boolean; message: string };
    try {
      result = (await codeApi.installUpdate(version)) as { restartRequired: boolean; message: string };
    } catch (error) {
      const message = formatApiError(error);
      appState.updateInstallError = message;
      appState.updateInstalling = false;
      showToast(message, 'error');
      throw error;
    }
    appState.updateInstalledVersion = version;
    showToast(result.restartRequired ? '更新已安装，请重启 A3S Code Web 服务' : result.message, 'success');
    try {
      appState.updateStatus = (await codeApi.updateStatus()) as UpdateStatus;
      if (!appState.updateStatus.updateAvailable) appState.updateInstalledVersion = null;
    } catch (error) {
      appState.updateCheckError = `更新已安装，但无法刷新版本状态：${formatApiError(error)}`;
    } finally {
      appState.updateInstalling = false;
    }
  });
  return {
    loadSettingsCategory,
    updateDefaultModel,
    saveLlmSettings,
    saveAgentSettings,
    saveContextSettings,
    saveIntegrationsSettings,
    refreshModelCatalog,
    loginWithOs,
    logout,
    checkForUpdates,
    installUpdate,
  };
}

function formatUpdateCheckError(error: unknown): string {
  if (error instanceof ApiError && error.status === 404) {
    return '当前 A3S Boot 版本尚未提供在线更新检查，请使用原安装渠道升级。';
  }
  return formatApiError(error);
}

function categoryData(category: ConfigCategory) {
  if (category === 'llm') return appState.llm;
  if (category === 'agent') return appState.agentSettings;
  if (category === 'context') return appState.contextSettings;
  return appState.integrationsSettings;
}
