import { X } from 'lucide-react';
import { useEffect } from 'react';

export const NOTICE_AUTO_DISMISS_MS = 4_500;

export interface NoticeBannerProps {
  notice: Readonly<{
    tone: 'info' | 'success' | 'error';
    message: string;
  }>;
  onDismiss: () => void;
}

export function NoticeBanner({ notice, onDismiss }: NoticeBannerProps) {
  useEffect(() => {
    if (notice.tone === 'error') return;
    const timeout = window.setTimeout(onDismiss, NOTICE_AUTO_DISMISS_MS);
    return () => window.clearTimeout(timeout);
  }, [notice.message, notice.tone, onDismiss]);

  return (
    <output className={`notice-banner notice-${notice.tone}`}>
      <i aria-hidden='true' />
      <span>{notice.message}</span>
      <button onClick={onDismiss} aria-label='关闭提示'>
        <X size={14} />
      </button>
    </output>
  );
}
