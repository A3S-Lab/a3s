import { useState } from 'react';
import { Copy, Minus, Play, Plus, Trash, X } from '@phosphor-icons/react';
import type { WorkflowPlaygroundCopy } from './workflow-copy';
import { workflowIconByProfile } from './workflow-icons';
import {
  getCatalogItem,
  type PlaygroundLang,
  type WorkflowNode,
  type WorkflowNodeConfiguration,
  type WorkflowNodeData,
  type WorkflowNodeProfile,
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

const outputVariables: Partial<Record<WorkflowNodeProfile, string[]>> = {
  'user-input': ['customer_message', 'priority', 'trigger'],
  'schedule-trigger': ['trigger'],
  'webhook-trigger': ['trigger'],
  'integration-trigger': ['trigger'],
  'question-classifier': ['intent', 'priority', 'confidence', 'selectedHandle'],
  'if-else': ['selectedHandle'],
  'human-input': ['decision', 'source'],
  agent: ['answer', 'context'],
  llm: ['text', 'intent', 'priority', 'confidence', 'usage'],
  output: ['result'],
  answer: ['answer'],
  template: ['result'],
  code: ['result'],
  'document-extractor': ['text', 'pages'],
  'http-request': ['status', 'body', 'method', 'url'],
  'knowledge-retrieval': ['records', 'query'],
  'list-operator': ['items'],
  'parameter-extractor': ['parameters', 'model'],
  tool: ['result', 'capability'],
  'variable-aggregator': ['value'],
  'variable-assigner': ['value', 'mode'],
  iteration: ['items', 'iterations'],
  loop: ['result', 'iterations', 'condition'],
};

function InputVariables({ profile }: { profile: WorkflowNodeProfile }) {
  return (
    <div className="a3s-variable-chips">
      <code>{'{{input.customer_message}}'}</code>
      <code>{'{{input.priority}}'}</code>
      {profile !== 'user-input' && profile !== 'question-classifier' ? <code>{'{{steps.classify.output}}'}</code> : null}
    </div>
  );
}

function OutputVariables({ profile }: { profile: WorkflowNodeProfile }) {
  const variables = outputVariables[profile] ?? ['result'];
  return (
    <div className="a3s-output-variables">
      {variables.map((variable) => <code key={variable}>{variable}<small>any</small></code>)}
    </div>
  );
}

interface ConfigurationFieldsProps {
  profile: WorkflowNodeProfile;
  config: WorkflowNodeConfiguration;
  copy: WorkflowPlaygroundCopy;
  update: (patch: Partial<WorkflowNodeConfiguration>) => void;
}

function ConfigurationFields({ profile, config, copy, update }: ConfigurationFieldsProps) {
  const catalogItem = getCatalogItem(profile);
  const ownsCapability = catalogItem.executionClass === 'owning_application_port';
  const hasFailurePolicy = ownsCapability && !['answer', 'llm', 'parameter-extractor', 'question-classifier'].includes(profile);

  return (
    <>
      {['template', 'output', 'answer'].includes(profile) ? (
        <label>
          <span>{copy.template}</span>
          <textarea className="a3s-inspector-code" rows={5} value={config.template} onChange={(event) => update({ template: event.currentTarget.value })} />
        </label>
      ) : null}

      {['llm', 'parameter-extractor', 'question-classifier'].includes(profile) ? (
        <>
          <label>
            <span>{copy.modelRoute}</span>
            <input className="a3s-inspector-code" value={config.model} onChange={(event) => update({ model: event.currentTarget.value })} />
          </label>
          {profile !== 'question-classifier' ? (
            <label>
              <span>{copy.prompt}</span>
              <textarea className="a3s-inspector-code" rows={6} value={config.prompt} onChange={(event) => update({ prompt: event.currentTarget.value })} />
            </label>
          ) : null}
        </>
      ) : null}

      {profile === 'code' ? (
        <label>
          <span>{copy.code}</span>
          <textarea className="a3s-inspector-code" rows={8} value={config.code} onChange={(event) => update({ code: event.currentTarget.value })} />
        </label>
      ) : null}

      {profile === 'http-request' ? (
        <div className="a3s-inspector-http">
          <label>
            <span>{copy.method}</span>
            <select value={config.method} onChange={(event) => update({ method: event.currentTarget.value as WorkflowNodeConfiguration['method'] })}>
              {['GET', 'POST', 'PUT', 'PATCH', 'DELETE'].map((method) => <option key={method} value={method}>{method}</option>)}
            </select>
          </label>
          <label>
            <span>{copy.url}</span>
            <input className="a3s-inspector-code" value={config.url} onChange={(event) => update({ url: event.currentTarget.value })} />
          </label>
        </div>
      ) : null}

      {profile === 'knowledge-retrieval' ? (
        <label>
          <span>{copy.query}</span>
          <input className="a3s-inspector-code" value={config.query} onChange={(event) => update({ query: event.currentTarget.value })} />
        </label>
      ) : null}

      {profile === 'list-operator' ? (
        <div className="a3s-inspector-http">
          <label>
            <span>{copy.filterField}</span>
            <input value={config.listFilterField} onChange={(event) => update({ listFilterField: event.currentTarget.value })} />
          </label>
          <label>
            <span>{copy.filterValue}</span>
            <input value={config.listFilterValue} onChange={(event) => update({ listFilterValue: event.currentTarget.value })} />
          </label>
        </div>
      ) : null}

      {profile === 'iteration' || profile === 'loop' ? (
        <>
          {profile === 'iteration' ? (
            <label>
              <span>{copy.collectionVariable}</span>
              <input className="a3s-inspector-code" value={config.inputVariable} onChange={(event) => update({ inputVariable: event.currentTarget.value })} />
            </label>
          ) : (
            <label>
              <span>{copy.loopCondition}</span>
              <input className="a3s-inspector-code" value={config.loopCondition} onChange={(event) => update({ loopCondition: event.currentTarget.value })} />
            </label>
          )}
          <label>
            <span>{copy.maxIterations}</span>
            <input type="number" min={1} max={100} value={config.maxIterations} onChange={(event) => update({ maxIterations: Number(event.currentTarget.value) })} />
          </label>
        </>
      ) : null}

      {profile === 'variable-aggregator' || profile === 'variable-assigner' ? (
        <div className="a3s-inspector-routes">
          <div><span>{copy.variableSelectors}</span></div>
          {config.variableSelectors.map((selector, index) => (
            <div className="a3s-inspector-selector" key={`${selector}-${index}`}>
              <input className="a3s-inspector-code" value={selector} onChange={(event) => update({ variableSelectors: config.variableSelectors.map((item, itemIndex) => itemIndex === index ? event.currentTarget.value : item) })} />
              <button type="button" onClick={() => update({ variableSelectors: config.variableSelectors.filter((_, itemIndex) => itemIndex !== index) })} aria-label={copy.removeVariable} title={copy.removeVariable}><Minus aria-hidden="true" /></button>
            </div>
          ))}
          <button className="a3s-inspector-add" type="button" onClick={() => update({ variableSelectors: [...config.variableSelectors, '{{current}}'] })}><Plus aria-hidden="true" />{copy.addVariable}</button>
          {profile === 'variable-assigner' ? (
            <label>
              <span>{copy.assignmentMode}</span>
              <select value={config.assignmentMode} onChange={(event) => update({ assignmentMode: event.currentTarget.value as WorkflowNodeConfiguration['assignmentMode'] })}>
                <option value="overwrite">{copy.assignmentOverwrite}</option>
                <option value="append">{copy.assignmentAppend}</option>
                <option value="clear">{copy.assignmentClear}</option>
              </select>
            </label>
          ) : null}
        </div>
      ) : null}

      {profile === 'if-else' || profile === 'question-classifier' ? (
        <>
          <label>
            <span>{copy.selector}</span>
            <input className="a3s-inspector-code" value={config.selector} onChange={(event) => update({ selector: event.currentTarget.value })} />
          </label>
          <div className="a3s-inspector-routes">
            <div><span>{copy.routes}</span></div>
            {config.routes.map((route, index) => (
              <div className="a3s-inspector-route" key={`${route.handle}-${index}`}>
                <label><span>{copy.routeHandle}</span><input value={route.handle} onChange={(event) => update({ routes: config.routes.map((item, routeIndex) => routeIndex === index ? { ...item, handle: event.currentTarget.value } : item) })} /></label>
                <label><span>{copy.routeEquals}</span><input value={route.equals} onChange={(event) => update({ routes: config.routes.map((item, routeIndex) => routeIndex === index ? { ...item, equals: event.currentTarget.value } : item) })} /></label>
                <button type="button" onClick={() => update({ routes: config.routes.filter((_, routeIndex) => routeIndex !== index) })} aria-label={copy.removeRoute} title={copy.removeRoute}><X aria-hidden="true" /></button>
              </div>
            ))}
            <button className="a3s-inspector-add" type="button" onClick={() => update({ routes: [...config.routes, { handle: `route_${config.routes.length + 1}`, equals: '' }] })}><Plus aria-hidden="true" />{copy.addRoute}</button>
          </div>
          <label><span>{copy.defaultRoute}</span><input value={config.defaultHandle} onChange={(event) => update({ defaultHandle: event.currentTarget.value })} /></label>
        </>
      ) : null}

      {profile === 'human-input' ? (
        <>
          <label><span>{copy.decisionMessage}</span><textarea rows={3} value={config.message} onChange={(event) => update({ message: event.currentTarget.value })} /></label>
          <label><span>{copy.decisionDetails}</span><textarea rows={4} value={config.details} onChange={(event) => update({ details: event.currentTarget.value })} /></label>
          <label><span>{copy.expiry}</span><input type="number" min={1} value={config.expiresAfterSeconds} onChange={(event) => update({ expiresAfterSeconds: Number(event.currentTarget.value) })} /></label>
        </>
      ) : null}

      {(ownsCapability || catalogItem.executionClass === 'invocation_only') && !['llm', 'parameter-extractor', 'question-classifier'].includes(profile) ? (
        <label>
          <span>{copy.capability}</span>
          <input className="a3s-inspector-code" value={config.capability} onChange={(event) => update({ capability: event.currentTarget.value })} />
        </label>
      ) : null}

      {hasFailurePolicy ? (
        <>
          <label><span>{copy.retryAttempts}</span><input type="number" min={1} max={32} value={config.retryAttempts} onChange={(event) => update({ retryAttempts: Number(event.currentTarget.value) })} /></label>
          <label>
            <span>{copy.failureHandling}</span>
            <select value={config.failureMode} onChange={(event) => update({ failureMode: event.currentTarget.value as WorkflowNodeConfiguration['failureMode'] })}>
              <option value="stop">{copy.failureStop}</option>
              <option value="default_output">{copy.failureDefault}</option>
              <option value="route">{copy.failureRoute}</option>
            </select>
          </label>
          {config.failureMode === 'default_output' ? <label><span>{copy.defaultOutput}</span><textarea className="a3s-inspector-code" rows={3} value={config.defaultOutput} onChange={(event) => update({ defaultOutput: event.currentTarget.value })} /></label> : null}
        </>
      ) : null}
    </>
  );
}

export function NodeInspector({ node, lang, copy, onClose, onChange, onRun, onDuplicate, onDelete }: NodeInspectorProps) {
  const [tab, setTab] = useState<'settings' | 'last-run'>('settings');
  const Icon = workflowIconByProfile[node.data.profile];
  const catalogItem = getCatalogItem(node.data.profile);
  const updateConfiguration = (patch: Partial<WorkflowNodeConfiguration>) => {
    onChange({ ...node.data, configuration: { ...node.data.configuration, ...patch } });
  };

  return (
    <aside className="a3s-node-inspector" aria-label={copy.inspector} data-testid="node-inspector">
      <header className="a3s-node-inspector__header">
        <div>
          <span aria-hidden="true"><Icon weight="duotone" /></span>
          <div><h2>{node.data.label}</h2><p>{catalogItem.name[lang]} · {catalogItem.semanticProfile}</p></div>
        </div>
        <button type="button" onClick={onClose} aria-label={copy.close} title={copy.close}><X aria-hidden="true" /></button>
      </header>

      <nav className="a3s-node-inspector__tabs" aria-label={copy.inspector}>
        <button type="button" className={tab === 'settings' ? 'is-active' : undefined} aria-pressed={tab === 'settings'} onClick={() => setTab('settings')}>{copy.settingsTab}</button>
        <button type="button" className={tab === 'last-run' ? 'is-active' : undefined} aria-pressed={tab === 'last-run'} onClick={() => setTab('last-run')}>{copy.lastRun}</button>
      </nav>

      {tab === 'settings' ? (
        <div className="a3s-node-inspector__scroll">
          <section className="a3s-inspector-section">
            <label><span>{copy.nodeName}</span><input value={node.data.label} onChange={(event) => onChange({ ...node.data, label: event.currentTarget.value })} /></label>
            <label><span>{copy.nodeDescription}</span><textarea rows={2} value={node.data.description} onChange={(event) => onChange({ ...node.data, description: event.currentTarget.value })} /></label>
          </section>
          <section className="a3s-inspector-section"><h3>{copy.inputVariables}</h3><InputVariables profile={node.data.profile} /></section>
          <section className="a3s-inspector-section">
            <h3>{copy.configuration}</h3>
            <ConfigurationFields profile={node.data.profile} config={node.data.configuration} copy={copy} update={updateConfiguration} />
          </section>
          <section className="a3s-inspector-section"><h3>{copy.outputVariables}</h3><OutputVariables profile={node.data.profile} /></section>
        </div>
      ) : (
        <div className="a3s-node-inspector__last-run">
          {node.data.lastRun ? (
            <>
              <div><strong>{copy.inputPayload}</strong><pre>{JSON.stringify(node.data.lastRun.input, null, 2)}</pre></div>
              <div><strong>{copy.outputPayload}</strong><pre>{JSON.stringify(node.data.lastRun.output, null, 2)}</pre></div>
            </>
          ) : <p>{copy.noNodeRun}</p>}
        </div>
      )}

      <footer className="a3s-node-inspector__footer">
        <button className="is-primary" type="button" onClick={onRun}><Play aria-hidden="true" weight="fill" />{copy.runStep}</button>
        <button type="button" onClick={onDuplicate} aria-label={copy.duplicate} title={copy.duplicate}><Copy aria-hidden="true" /></button>
        <button className="is-danger" type="button" onClick={onDelete} aria-label={copy.delete} title={copy.delete}><Trash aria-hidden="true" /></button>
      </footer>
    </aside>
  );
}
