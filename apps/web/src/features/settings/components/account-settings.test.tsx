import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../../state/app-state';
import type { SettingsActions } from '../settings-actions';
import { AccountSettings } from './account-settings';

describe('AccountSettings', () => {
  const actions = {
    refreshModelCatalog: vi.fn(async () => undefined),
    loginWithOs: vi.fn(async () => undefined),
    logout: vi.fn(async () => undefined),
  } as unknown as SettingsActions;

  beforeEach(() => {
    vi.clearAllMocks();
    appState.health = {
      ok: true,
      app: 'A3S Code',
      version: '0.7.7',
      configPath: '/repo/config.acl',
      workspace: '/repo',
    };
    appState.osAccount = {
      configured: false,
      signedIn: false,
      needsRefresh: false,
      capabilitySkillActive: false,
      builtinSkillActive: false,
      runtimeToolActive: false,
    };
    appState.modelCatalogRefreshing = false;
    appState.modelCatalogRefreshError = null;
    appState.modelCatalogRefreshedAt = null;
    appState.modelCatalog = {
      defaultModel: 'codex/gpt-5.4',
      warnings: [],
      items: [
        {
          id: 'anthropic/claude-opus',
          name: 'claude-opus',
          source: 'anthropic',
          reasoning: false,
          toolCall: true,
        },
        {
          id: 'codex/gpt-5.4',
          name: 'gpt-5.4',
          source: 'Codex',
          reasoning: true,
          toolCall: true,
        },
        {
          id: 'workbuddy/auto',
          name: 'auto',
          source: 'WorkBuddy',
          reasoning: true,
          toolCall: true,
        },
        {
          id: 'workbuddy/glm-5.1',
          name: 'glm-5.1',
          source: 'WorkBuddy',
          reasoning: true,
          toolCall: true,
        },
      ],
    };
  });

  afterEach(() => cleanup());

  it('separates configured Providers from actually signed-in local tool accounts', () => {
    render(<AccountSettings actions={actions} />);

    const claude = screen.getByRole('listitem', { name: 'Claude Code 账户状态' });
    const codex = screen.getByRole('listitem', { name: 'Codex 账户状态' });
    const workbuddy = screen.getByRole('listitem', { name: 'WorkBuddy 账户状态' });

    expect(within(claude).getByText('未连接')).toBeInTheDocument();
    expect(within(claude).getByText('claude auth login')).toBeInTheDocument();
    expect(within(codex).getByText(/1 个可用模型/)).toBeInTheDocument();
    expect(within(codex).getByText('已连接')).toBeInTheDocument();
    expect(within(workbuddy).getByText(/2 个可用模型/)).toBeInTheDocument();
  });

  it('exposes one explicit refresh action for all local account models', () => {
    render(<AccountSettings actions={actions} />);

    fireEvent.click(screen.getByRole('button', { name: '刷新账户模型' }));

    expect(actions.refreshModelCatalog).toHaveBeenCalledTimes(1);
  });
});
