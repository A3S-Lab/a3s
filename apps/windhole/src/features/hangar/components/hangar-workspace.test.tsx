import { fireEvent, render, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { createHangarDraft, DEFAULT_HANGAR_ROSTER } from '../hangar-configuration';
import { HangarWorkspace } from './hangar-workspace';

describe('HangarWorkspace', () => {
  it('presents a game-style 3D combination flow and deploys the active squadron', () => {
    const onDraftChange = vi.fn();
    const onAddToRoster = vi.fn();
    const onSelectRoster = vi.fn();
    const onRemoveFromRoster = vi.fn();
    const onDeploy = vi.fn();
    const draft = createHangarDraft('a3s');
    const { container } = render(
      <HangarWorkspace
        draft={draft}
        roster={DEFAULT_HANGAR_ROSTER}
        selectedRosterId={DEFAULT_HANGAR_ROSTER[0].id}
        preview={<div data-testid='three-preview' />}
        onDraftChange={onDraftChange}
        onAddToRoster={onAddToRoster}
        onSelectRoster={onSelectRoster}
        onRemoveFromRoster={onRemoveFromRoster}
        onDeploy={onDeploy}
      />
    );
    const view = within(container);

    expect(view.getByTestId('three-preview')).toBeInTheDocument();
    expect(view.getByRole('heading', { name: '智能体机库' })).toBeInTheDocument();
    expect(view.getAllByRole('button', { name: /^选择 (J-50|J-35|F-35|F-22|Generic)/ })).toHaveLength(5);
    expect(
      within(view.getByRole('button', { name: '选择 Codex Systems Pilot' })).getByText('需配置本地或 OCI Adapter')
    ).toBeInTheDocument();
    expect(
      within(view.getByRole('button', { name: '选择 Claude Test Pilot' })).getByText('需配置本地或 OCI Adapter')
    ).toBeInTheDocument();

    fireEvent.click(view.getByRole('button', { name: '选择 F-35 Lightning II，隐身打击' }));
    expect(onDraftChange).toHaveBeenCalledWith(expect.objectContaining({ airframeId: 'f-35' }));

    fireEvent.click(view.getByRole('button', { name: '加入新组合' }));
    expect(onAddToRoster).toHaveBeenCalledWith(draft);

    fireEvent.click(view.getByRole('button', { name: /进入作战地图/ }));
    expect(onDeploy).toHaveBeenCalledOnce();
    expect(container).not.toHaveTextContent(/wind\s*tunnel|风洞|active\s*test/i);
  });

  it('keeps an unsaved preview from entering the map', () => {
    const onDeploy = vi.fn();
    const draft = createHangarDraft('a3s');
    const { getByRole } = render(
      <HangarWorkspace
        draft={draft}
        roster={DEFAULT_HANGAR_ROSTER}
        selectedRosterId={DEFAULT_HANGAR_ROSTER[0].id}
        onDraftChange={vi.fn()}
        onAddToRoster={vi.fn()}
        onSelectRoster={vi.fn()}
        onRemoveFromRoster={vi.fn()}
        onDeploy={onDeploy}
        deployReady={false}
      />
    );

    const deployButton = getByRole('button', { name: /组合尚未保存/ });
    expect(deployButton).toBeDisabled();
    fireEvent.click(deployButton);
    expect(onDeploy).not.toHaveBeenCalled();
  });

  it('requires a supported Candidate reference before adding a new combination', () => {
    const onAddToRoster = vi.fn();
    const draft = createHangarDraft('codex');
    const { getByRole, getAllByText } = render(
      <HangarWorkspace
        draft={draft}
        roster={DEFAULT_HANGAR_ROSTER}
        onDraftChange={vi.fn()}
        onAddToRoster={onAddToRoster}
        onSelectRoster={vi.fn()}
        onRemoveFromRoster={vi.fn()}
      />
    );

    expect(getAllByText(/需配置 Candidate Adapter/).length).toBeGreaterThan(0);
    const addButton = getByRole('button', { name: '先配置 Candidate Adapter' });
    expect(addButton).toBeDisabled();
    fireEvent.click(addButton);
    expect(onAddToRoster).not.toHaveBeenCalled();
  });

  it('keeps the squadron and the relevant save action in the persistent dock', () => {
    const onUpdateRoster = vi.fn();
    const draft = createHangarDraft('a3s');
    const { container } = render(
      <HangarWorkspace
        draft={draft}
        roster={DEFAULT_HANGAR_ROSTER}
        selectedRosterId={DEFAULT_HANGAR_ROSTER[0].id}
        onDraftChange={vi.fn()}
        onAddToRoster={vi.fn()}
        onSelectRoster={vi.fn()}
        onRemoveFromRoster={vi.fn()}
        onUpdateRoster={onUpdateRoster}
        updateReady
      />
    );

    const dock = container.querySelector('.hangar-workspace__squadron-dock');
    expect(dock).not.toBeNull();
    const dockView = within(dock as HTMLElement);
    expect(dockView.getAllByRole('button', { name: /^选择编队成员/ })).toHaveLength(DEFAULT_HANGAR_ROSTER.length);
    expect(dockView.getByRole('button', { name: '更新当前组合' })).toHaveClass('is-primary');
    expect(dockView.getByRole('button', { name: '加入新组合' })).toHaveClass('is-secondary');

    fireEvent.click(dockView.getByRole('button', { name: '更新当前组合' }));
    expect(onUpdateRoster).toHaveBeenCalledOnce();
  });

  it('does not describe an invalid unsaved draft as synchronized', () => {
    const draft = createHangarDraft('generic');
    const { getByText, queryByText } = render(
      <HangarWorkspace
        draft={draft}
        roster={DEFAULT_HANGAR_ROSTER}
        selectedRosterId={DEFAULT_HANGAR_ROSTER[0].id}
        onDraftChange={vi.fn()}
        onAddToRoster={vi.fn()}
        onSelectRoster={vi.fn()}
        onRemoveFromRoster={vi.fn()}
        onUpdateRoster={vi.fn()}
        updateReady
      />
    );

    expect(getByText('先完善当前组合').closest('button')).toBeDisabled();
    expect(queryByText('当前组合已同步')).not.toBeInTheDocument();
  });
});
