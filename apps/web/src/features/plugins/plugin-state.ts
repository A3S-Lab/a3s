import type {
  PluginActivityCatalog,
  PluginActivityContent,
  PluginMarketplaceCatalog,
  PluginOperationPlan,
  PluginOperationRequest,
} from '../../types/api';

export type PluginLoadStatus = 'idle' | 'loading' | 'ready' | 'error';

export interface PluginContextField {
  label: string;
  value: string;
}

export interface PluginContextProposal {
  sourceKey: string;
  title: string;
  summary: string;
  prompt: string;
  fields: PluginContextField[];
  usePackageSkill: boolean;
}

export interface PluginOperationReview {
  request: PluginOperationRequest;
  plan: PluginOperationPlan;
}

export interface PluginsState {
  pluginCatalog: PluginActivityCatalog;
  pluginCatalogStatus: PluginLoadStatus;
  pluginCatalogError: string | null;
  activePluginKey: string | null;
  pluginContentByKey: Record<string, PluginActivityContent>;
  pluginContentStatus: PluginLoadStatus;
  pluginContentError: string | null;
  pluginRuntimeError: string | null;
  pluginContextProposal: PluginContextProposal | null;
  pluginMarketplace: PluginMarketplaceCatalog | null;
  pluginMarketplaceStatus: PluginLoadStatus;
  pluginMarketplaceError: string | null;
  pluginOperationReview: PluginOperationReview | null;
  pluginOperationStatus: PluginLoadStatus;
  pluginOperationError: string | null;
}

function activePluginKeyFromHash(): string | null {
  const encoded = window.location.hash.match(/^#plugin\/([^/]+)$/)?.[1];
  if (!encoded) return null;
  try {
    return decodeURIComponent(encoded);
  } catch {
    return null;
  }
}

export function createPluginsState(): PluginsState {
  return {
    pluginCatalog: {
      schemaVersion: 1,
      available: false,
      generation: 0,
      revision: '',
      items: [],
    },
    pluginCatalogStatus: 'idle',
    pluginCatalogError: null,
    activePluginKey: activePluginKeyFromHash(),
    pluginContentByKey: {},
    pluginContentStatus: 'idle',
    pluginContentError: null,
    pluginRuntimeError: null,
    pluginContextProposal: null,
    pluginMarketplace: null,
    pluginMarketplaceStatus: 'idle',
    pluginMarketplaceError: null,
    pluginOperationReview: null,
    pluginOperationStatus: 'idle',
    pluginOperationError: null,
  };
}
