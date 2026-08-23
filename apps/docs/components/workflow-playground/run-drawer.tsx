import { Info, Play, X } from '@phosphor-icons/react';
import type { WorkflowPlaygroundCopy } from './workflow-copy';
import type { WorkflowValidationIssue } from './workflow-model';

interface RunDrawerProps {
  copy: WorkflowPlaygroundCopy;
  open: boolean;
  running: boolean;
  values: Record<string, string>;
  issues: WorkflowValidationIssue[];
  onClose: () => void;
  onChange: (key: string, value: string) => void;
  onStart: () => void;
}

export function RunDrawer({ copy, open, running, values, issues, onClose, onChange, onStart }: RunDrawerProps) {
  if (!open) return null;

  return (
    <aside className="a3s-run-drawer" aria-label={copy.testRun} data-testid="run-drawer">
      <header>
        <div><h2>{copy.testRun}</h2><p>{copy.runDescription}</p></div>
        <button type="button" onClick={onClose} aria-label={copy.close} title={copy.close} disabled={running}>
          <X aria-hidden="true" />
        </button>
      </header>

      <form onSubmit={(event) => { event.preventDefault(); onStart(); }}>
        <label>
          <span>{copy.customerMessage}</span>
          <textarea
            rows={5}
            value={values.customer_message ?? ''}
            onChange={(event) => onChange('customer_message', event.currentTarget.value)}
            required
          />
        </label>
        <label>
          <span>{copy.priority}</span>
          <select
            value={values.priority ?? 'normal'}
            onChange={(event) => onChange('priority', event.currentTarget.value)}
          >
            <option value="normal">{copy.normal}</option>
            <option value="high">{copy.high}</option>
          </select>
        </label>

        {issues.length > 0 ? (
          <section className="a3s-run-validation" role="alert">
            <strong>{copy.validationFailed}</strong>
            <ul>{issues.map((issue, index) => <li key={`${issue.code}-${issue.nodeId ?? index}`}>{copy.validation[issue.code]}</li>)}</ul>
          </section>
        ) : null}

        <div className="a3s-run-notice"><Info aria-hidden="true" /><p>{copy.simulatedNotice}</p></div>

        <button className="a3s-run-submit" type="submit" disabled={running || issues.length > 0}>
          <Play aria-hidden="true" weight="fill" />{running ? copy.running : copy.startRun}
        </button>
      </form>
    </aside>
  );
}
