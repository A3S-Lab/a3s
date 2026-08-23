import { Copy, Play, Plus, Trash, X } from '@phosphor-icons/react';
import type { WorkflowPlaygroundCopy } from './workflow-copy';
import { workflowIconByKind } from './workflow-icons';
import {
  getCatalogItem,
  type PlaygroundLang,
  type WorkflowNode,
  type WorkflowNodeConfiguration,
  type WorkflowNodeData,
  type WorkflowStepKind,
} from './workflow-model';

interface NodeInspectorProps {
  node: WorkflowNode;
  lang: PlaygroundLang;
  copy: WorkflowPlaygroundCopy;
  onClose: () => void;
  onChange: (data: WorkflowNodeData) => void;
  onRun: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
}

const capabilityKinds = new Set<WorkflowStepKind>([
  'execution', 'agent', 'mcp', 'model', 'tool', 'service', 'memory', 'subworkflow',
]);

const outputVariables: Partial<Record<WorkflowStepKind, string[]>> = {
  input: ['customer_message', 'priority'],
  model: ['intent', 'priority', 'confidence'],
  branch: ['selectedHandle'],
  human_decision: ['decision', 'source'],
  agent: ['answer', 'context'],
  output: ['result'],
  transform: ['result'],
};

function InputVariables({ kind }: { kind: WorkflowStepKind }) {
  return (
    <div className="a3s-variable-chips">
      <code>{'{{input.customer_message}}'}</code>
      <code>{'{{input.priority}}'}</code>
      {kind !== 'input' && kind !== 'model' ? <code>{'{{steps.classify.output}}'}</code> : null}
    </div>
  );
}

function OutputVariables({ kind }: { kind: WorkflowStepKind }) {
  const variables = outputVariables[kind] ?? ['result'];
  return (
    <div className="a3s-output-variables">
      {variables.map((variable) => <code key={variable}>{variable}<small>any</small></code>)}
    </div>
  );
}

