import { type ReactNode, useCallback, useRef, useState } from 'react';
import { Button, Dialog } from '../../../design-system/primitives';
import { OfficeTextArea, OfficeTextField } from './office-text-field';

interface OfficePromptRequest {
  kind: 'prompt';
  title: string;
  description?: string;
  value: string;
  placeholder?: string;
  multiline?: boolean;
  confirmLabel: string;
}

interface OfficeNoticeRequest {
  kind: 'notice';
  title: string;
  description?: string;
  confirmLabel: string;
}

type OfficeDialogRequest = OfficePromptRequest | OfficeNoticeRequest;

export interface OfficePromptOptions {
  title: string;
  description?: string;
  initialValue?: string;
  placeholder?: string;
  multiline?: boolean;
  confirmLabel?: string;
}

export interface OfficeNoticeOptions {
  title: string;
  description?: string;
  confirmLabel?: string;
}

export function useOfficeDialog(): {
  prompt: (options: OfficePromptOptions) => Promise<string | null>;
  notice: (options: OfficeNoticeOptions) => Promise<void>;
  dialog: ReactNode;
} {
  const [request, setRequest] = useState<OfficeDialogRequest | null>(null);
  const promptResolver = useRef<((value: string | null) => void) | null>(null);
  const noticeResolver = useRef<(() => void) | null>(null);

  const closePrompt = useCallback((value: string | null) => {
    promptResolver.current?.(value);
    promptResolver.current = null;
    setRequest(null);
  }, []);
  const closeNotice = useCallback(() => {
    noticeResolver.current?.();
    noticeResolver.current = null;
    setRequest(null);
  }, []);

  const prompt = useCallback(
    (options: OfficePromptOptions) =>
      new Promise<string | null>((resolve) => {
        promptResolver.current?.(null);
        noticeResolver.current?.();
        promptResolver.current = resolve;
        noticeResolver.current = null;
        setRequest({
          kind: 'prompt',
          title: options.title,
          description: options.description,
          value: options.initialValue ?? '',
          placeholder: options.placeholder,
          multiline: options.multiline,
          confirmLabel: options.confirmLabel ?? '确定',
        });
      }),
    []
  );

  const notice = useCallback(
    (options: OfficeNoticeOptions) =>
      new Promise<void>((resolve) => {
        promptResolver.current?.(null);
        noticeResolver.current?.();
        promptResolver.current = null;
        noticeResolver.current = resolve;
        setRequest({
          kind: 'notice',
          title: options.title,
          description: options.description,
          confirmLabel: options.confirmLabel ?? '知道了',
        });
      }),
    []
  );

  const dialog = request ? (
    <Dialog
      title={request.title}
      description={request.description}
      className='work-office-dialog'
      onClose={() => (request.kind === 'prompt' ? closePrompt(null) : closeNotice())}
      footer={
        request.kind === 'prompt' ? (
          <>
            <Button tone='quiet' onClick={() => closePrompt(null)}>
              取消
            </Button>
            <Button onClick={() => closePrompt(request.value)}>{request.confirmLabel}</Button>
          </>
        ) : (
          <Button onClick={closeNotice}>{request.confirmLabel}</Button>
        )
      }
    >
      {request.kind === 'prompt' && (
        <div className='work-office-dialog-field'>
          <span className='sr-only'>{request.title}</span>
          {request.multiline ? (
            <OfficeTextArea
              aria-label={request.title}
              value={request.value}
              placeholder={request.placeholder}
              onChange={(event) => setRequest({ ...request, value: event.target.value })}
            />
          ) : (
            <OfficeTextField
              aria-label={request.title}
              value={request.value}
              placeholder={request.placeholder}
              onChange={(event) => setRequest({ ...request, value: event.target.value })}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault();
                  closePrompt(request.value);
                }
              }}
            />
          )}
        </div>
      )}
    </Dialog>
  ) : null;

  return { prompt, notice, dialog };
}
