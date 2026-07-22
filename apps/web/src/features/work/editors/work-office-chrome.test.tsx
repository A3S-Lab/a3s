import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { WorkOfficeRibbon, WorkOfficeRibbonButton } from './work-office-chrome';

afterEach(cleanup);

describe('Work Office ribbon chrome', () => {
  it('opens the file menu once when Enter produces its native click', async () => {
    renderRibbon();
    const trigger = screen.getByRole('button', { name: '文件' });

    fireEvent.keyDown(trigger, { key: 'Enter' });
    fireEvent.click(trigger);

    expect(screen.getByRole('menu', { name: '文件菜单' })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole('menuitem', { name: '另存为' })).toHaveFocus());
  });

  it('opens from either arrow key and focuses the corresponding edge action', async () => {
    renderRibbon();
    const trigger = screen.getByRole('button', { name: '文件' });

    fireEvent.keyDown(trigger, { key: 'ArrowUp' });

    expect(screen.getByRole('menu', { name: '文件菜单' })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole('menuitem', { name: '打印' })).toHaveFocus());
  });

  it('closes the file menu when keyboard focus leaves it', async () => {
    render(
      <>
        {ribbon()}
        <button type='button'>编辑正文</button>
      </>
    );
    fireEvent.click(screen.getByRole('button', { name: '文件' }));
    const saveAs = screen.getByRole('menuitem', { name: '另存为' });
    await waitFor(() => expect(saveAs).toHaveFocus());

    screen.getByRole('button', { name: '编辑正文' }).focus();

    await waitFor(() => expect(screen.queryByRole('menu', { name: '文件菜单' })).not.toBeInTheDocument());
  });

  it('exposes pressed state only for toggle and selection controls', () => {
    render(
      <>
        <WorkOfficeRibbonButton label='保存'>保存</WorkOfficeRibbonButton>
        <WorkOfficeRibbonButton label='加粗' active={false}>
          B
        </WorkOfficeRibbonButton>
        <WorkOfficeRibbonButton label='居中' active>
          居中
        </WorkOfficeRibbonButton>
      </>
    );

    expect(screen.getByRole('button', { name: '保存' })).not.toHaveAttribute('aria-pressed');
    expect(screen.getByRole('button', { name: '加粗' })).toHaveAttribute('aria-pressed', 'false');
    expect(screen.getByRole('button', { name: '居中' })).toHaveAttribute('aria-pressed', 'true');
  });
});

function renderRibbon() {
  return render(ribbon());
}

function ribbon() {
  return (
    <WorkOfficeRibbon
      ariaLabel='测试功能区'
      tabs={[
        { id: 'home', label: '开始' },
        { id: 'insert', label: '插入' },
      ]}
      defaultTab='home'
      panels={{ home: <button type='button'>加粗</button>, insert: <button type='button'>图片</button> }}
      fileActions={[
        { id: 'save', label: '保存', disabled: true, onSelect: vi.fn() },
        { id: 'save-as', label: '另存为', onSelect: vi.fn() },
        { id: 'print', label: '打印', onSelect: vi.fn() },
      ]}
    />
  );
}
