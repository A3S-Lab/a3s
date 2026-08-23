import { CheckCircle, Trash, WarningCircle, X } from '@phosphor-icons/react';
import type { WorkflowPlaygroundCopy } from './workflow-copy';
import type { WorkflowValidationIssue } from './workflow-model';

interface WorkflowOverlaysProps {
  copy: WorkflowPlaygroundCopy;
  issues: WorkflowValidationIssue[];
  validationOpen: boolean;
  resetOpen: boolean;
  toast?: string;
  onCloseValidation: () => void;
  onCloseReset: () => void;
  onConfirmReset: () => void;
}

export function WorkflowOverlays({
  copy,
  issues,
  validationOpen,
  resetOpen,
  toast,
  onCloseValidation,
  onCloseReset,
  onConfirmReset,
}: WorkflowOverlaysProps) {
  return (
    <>
      {validationOpen ? (
        <section className="a3s-validation-popover" role="status" aria-label={copy.validationTitle}>
          <header>
            {issues.length === 0 ? <CheckCircle aria-hidden="true" weight="fill" /> : <WarningCircle aria-hidden="true" />}
            <div><strong>{copy.validationTitle}</strong><p>{issues.length === 0 ? copy.validationPassed : copy.validationFailed}</p></div>
            <button type="button" onClick={onCloseValidation} aria-label={copy.close} title={copy.close}><X aria-hidden="true" /></button>
          </header>
          {issues.length > 0 ? <ul>{issues.map((issue, index) => <li key={`${issue.code}-${issue.nodeId ?? index}`}>{copy.validation[issue.code]}</li>)}</ul> : null}
        </section>
      ) : null}

      {resetOpen ? (
        <div className="a3s-workflow-dialog-backdrop" role="presentation">
          <dialog className="a3s-workflow-dialog" open aria-labelledby="workflow-reset-title">
            <Trash aria-hidden="true" />
            <h2 id="workflow-reset-title">{copy.resetTitle}</h2>
            <p>{copy.resetDescription}</p>
            <div><button type="button" onClick={onCloseReset}>{copy.cancel}</button><button className="is-danger" type="button" onClick={onConfirmReset}>{copy.confirmReset}</button></div>
          </dialog>
        </div>
      ) : null}

      {toast ? <div className="a3s-workflow-toast" role="status"><CheckCircle aria-hidden="true" weight="fill" />{toast}</div> : null}
    </>
  );
}
