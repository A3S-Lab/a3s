import { useEffect, useState } from 'react';
import { CheckCircle, ClockCounterClockwise, Code, ListBullets, X } from '@phosphor-icons/react';
import type { WorkflowPlaygroundCopy } from './workflow-copy';
import type { WorkflowRunStep } from './workflow-model';

export type DebugTab = 'trace' | 'variables' | 'history';

export interface PlaygroundRunRecord {
  id: string;
  status: 'succeeded' | 'failed';
  startedAt: string;
  durationMs: number;
  steps: WorkflowRunStep[];
}

interface DebugPanelProps {
  copy: WorkflowPlaygroundCopy;
  open: boolean;
  activeTab: DebugTab;
  onTabChange: (tab: DebugTab) => void;
  onClose: () => void;
  trace: WorkflowRunStep[];
  runningNodeId?: string;
  variables: Record<string, string>;
  onVariableChange: (key: string, value: string) => void;
  history: PlaygroundRunRecord[];
  onSelectNode: (nodeId: string) => void;
}

export function DebugPanel({
  copy,
  open,
  activeTab,
  onTabChange,
  onClose,
  trace,
  runningNodeId,
  variables,
  onVariableChange,
  history,
  onSelectNode,
}: DebugPanelProps) {
  const [selectedStepId, setSelectedStepId] = useState<string>();
  const selectedStep = trace.find((step) => step.nodeId === selectedStepId) ?? trace.at(-1);

  useEffect(() => {
    if (runningNodeId) setSelectedStepId(runningNodeId);
  }, [runningNodeId]);

  if (!open) return null;

  const tabs: Array<{ id: DebugTab; label: string; icon: typeof ListBullets }> = [
    { id: 'trace', label: copy.trace, icon: ListBullets },
    { id: 'variables', label: copy.cachedVariables, icon: Code },
    { id: 'history', label: copy.runHistory, icon: ClockCounterClockwise },
  ];

  return (
    <section className="a3s-debug-panel" aria-label={copy.debug} data-testid="debug-panel">
      <header>
        <nav aria-label={copy.debug}>
          {tabs.map((tab) => {
            const Icon = tab.icon;
            return (
              <button
                type="button"
                key={tab.id}
                className={activeTab === tab.id ? 'is-active' : undefined}
                aria-pressed={activeTab === tab.id}
                onClick={() => onTabChange(tab.id)}
              >
                <Icon aria-hidden="true" />{tab.label}
              </button>
            );
          })}
        </nav>
        <button type="button" onClick={onClose} aria-label={copy.close} title={copy.close}>
          <X aria-hidden="true" />
        </button>
      </header>

      {activeTab === 'trace' ? (
        <div className="a3s-debug-trace">
          <div className="a3s-debug-trace__list">
            {trace.length === 0 && !runningNodeId ? <p>{copy.noTrace}</p> : null}
            {trace.map((step) => (
              <button
                type="button"
                key={step.nodeId}
                className={selectedStep?.nodeId === step.nodeId ? 'is-active' : undefined}
                onClick={() => {
                  setSelectedStepId(step.nodeId);
                  onSelectNode(step.nodeId);
                }}
              >
                <CheckCircle aria-hidden="true" weight="fill" />
                <span><strong>{step.label}</strong><small>{step.kind}</small></span>
                <time>{step.durationMs} ms</time>
              </button>
            ))}
            {runningNodeId && !trace.some((step) => step.nodeId === runningNodeId) ? (
              <div className="a3s-debug-trace__running" role="status">
                <i aria-hidden="true" /><span>{copy.running}</span>
              </div>
            ) : null}
          </div>
          <div className="a3s-debug-trace__detail">
            {selectedStep ? (
              <>
                <div><strong>{selectedStep.label}</strong><span>{copy.statuses[selectedStep.status]}</span></div>
                <div className="a3s-debug-json-grid">
                  <section><h4>{copy.inputPayload}</h4><pre>{JSON.stringify(selectedStep.input, null, 2)}</pre></section>
                  <section><h4>{copy.outputPayload}</h4><pre>{JSON.stringify(selectedStep.output, null, 2)}</pre></section>
                </div>
              </>
            ) : <p>{copy.noTrace}</p>}
          </div>
        </div>
      ) : null}

      {activeTab === 'variables' ? (
        <div className="a3s-debug-variables">
          {Object.entries(variables).map(([key, value]) => (
            <label key={key}>
              <span><code>{key}</code><small>string</small></span>
              <input
                value={value}
                onChange={(event) => onVariableChange(key, event.currentTarget.value)}
                aria-label={`${copy.editValue}: ${key}`}
              />
            </label>
          ))}
        </div>
      ) : null}

      {activeTab === 'history' ? (
        <div className="a3s-debug-history">
          {history.length === 0 ? <p>{copy.noHistory}</p> : history.map((run) => (
            <article key={run.id}>
              <CheckCircle aria-hidden="true" weight="fill" />
              <div><strong>{run.id}</strong><time>{run.startedAt}</time></div>
              <span>{run.steps.length} {copy.steps}</span>
              <b>{run.durationMs} ms</b>
            </article>
          ))}
        </div>
      ) : null}
    </section>
  );
}
