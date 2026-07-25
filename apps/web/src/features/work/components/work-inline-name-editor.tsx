import { Check, LoaderCircle, X } from 'lucide-react';
import { useEffect, useId, useRef } from 'react';
import { IconButton } from '../../../design-system/primitives';

export function WorkInlineNameEditor({
  value,
  label,
  saveLabel,
  cancelLabel,
  selectionEnd,
  busy = false,
  error = null,
  className = '',
  onChange,
  onSave,
  onCancel,
}: {
  value: string;
  label: string;
  saveLabel: string;
  cancelLabel: string;
  selectionEnd: number;
  busy?: boolean;
  error?: string | null;
  className?: string;
  onChange: (value: string) => void;
  onSave: () => void | Promise<void>;
  onCancel: () => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const errorId = useId();

  useEffect(() => {
    const input = inputRef.current;
    if (!input) return;
    input.focus();
    input.setSelectionRange(0, selectionEnd);
  }, [selectionEnd]);

  return (
    <form
      className={`work-inline-name-editor ${className}`.trim()}
      data-work-inline-name-editor
      aria-label={label}
      onClick={(event) => event.stopPropagation()}
      onDoubleClick={(event) => event.stopPropagation()}
      onPointerDown={(event) => event.stopPropagation()}
      onSubmit={(event) => {
        event.preventDefault();
        if (value.trim() && !busy) void onSave();
      }}
    >
      <input
        ref={inputRef}
        aria-label={label}
        aria-invalid={Boolean(error)}
        aria-describedby={error ? errorId : undefined}
        value={value}
        maxLength={255}
        disabled={busy}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          event.stopPropagation();
          if (event.key !== 'Escape') return;
          event.preventDefault();
          if (!busy) onCancel();
        }}
      />
      {error && (
        <span className='work-inline-name-error' id={errorId} role='alert' title={error}>
          <span aria-hidden='true'>!</span>
          <span className='sr-only'>{error}</span>
        </span>
      )}
      <IconButton type='submit' label={saveLabel} disabled={!value.trim() || busy}>
        {busy ? <LoaderCircle className='spin' size={13} /> : <Check size={13} />}
      </IconButton>
      <IconButton label={cancelLabel} disabled={busy} onClick={onCancel}>
        <X size={13} />
      </IconButton>
    </form>
  );
}
