import { memo } from 'react';
import { Copy, Play, Trash } from '@phosphor-icons/react';
import { Handle, NodeToolbar, Position, type NodeProps } from '@xyflow/react';
import { workflowCopy } from './workflow-copy';
import { workflowIconByProfile } from './workflow-icons';
import { getCatalogItem, isTerminalProfile, isTriggerProfile, type WorkflowNode } from './workflow-model';

function configurationSummary(node: WorkflowNode): string {
  const config = node.data.configuration;
  if (node.data.profile === 'if-else' || node.data.profile === 'question-classifier') return config.selector || config.model;
  if (node.data.profile === 'template' || node.data.profile === 'output' || node.data.profile === 'answer') return config.template;
  if (node.data.profile === 'human-input') return `${config.expiresAfterSeconds}s`;
  if (node.data.profile === 'llm' || node.data.profile === 'parameter-extractor') return config.model;
  if (node.data.profile === 'http-request') return `${config.method} ${config.url}`;
  if (node.data.profile === 'list-operator') return `${config.listFilterField} = ${config.listFilterValue}`;
  if (node.data.profile === 'iteration' || node.data.profile === 'loop') return `max ${config.maxIterations}`;
  if (config.capability) return config.capability;
  return node.data.description;
}

function WorkflowNodeComponent({ id, data, selected }: NodeProps<WorkflowNode>) {
  const lang = data.lang ?? 'en';
  const copy = workflowCopy[lang];
  const Icon = workflowIconByProfile[data.profile];
  const status = data.runtimeStatus ?? 'idle';
  const hasTarget = !isTriggerProfile(data.profile);
  const hasSource = !isTerminalProfile(data.profile);

  return (
    <article
      className="a3s-workflow-node"
      data-profile={data.profile}
      data-kind={data.kind}
      data-status={status}
      data-relation={data.relationState}
      aria-label={`${data.label}, ${getCatalogItem(data.profile).name[lang]}, ${copy.statuses[status]}`}
    >
      <NodeToolbar className="a3s-node-toolbar" isVisible={selected} position={Position.Top}>
        <button type="button" onClick={() => data.onRun?.(id)} aria-label={copy.runStep} title={copy.runStep}>
          <Play aria-hidden="true" weight="fill" />
        </button>
        <button type="button" onClick={() => data.onDuplicate?.(id)} aria-label={copy.duplicate} title={copy.duplicate}>
          <Copy aria-hidden="true" />
        </button>
        <button type="button" onClick={() => data.onDelete?.(id)} aria-label={copy.delete} title={copy.delete}>
          <Trash aria-hidden="true" />
        </button>
      </NodeToolbar>

      {hasTarget ? <Handle className="a3s-node-handle" type="target" position={Position.Left} /> : null}

      <header>
        <span className="a3s-workflow-node__icon" aria-hidden="true"><Icon weight="duotone" /></span>
        <span>
          <strong>{data.label}</strong>
          <small>{getCatalogItem(data.profile).name[lang]}</small>
        </span>
        <i className="a3s-workflow-node__status" title={copy.statuses[status]} aria-hidden="true" />
      </header>
      <p>{configurationSummary({ id, data, type: 'workflow', position: { x: 0, y: 0 } })}</p>

      {hasSource ? <Handle className="a3s-node-handle" type="source" position={Position.Right} /> : null}
    </article>
  );
}

export const WorkflowNodeView = memo(WorkflowNodeComponent);
