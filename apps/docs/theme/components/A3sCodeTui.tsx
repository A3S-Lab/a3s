import { forwardRef, type ReactNode } from 'react';

export type A3sCodeTuiSurface =
  | 'hero'
  | 'code-intelligence'
  | 'hitl'
  | 'progressive-api'
  | 'runtime-tool'
  | 'cross-session-context';

interface A3sCodeTuiProps {
  ariaHidden?: boolean;
  ariaLabel?: string;
  assistiveText?: string;
  children: ReactNode;
  className?: string;
  controls?: ReactNode;
  footerAction?: ReactNode;
  footerLead: ReactNode;
  footerMeta: ReactNode;
  prompt: ReactNode;
  promptActive?: boolean;
  promptPrefix?: ReactNode;
  showcase?: boolean;
  status: string;
  summary?: ReactNode;
  surface: A3sCodeTuiSurface;
  phase?: string;
  title: string;
  toolbar?: ReactNode;
}

function classNames(...values: Array<string | false | undefined>) {
  return values.filter(Boolean).join(' ');
}

export const A3sCodeTui = forwardRef<HTMLElement, A3sCodeTuiProps>(
  function A3sCodeTui(
    {
      ariaHidden = false,
      ariaLabel,
      assistiveText,
      children,
      className,
      controls,
      footerAction,
      footerLead,
      footerMeta,
      prompt,
      promptActive = false,
      promptPrefix = '›',
      showcase = false,
      status,
      summary,
      surface,
      phase,
      title,
      toolbar,
    },
    ref,
  ) {
    return (
      <aside
        ref={ref}
        aria-hidden={ariaHidden || undefined}
        aria-label={ariaHidden ? undefined : ariaLabel}
        className={classNames('cli-terminal', 'a3s-code-tui', className)}
        data-a3s-code-tui={surface}
        data-cli-terminal-showcase={showcase || undefined}
        data-phase={phase}
      >
        <header data-tui-region="chrome">
          <span aria-hidden="true">
            <i />
            <i />
            <i />
          </span>
          <code>{title}</code>
          <div className="cli-terminal__header-actions">
            <em>{status}</em>
            {controls}
          </div>
        </header>

        {toolbar}

        <div className="cli-terminal__body a3s-code-tui__viewport">
          <p
            className="cli-terminal__prompt a3s-code-tui__prompt"
            data-tui-region="prompt"
          >
            <span>{promptPrefix}</span>
            <code>
              {prompt}
              {promptActive ? <i className="cli-terminal__cursor" /> : null}
            </code>
          </p>

          <div
            className="a3s-code-tui__transcript"
            data-tui-region="transcript"
          >
            {children}
          </div>

          {summary}
        </div>

        <footer
          className="cli-terminal__footer a3s-code-tui__footer"
          data-tui-region="footer"
        >
          <div>
            <b>{footerLead}</b>
            <i aria-hidden="true" />
            <code>{footerMeta}</code>
          </div>
          {footerAction}
        </footer>

        {assistiveText ? (
          <p className="cli-visually-hidden" aria-live="polite">
            {assistiveText}
          </p>
        ) : null}
      </aside>
    );
  },
);
