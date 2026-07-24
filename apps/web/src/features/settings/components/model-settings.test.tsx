import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../../state/app-state';
import type { LlmSettings } from '../../../types/api';
import type { SettingsActions } from '../settings-actions';
import { ModelSettings } from './model-settings';

const source: LlmSettings = {
  category: 'llm',
  effect: { scope: 'newTasks', label: 'New tasks', description: 'Applies to new tasks' },
  configPath: '/repo/.a3s/config.acl',
  defaultModel: 'openai/model-a',
  providers: [
    {
      name: 'openai',
      apiKey: null,
      baseUrl: null,
      headers: {},
      sessionIdHeader: null,
      models: [{ id: 'model-a', name: 'Model A' }],
    },
  ],
};

describe('ModelSettings editing continuity', () => {
  const actions = {
    loadSettingsCategory: vi.fn(async () => undefined),
    saveLlmSettings: vi.fn(async (draft) => ({ ...structuredClone(source), ...draft })),
  } as unknown as SettingsActions;

  beforeEach(() => {
    vi.clearAllMocks();
    appState.llm = structuredClone(source);
    appState.settingsCategoryLoading.llm = false;
    appState.settingsCategorySaving.llm = false;
    appState.settingsCategoryErrors.llm = null;
    appState.settingsCategorySavedAt.llm = null;
  });

  afterEach(() => cleanup());

  it('keeps focus while the Provider name changes', () => {
    render(<ModelSettings actions={actions} />);
    const input = screen.getByRole('textbox', { name: 'openai 名称' });
    input.focus();

    fireEvent.change(input, { target: { value: 'renamed-provider' } });

    expect(screen.getByRole('textbox', { name: 'renamed-provider 名称' })).toHaveFocus();
  });

  it('keeps focus while the model id changes', () => {
    render(<ModelSettings actions={actions} />);
    fireEvent.click(screen.getByRole('button', { name: '编辑模型 openai/model-a' }));
    const input = screen.getByRole('textbox', { name: 'openai/model-a 模型标识' });
    input.focus();

    fireEvent.change(input, { target: { value: 'model-b' } });

    const updatedInput = screen.getByRole('textbox', { name: 'openai/model-b 模型标识' });
    expect(updatedInput).toHaveFocus();
  });

  it('keeps the default-model picker compact and flat', () => {
    render(<ModelSettings actions={actions} />);

    fireEvent.click(screen.getByRole('button', { name: /设置默认模型/ }));

    expect(screen.getByRole('option', { name: /Model A/ })).toHaveAttribute('aria-selected', 'true');
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
  });

  it('selects a newly added Provider for immediate editing', () => {
    render(<ModelSettings actions={actions} />);

    fireEvent.click(screen.getByRole('button', { name: '添加 Provider' }));

    expect(screen.getByRole('button', { name: '编辑 Provider new-provider' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('textbox', { name: 'new-provider 名称' })).toHaveValue('new-provider');
  });

  it('adds a model through the focused model dialog', () => {
    render(<ModelSettings actions={actions} />);

    fireEvent.click(screen.getByRole('button', { name: '添加模型' }));

    expect(screen.getByRole('dialog', { name: '为 openai 添加模型' })).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: 'openai/new-model 模型标识' })).toHaveValue('new-model');
    fireEvent.click(screen.getByRole('button', { name: '保存模型' }));

    expect(screen.queryByRole('dialog', { name: '为 openai 添加模型' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '编辑模型 openai/new-model' })).toBeInTheDocument();
  });

  it('shows connection and models together while keeping advanced settings collapsed', () => {
    render(<ModelSettings actions={actions} />);

    const runtimeDetails = screen.getByText('高级运行参数').closest('details');
    expect(runtimeDetails).not.toHaveAttribute('open');
    expect(screen.getByText('Provider 与模型').compareDocumentPosition(screen.getByText('高级运行参数'))).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(screen.getByRole('textbox', { name: 'openai 名称' })).toBeVisible();
    expect(screen.getByRole('table')).toBeVisible();
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '编辑模型 openai/model-a' }));

    const limitsDetails = screen.getByText('能力与限额').closest('details');
    const connectionOverrideDetails = screen.getByText('模型级连接覆盖').closest('details');
    expect(limitsDetails).not.toHaveAttribute('open');
    expect(connectionOverrideDetails).not.toHaveAttribute('open');
    expect(screen.getByRole('textbox', { name: 'openai/model-a 模型标识' })).toBeVisible();
  });

  it('lets the user undo an unsaved category draft', () => {
    render(<ModelSettings actions={actions} />);
    fireEvent.change(screen.getByRole('textbox', { name: 'openai 名称' }), {
      target: { value: 'renamed-provider' },
    });

    fireEvent.click(screen.getByRole('button', { name: '撤销' }));

    expect(screen.getByRole('textbox', { name: 'openai 名称' })).toHaveValue('openai');
    expect(screen.queryByRole('button', { name: '撤销' })).not.toBeInTheDocument();
  });
});
