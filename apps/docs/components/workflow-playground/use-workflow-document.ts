import { useCallback, useRef, useState } from 'react';
import { cloneWorkflowGraph, type WorkflowGraph } from './workflow-model';

interface WorkflowDocumentState {
  past: WorkflowGraph[];
  present: WorkflowGraph;
  future: WorkflowGraph[];
}

type GraphUpdater = WorkflowGraph | ((graph: WorkflowGraph) => WorkflowGraph);

const historyLimit = 60;

function sameGraph(left: WorkflowGraph, right: WorkflowGraph): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

export function useWorkflowDocument(initial: () => WorkflowGraph) {
  const [document, setDocument] = useState<WorkflowDocumentState>(() => ({
    past: [],
    present: initial(),
    future: [],
  }));
  const dragOrigin = useRef<WorkflowGraph | undefined>(undefined);

  const commit = useCallback((updater: GraphUpdater) => {
    setDocument((current) => {
      const base = cloneWorkflowGraph(current.present);
      const next = typeof updater === 'function' ? updater(base) : updater;
      if (sameGraph(current.present, next)) return current;
      return {
        past: [...current.past, cloneWorkflowGraph(current.present)].slice(-historyLimit),
        present: cloneWorkflowGraph(next),
        future: [],
      };
    });
  }, []);

  const updateTransient = useCallback((updater: (graph: WorkflowGraph) => WorkflowGraph) => {
    setDocument((current) => ({ ...current, present: updater(current.present) }));
  }, []);

  const undo = useCallback(() => {
    setDocument((current) => {
      const previous = current.past.at(-1);
      if (!previous) return current;
      return {
        past: current.past.slice(0, -1),
        present: cloneWorkflowGraph(previous),
        future: [cloneWorkflowGraph(current.present), ...current.future],
      };
    });
  }, []);

  const redo = useCallback(() => {
    setDocument((current) => {
      const next = current.future[0];
      if (!next) return current;
      return {
        past: [...current.past, cloneWorkflowGraph(current.present)].slice(-historyLimit),
        present: cloneWorkflowGraph(next),
        future: current.future.slice(1),
      };
    });
  }, []);

  const restore = useCallback((graph: WorkflowGraph) => {
    setDocument({ past: [], present: cloneWorkflowGraph(graph), future: [] });
  }, []);

  const beginDrag = useCallback(() => {
    dragOrigin.current = cloneWorkflowGraph(document.present);
  }, [document.present]);

  const endDrag = useCallback(() => {
    const origin = dragOrigin.current;
    dragOrigin.current = undefined;
    if (!origin) return;
    setDocument((current) => {
      if (sameGraph(origin, current.present)) return current;
      return {
        past: [...current.past, origin].slice(-historyLimit),
        present: current.present,
        future: [],
      };
    });
  }, []);

  return {
    graph: document.present,
    canUndo: document.past.length > 0,
    canRedo: document.future.length > 0,
    commit,
    updateTransient,
    undo,
    redo,
    restore,
    beginDrag,
    endDrag,
  };
}
