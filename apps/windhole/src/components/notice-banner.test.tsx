import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { NoticeBanner, NOTICE_AUTO_DISMISS_MS } from './notice-banner';

describe('NoticeBanner', () => {
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it('automatically clears non-error feedback so it does not cover the battlefield indefinitely', () => {
    vi.useFakeTimers();
    const onDismiss = vi.fn();
    render(<NoticeBanner notice={{ tone: 'success', message: '编队已更新。' }} onDismiss={onDismiss} />);

    act(() => vi.advanceTimersByTime(NOTICE_AUTO_DISMISS_MS));

    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it('keeps errors visible until the user dismisses them', () => {
    vi.useFakeTimers();
    const onDismiss = vi.fn();
    const view = render(<NoticeBanner notice={{ tone: 'error', message: '部署失败。' }} onDismiss={onDismiss} />);

    act(() => vi.advanceTimersByTime(NOTICE_AUTO_DISMISS_MS * 2));
    expect(onDismiss).not.toHaveBeenCalled();

    fireEvent.click(view.getByRole('button', { name: '关闭提示' }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });
});
