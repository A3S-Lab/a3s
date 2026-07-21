import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createWorkArtifact } from '../work-templates';
import { WorkHome } from './work-home';

describe('Work file center', () => {
  afterEach(cleanup);

  it('starts every Office artifact type from the home surface', () => {
    const create = vi.fn();
    render(
      <WorkHome
        artifacts={[]}
        folders={[]}
        view='home'
        activeFolderId={null}
        loading={false}
        error={null}
        onCreate={create}
        onOpen={vi.fn()}
        onImport={vi.fn()}
        onToggleFavorite={vi.fn()}
        onRename={vi.fn()}
        onCopy={vi.fn()}
        onMove={vi.fn()}
        onRestore={vi.fn()}
        onDelete={vi.fn()}
        onOpenFolder={vi.fn()}
        onCreateFolder={vi.fn()}
        onRenameFolder={vi.fn()}
        onRestoreFolder={vi.fn()}
        onDeleteFolder={vi.fn()}
        onRetry={vi.fn()}
      />
    );

    expect(screen.getByRole('button', { name: /空白文字/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /空白表格/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /空白演示/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /季度计划/ }));
    expect(create).toHaveBeenCalledWith('quarterly-plan');
  });

  it('searches and opens persisted artifacts', () => {
    const open = vi.fn();
    const document = createWorkArtifact('project-brief');
    const spreadsheet = createWorkArtifact('quarterly-plan');
    render(
      <WorkHome
        artifacts={[document, spreadsheet]}
        folders={[]}
        view='recent'
        activeFolderId={null}
        loading={false}
        error={null}
        onCreate={vi.fn()}
        onOpen={open}
        onImport={vi.fn()}
        onToggleFavorite={vi.fn()}
        onRename={vi.fn()}
        onCopy={vi.fn()}
        onMove={vi.fn()}
        onRestore={vi.fn()}
        onDelete={vi.fn()}
        onOpenFolder={vi.fn()}
        onCreateFolder={vi.fn()}
        onRenameFolder={vi.fn()}
        onRestoreFolder={vi.fn()}
        onDeleteFolder={vi.fn()}
        onRetry={vi.fn()}
      />
    );

    fireEvent.change(screen.getByRole('searchbox', { name: '搜索 Work 文件' }), {
      target: { value: '季度' },
    });
    expect(screen.queryByRole('button', { name: `打开 ${document.title}` })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: `打开 ${spreadsheet.title}` }));
    expect(open).toHaveBeenCalledWith(spreadsheet.id);
  });

  it('separates trashed files and exposes explicit recovery', () => {
    const restore = vi.fn();
    const active = createWorkArtifact('project-brief');
    const trashed = { ...createWorkArtifact('quarterly-plan'), trashedAt: Date.now() };
    render(
      <WorkHome
        artifacts={[active, trashed]}
        folders={[]}
        view='trash'
        activeFolderId={null}
        loading={false}
        error={null}
        onCreate={vi.fn()}
        onOpen={vi.fn()}
        onImport={vi.fn()}
        onToggleFavorite={vi.fn()}
        onRename={vi.fn()}
        onCopy={vi.fn()}
        onMove={vi.fn()}
        onRestore={restore}
        onDelete={vi.fn()}
        onOpenFolder={vi.fn()}
        onCreateFolder={vi.fn()}
        onRenameFolder={vi.fn()}
        onRestoreFolder={vi.fn()}
        onDeleteFolder={vi.fn()}
        onRetry={vi.fn()}
      />
    );

    expect(screen.queryByRole('button', { name: `打开 ${active.title}` })).not.toBeInTheDocument();
    fireEvent.click(screen.getByLabelText(`${trashed.title} 更多操作`));
    fireEvent.click(screen.getByRole('button', { name: '恢复' }));
    expect(restore).toHaveBeenCalledWith(trashed.id);
  });
});