export function NodeInspector({
  node,
  lang,
  copy,
  onClose,
  onChange,
  onRun,
  onDuplicate,
  onDelete,
}: NodeInspectorProps) {
  const Icon = workflowIconByKind[node.data.kind];
  const updateConfiguration = (patch: Partial<WorkflowNodeConfiguration>) => {
    onChange({
      ...node.data,
      configuration: { ...node.data.configuration, ...patch },
    });
  };
  const config = node.data.configuration;

  return (
    <aside className="a3s-node-inspector" aria-label={copy.inspector} data-testid="node-inspector">
      <header className="a3s-node-inspector__header">
        <div>
          <span aria-hidden="true"><Icon weight="duotone" /></span>
          <div>
            <h2>{node.data.label}</h2>
            <p>{getCatalogItem(node.data.kind).name[lang]}</p>
          </div>
        </div>
        <button type="button" onClick={onClose} aria-label={copy.close} title={copy.close}>
          <X aria-hidden="true" />
        </button>
      </header>

      <div className="a3s-node-inspector__scroll">
        <section className="a3s-inspector-section">
          <label>
            <span>{copy.nodeName}</span>
            <input
              value={node.data.label}
              onChange={(event) => onChange({ ...node.data, label: event.currentTarget.value })}
            />
          </label>
          <label>
            <span>{copy.nodeDescription}</span>
            <textarea
              rows={2}
              value={node.data.description}
              onChange={(event) => onChange({ ...node.data, description: event.currentTarget.value })}
            />
          </label>
        </section>

        <section className="a3s-inspector-section">
          <h3>{copy.inputVariables}</h3>
          <InputVariables kind={node.data.kind} />
        </section>

        <section className="a3s-inspector-section">
          <h3>{copy.configuration}</h3>

          {(node.data.kind === 'transform' || node.data.kind === 'output') ? (
            <label>
              <span>{copy.template}</span>
              <textarea
                className="a3s-inspector-code"
                rows={5}
                value={config.template}
                onChange={(event) => updateConfiguration({ template: event.currentTarget.value })}
              />
            </label>
          ) : null}

          {node.data.kind === 'branch' ? (
            <>
              <label>
                <span>{copy.selector}</span>
                <input
                  className="a3s-inspector-code"
                  value={config.selector}
                  onChange={(event) => updateConfiguration({ selector: event.currentTarget.value })}
                />
              </label>
              <div className="a3s-inspector-routes">
                <div><span>{copy.routes}</span></div>
                {config.routes.map((route, index) => (
                  <div className="a3s-inspector-route" key={`${route.handle}-${index}`}>
                    <label>
                      <span>{copy.routeHandle}</span>
                      <input
                        value={route.handle}
                        onChange={(event) => updateConfiguration({
                          routes: config.routes.map((item, routeIndex) => (
                            routeIndex === index ? { ...item, handle: event.currentTarget.value } : item
                          )),
                        })}
                      />
                    </label>
                    <label>
                      <span>{copy.routeEquals}</span>
                      <input
                        value={route.equals}
                        onChange={(event) => updateConfiguration({
                          routes: config.routes.map((item, routeIndex) => (
                            routeIndex === index ? { ...item, equals: event.currentTarget.value } : item
                          )),
                        })}
                      />
                    </label>
                    <button
                      type="button"
                      onClick={() => updateConfiguration({ routes: config.routes.filter((_, routeIndex) => routeIndex !== index) })}
                      aria-label={copy.removeRoute}
                      title={copy.removeRoute}
                    >
                      <X aria-hidden="true" />
                    </button>
                  </div>
                ))}
                <button
                  className="a3s-inspector-add"
                  type="button"
                  onClick={() => updateConfiguration({
                    routes: [...config.routes, { handle: `route_${config.routes.length + 1}`, equals: '' }],
                  })}
                >
                  <Plus aria-hidden="true" />{copy.addRoute}
                </button>
              </div>
              <label>
                <span>{copy.defaultRoute}</span>
                <input
                  value={config.defaultHandle}
                  onChange={(event) => updateConfiguration({ defaultHandle: event.currentTarget.value })}
                />
              </label>
            </>
          ) : null}

          {node.data.kind === 'human_decision' ? (
            <>
              <label>
                <span>{copy.decisionMessage}</span>
                <textarea
                  rows={3}
                  value={config.message}
                  onChange={(event) => updateConfiguration({ message: event.currentTarget.value })}
                />
              </label>
              <label>
                <span>{copy.decisionDetails}</span>
                <textarea
                  rows={4}
                  value={config.details}
                  onChange={(event) => updateConfiguration({ details: event.currentTarget.value })}
                />
              </label>
              <label>
                <span>{copy.expiry}</span>
                <input
                  type="number"
                  min={1}
                  value={config.expiresAfterSeconds}
                  onChange={(event) => updateConfiguration({ expiresAfterSeconds: Number(event.currentTarget.value) })}
                />
              </label>
            </>
          ) : null}

          {capabilityKinds.has(node.data.kind) ? (
            <>
              <label>
                <span>{copy.capability}</span>
                <input
                  className="a3s-inspector-code"
                  value={config.capability}
                  onChange={(event) => updateConfiguration({ capability: event.currentTarget.value })}
                />
              </label>
              <label>
                <span>{copy.retryAttempts}</span>
                <input
                  type="number"
                  min={1}
                  max={32}
                  value={config.retryAttempts}
                  onChange={(event) => updateConfiguration({ retryAttempts: Number(event.currentTarget.value) })}
                />
              </label>
              <label>
                <span>{copy.failureHandling}</span>
                <select
                  value={config.failureMode}
                  onChange={(event) => updateConfiguration({ failureMode: event.currentTarget.value as WorkflowNodeConfiguration['failureMode'] })}
                >
                  <option value="stop">{copy.failureStop}</option>
                  <option value="default_output">{copy.failureDefault}</option>
                  <option value="route">{copy.failureRoute}</option>
                </select>
              </label>
              {config.failureMode === 'default_output' ? (
                <label>
                  <span>{copy.defaultOutput}</span>
                  <textarea
                    className="a3s-inspector-code"
                    rows={3}
                    value={config.defaultOutput}
                    onChange={(event) => updateConfiguration({ defaultOutput: event.currentTarget.value })}
                  />
                </label>
              ) : null}
            </>
          ) : null}
        </section>

        <section className="a3s-inspector-section">
          <h3>{copy.outputVariables}</h3>
          <OutputVariables kind={node.data.kind} />
        </section>
      </div>

      <footer className="a3s-node-inspector__footer">
        <button className="is-primary" type="button" onClick={onRun}>
          <Play aria-hidden="true" weight="fill" />{copy.runStep}
        </button>
        <button type="button" onClick={onDuplicate} aria-label={copy.duplicate} title={copy.duplicate}>
          <Copy aria-hidden="true" />
        </button>
        <button className="is-danger" type="button" onClick={onDelete} aria-label={copy.delete} title={copy.delete}>
          <Trash aria-hidden="true" />
        </button>
      </footer>
    </aside>
  );
}
