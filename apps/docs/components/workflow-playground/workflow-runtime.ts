import type { PlaygroundLang, WorkflowGraph } from './workflow-model';

export const defaultWorkflowInputs: Record<PlaygroundLang, Record<string, string>> = {
  en: {
    customer_message: 'I was charged twice for the same invoice.',
    priority: 'high',
  },
  cn: {
    customer_message: '同一张账单被重复扣款了。',
    priority: 'high',
  },
};

export function waitForWorkflowStep(ms: number, signal: AbortSignal): Promise<boolean> {
  return new Promise((resolve) => {
    const timeout = window.setTimeout(() => resolve(true), ms);
    signal.addEventListener('abort', () => {
      window.clearTimeout(timeout);
      resolve(false);
    }, { once: true });
  });
}

export function isWorkflowEditableTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLElement
    && (target.matches('input, textarea, select') || target.isContentEditable);
}

export function stripWorkflowRuntimeData(graph: WorkflowGraph): WorkflowGraph {
  return {
    nodes: graph.nodes.map((node) => ({
      ...node,
      selected: false,
      data: {
        kind: node.data.kind,
        label: node.data.label,
        description: node.data.description,
        configuration: node.data.configuration,
      },
    })),
    edges: graph.edges.map((edge) => ({
      id: edge.id,
      source: edge.source,
      target: edge.target,
      type: 'workflow',
      label: edge.label,
      data: edge.data?.route ? { route: edge.data.route } : {},
    })),
  };
}
