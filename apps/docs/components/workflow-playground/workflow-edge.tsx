import { memo } from 'react';
import { Plus } from '@phosphor-icons/react';
import {
  BaseEdge,
  EdgeLabelRenderer,
  getSmoothStepPath,
  type EdgeProps,
} from '@xyflow/react';
import type { WorkflowEdge } from './workflow-model';

function WorkflowEdgeComponent({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  markerEnd,
  data,
  selected,
}: EdgeProps<WorkflowEdge>) {
  const [path, labelX, labelY] = getSmoothStepPath({
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
    borderRadius: 18,
    offset: 28,
  });

  return (
    <>
      <BaseEdge
        id={id}
        path={path}
        markerEnd={markerEnd}
        className={[selected ? 'is-selected' : '', data?.relationState === 'dimmed' ? 'is-dimmed' : ''].filter(Boolean).join(' ') || undefined}
      />
      <EdgeLabelRenderer>
        <div
          className={`a3s-workflow-edge-label nodrag nopan${data?.relationState === 'dimmed' ? ' is-dimmed' : ''}`}
          style={{ transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)` }}
        >
          {data?.route ? <span>{data.route}</span> : null}
          <button
            type="button"
            onClick={(event) => {
              event.stopPropagation();
              data?.onInsert?.(id);
            }}
            aria-label={data?.insertLabel ?? 'Insert node'}
            title={data?.insertLabel ?? 'Insert node'}
          >
            <Plus aria-hidden="true" weight="bold" />
          </button>
        </div>
      </EdgeLabelRenderer>
    </>
  );
}

export const WorkflowEdgeView = memo(WorkflowEdgeComponent);
