import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { WorkSidebar } from './work-sidebar';

describe('Work Finder sidebar', () => {
  it('opens, removes, and accepts drops on local favorite folders', () => {
    const onOpenLocalFavorite = vi.fn();
    const onRemoveLocalFavorite = vi.fn();
    const onMoveLocalEntries = vi.fn();
    const onImportLocalDrop = vi.fn();
    const onPickWorkspace = vi.fn();
    render(
      <WorkSidebar
        surface='files'
        localRootName='docs'
        localRootPath='/docs'
        localCurrentPath='/docs/Reports'
        localFavoritePaths={['/docs/Reports']}
        view='home'
        totalCount={0}
        favoriteCount={0}
        trashCount={0}
        folders={[]}
        activeFolderId={null}
        onChangeView={vi.fn()}
        onOpenFolder={vi.fn()}
        onOpenLocalFiles={vi.fn()}
        onOpenLocalFavorite={onOpenLocalFavorite}
        onRemoveLocalFavorite={onRemoveLocalFavorite}
        onMoveLocalEntries={onMoveLocalEntries}
        onImportLocalDrop={onImportLocalDrop}
        onPickWorkspace={onPickWorkspace}
        onCreate={vi.fn()}
        onImport={vi.fn()}
      />
    );
    fireEvent.click(screen.getByRole('button', { name: '切换工作区' }));
    expect(onPickWorkspace).toHaveBeenCalled();
    const dataTransfer = {
      dropEffect: 'none',
      types: ['application/x-a3s-work-local-paths'],
      getData: vi.fn(() => JSON.stringify(['/docs/Plan.docx'])),
    };
    const favorite = screen.getByRole('button', { name: '打开收藏文件夹 Reports' });

    fireEvent.click(favorite);
    expect(onOpenLocalFavorite).toHaveBeenCalledWith('/docs/Reports');
    fireEvent.dragOver(favorite, { dataTransfer });
    fireEvent.drop(favorite, { dataTransfer });
    expect(onMoveLocalEntries).toHaveBeenCalledWith(['/docs/Plan.docx'], '/docs/Reports');

    fireEvent.click(screen.getByRole('button', { name: '从侧边栏移除 Reports' }));
    expect(onRemoveLocalFavorite).toHaveBeenCalledWith('/docs/Reports');

    const systemDrop = {
      dropEffect: 'none',
      types: ['Files'],
      items: [],
      files: [{ name: 'notes.txt' }],
    } as unknown as DataTransfer;
    fireEvent.dragOver(favorite, { dataTransfer: systemDrop });
    fireEvent.drop(favorite, { dataTransfer: systemDrop });
    expect(onImportLocalDrop).toHaveBeenCalledWith(systemDrop, '/docs/Reports');
  });
});
