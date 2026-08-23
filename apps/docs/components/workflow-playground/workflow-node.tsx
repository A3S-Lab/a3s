import { memo } from 'react';
import { Copy, Play, Trash } from '@phosphor-icons/react';
import { Handle, NodeToolbar, Position, type NodeProps } from '@xyflow/react';
import { workflowCopy } from './workflow-copy';
import { workflowIconByKind } from './workflow-icons';
import { getCatalogItem, type WorkflowNode } from './workflow-model';

function configurationSummary(node: WorkflowNode): string {
  const config = node.data.configuration;
  if (node.data.kind === 'branch') return config.selector;
  if (node.data.kind === 'transform' || node.data.kind === 'output') return config.template;
  if (node.data.kind === 'human_decision') return `${config.expiresAfterSeconds}s`;
  if (config.capability) return config.capability;
  return node.data.description;
}

function WorkflowNodeComponent({ id, data, selected }: NodeProps<WorkflowNode>) {
  const lang = data.lang ?? 'en';
  const copy = workflowCopy[lang];
  const Icon = workflowIconByKind[data.kind];
  const status = data.runtimeStatus ?? 'idle';
  const hasTarget = data.kind !== 'input';
  const hasSource = data.kind !== 'output';

  return (
    <article
      className="a3s-workflow-node"
      data-kind={data.kind}
      data-status={status}
      aria-label={`${data.label}, ${getCatalogItem(data.kind).name[lang]}, ${copy.statuses[status]}`}
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
          <small>{getCatalogItem(data.kind).name[lang]}</small>
        </span>
        <i className="a3s-workflow-node__status" title={copy.statuses[status]} aria-hidden="true" />
      </header>
      <p>{configurationSummary({ id, data, type: 'workflow', position: { x: 0, y: 0 } })}</p>

      {hasSource ? <Handle className="a3s-node-handle" type="source" position={Position.Right} /> : null}
    </article>
  );
}

export const WorkflowNodeView = memo(WorkflowNodeComponent);
