import type { Edge, Node, XYPosition } from '@xyflow/react';

export type PlaygroundLang = 'cn' | 'en';

export type WorkflowStepKind =
  | 'input'
  | 'output'
  | 'transform'
  | 'branch'
  | 'human_decision'
  | 'execution'
  | 'agent'
  | 'mcp'
  | 'model'
  | 'tool'
  | 'service'
  | 'memory'
  | 'subworkflow';

export type WorkflowNodeGroup = 'flow' | 'intelligence' | 'integration' | 'human';
export type WorkflowNodeStatus = 'idle' | 'queued' | 'running' | 'succeeded' | 'failed';

export interface WorkflowRoute {
  handle: string;
  equals: string;
}

export interface WorkflowNodeConfiguration {
  template: string;
  selector: string;
  routes: WorkflowRoute[];
  defaultHandle: string;
  message: string;
  details: string;
  expiresAfterSeconds: number;
  capability: string;
  retryAttempts: number;
  failureMode: 'stop' | 'default_output' | 'route';
  defaultOutput: string;
}

export interface WorkflowNodeData extends Record<string, unknown> {
  kind: WorkflowStepKind;
  label: string;
  description: string;
  configuration: WorkflowNodeConfiguration;
  lang?: PlaygroundLang;
  runtimeStatus?: WorkflowNodeStatus;
  onRun?: (nodeId: string) => void;
  onDuplicate?: (nodeId: string) => void;
  onDelete?: (nodeId: string) => void;
}

export interface WorkflowEdgeData extends Record<string, unknown> {
  route?: string;
  insertLabel?: string;
  onInsert?: (edgeId: string) => void;
}

export type WorkflowNode = Node<WorkflowNodeData, 'workflow'>;
export type WorkflowEdge = Edge<WorkflowEdgeData, 'workflow'>;

export interface WorkflowGraph {
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
}

export interface WorkflowCatalogItem {
  kind: WorkflowStepKind;
  group: WorkflowNodeGroup;
  name: Record<PlaygroundLang, string>;
  description: Record<PlaygroundLang, string>;
}

export type WorkflowValidationCode =
  | 'input_count'
  | 'output_missing'
  | 'missing_source'
  | 'missing_target'
  | 'missing_incoming'
  | 'missing_outgoing'
  | 'branch_routes'
  | 'cycle'
  | 'configuration';

export interface WorkflowValidationIssue {
  code: WorkflowValidationCode;
  nodeId?: string;
}

export interface WorkflowRunStep {
  nodeId: string;
  label: string;
  kind: WorkflowStepKind;
  status: Exclude<WorkflowNodeStatus, 'idle' | 'queued' | 'running'>;
  durationMs: number;
  input: Record<string, unknown>;
  output: Record<string, unknown>;
}

const bilingual = (en: string, cn: string): Record<PlaygroundLang, string> => ({ en, cn });

export const workflowCatalog: WorkflowCatalogItem[] = [
  { kind: 'input', group: 'flow', name: bilingual('Input', '输入'), description: bilingual('Declare workflow inputs', '声明工作流输入') },
  { kind: 'output', group: 'flow', name: bilingual('Output', '输出'), description: bilingual('Return typed results', '返回类型化结果') },
  { kind: 'transform', group: 'flow', name: bilingual('Transform', '转换'), description: bilingual('Render a deterministic template', '渲染确定性模板') },
  { kind: 'branch', group: 'flow', name: bilingual('Branch', '分支'), description: bilingual('Route by an exact selector', '按精确选择器路由') },
  { kind: 'subworkflow', group: 'flow', name: bilingual('Subworkflow', '子工作流'), description: bilingual('Call an immutable revision', '调用不可变工作流版本') },
  { kind: 'agent', group: 'intelligence', name: bilingual('Agent', 'Agent'), description: bilingual('Run an admitted Agent profile', '运行准入的 Agent Profile') },
  { kind: 'model', group: 'intelligence', name: bilingual('Model', '模型'), description: bilingual('Use an exact model route', '使用精确模型路由') },
  { kind: 'memory', group: 'intelligence', name: bilingual('Memory', '记忆'), description: bilingual('Use an admitted memory capability', '使用准入的记忆能力') },
  { kind: 'mcp', group: 'integration', name: bilingual('MCP', 'MCP'), description: bilingual('Invoke an admitted MCP service', '调用准入的 MCP 服务') },
  { kind: 'tool', group: 'integration', name: bilingual('Tool', '工具'), description: bilingual('Apply an A3S Use capability', '应用 A3S Use 能力') },
  { kind: 'service', group: 'integration', name: bilingual('Service', '服务'), description: bilingual('Call a governed connector', '调用受治理的连接器') },
  { kind: 'execution', group: 'integration', name: bilingual('Execution', '执行'), description: bilingual('Run a finite task template', '运行有限任务模板') },
  { kind: 'human_decision', group: 'human', name: bilingual('Human decision', '人工决策'), description: bilingual('Pause for an authorized decision', '暂停并等待授权决策') },
];

export function getCatalogItem(kind: WorkflowStepKind): WorkflowCatalogItem {
  const item = workflowCatalog.find((candidate) => candidate.kind === kind);
  if (!item) throw new Error(`Unsupported workflow step kind: ${kind}`);
  return item;
}

export function defaultConfiguration(kind: WorkflowStepKind, lang: PlaygroundLang = 'en'): WorkflowNodeConfiguration {
  const base: WorkflowNodeConfiguration = {
    template: '',
    selector: '',
    routes: [],
    defaultHandle: 'default',
    message: '',
    details: '',
    expiresAfterSeconds: 3600,
    capability: '',
    retryAttempts: 3,
    failureMode: 'stop',
    defaultOutput: '{}',
  };

  if (kind === 'transform') base.template = '{{current.result}}';
  if (kind === 'output') base.template = '{{steps.compose.output.answer}}';
  if (kind === 'branch') {
    base.selector = '{{steps.classify.output.priority}}';
    base.routes = [
      { handle: 'priority', equals: 'high' },
      { handle: 'standard', equals: 'normal' },
    ];
    base.defaultHandle = 'standard';
  }
  if (kind === 'human_decision') {
    base.message = lang === 'cn' ? '批准高优先级响应' : 'Approve the priority response';
    base.details = lang === 'cn'
      ? '请在工作流继续前审核建议操作。'
      : 'Review the proposed action before the workflow continues.';
  }
  if (kind === 'model') base.capability = 'inference/support-classifier@3';
  if (kind === 'agent') base.capability = 'agent/support-response@5';
  if (kind === 'memory') base.capability = 'memory/customer-context@2';
  if (kind === 'mcp') base.capability = 'mcp/customer-records@2';
  if (kind === 'tool') base.capability = 'tool/ticket-priority@1';
  if (kind === 'service') base.capability = 'connector/support-api@4';
  if (kind === 'execution') base.capability = 'execution/triage-task@2';
  if (kind === 'subworkflow') base.capability = 'workflow/escalation@7';
  return base;
}

export function createWorkflowNode(
  kind: WorkflowStepKind,
  position: XYPosition,
  lang: PlaygroundLang,
  id: string,
  label?: string,
): WorkflowNode {
  const item = getCatalogItem(kind);
  return {
    id,
    type: 'workflow',
    position,
    data: {
      kind,
      label: label ?? item.name[lang],
      description: item.description[lang],
      configuration: defaultConfiguration(kind, lang),
    },
  };
}

function edge(id: string, source: string, target: string, route?: string): WorkflowEdge {
  return {
    id,
    source,
    target,
    type: 'workflow',
    data: route ? { route } : {},
    label: route,
  };
}

export function createInitialWorkflow(lang: PlaygroundLang): WorkflowGraph {
  const labels = lang === 'cn'
    ? ['接收请求', '识别意图', '优先级判断', '人工确认', '生成回复', '返回结果']
    : ['Receive request', 'Classify intent', 'Priority check', 'Human review', 'Compose response', 'Return result'];

  return {
    nodes: [
      createWorkflowNode('input', { x: 60, y: 220 }, lang, 'input', labels[0]),
      createWorkflowNode('model', { x: 350, y: 220 }, lang, 'classify', labels[1]),
      createWorkflowNode('branch', { x: 650, y: 220 }, lang, 'priority', labels[2]),
      createWorkflowNode('human_decision', { x: 960, y: 70 }, lang, 'review', labels[3]),
      createWorkflowNode('agent', { x: 960, y: 370 }, lang, 'compose', labels[4]),
      createWorkflowNode('output', { x: 1280, y: 370 }, lang, 'output', labels[5]),
    ],
    edges: [
      edge('input-classify', 'input', 'classify'),
      edge('classify-priority', 'classify', 'priority'),
      edge('priority-review', 'priority', 'review', 'priority'),
      edge('priority-compose', 'priority', 'compose', 'standard'),
      edge('review-compose', 'review', 'compose'),
      edge('compose-output', 'compose', 'output'),
    ],
  };
}

export function cloneWorkflowGraph(graph: WorkflowGraph): WorkflowGraph {
  return {
    nodes: graph.nodes.map((node) => ({
      ...node,
      position: { ...node.position },
      data: {
        ...node.data,
        configuration: {
          ...node.data.configuration,
          routes: node.data.configuration.routes.map((route) => ({ ...route })),
        },
      },
    })),
    edges: graph.edges.map((item) => ({ ...item, data: { ...item.data } })),
  };
}

function configurationIsValid(node: WorkflowNode): boolean {
  const config = node.data.configuration;
  if (node.data.kind === 'transform' || node.data.kind === 'output') return config.template.trim().length > 0;
  if (node.data.kind === 'branch') return config.selector.trim().length > 0 && config.routes.length > 0;
  if (node.data.kind === 'human_decision') return config.message.trim().length > 0 && config.expiresAfterSeconds > 0;
  if (['execution', 'agent', 'mcp', 'model', 'tool', 'service', 'memory', 'subworkflow'].includes(node.data.kind)) {
    return config.capability.trim().length > 0;
  }
  return true;
}

export function validateWorkflow(graph: WorkflowGraph): WorkflowValidationIssue[] {
  const issues: WorkflowValidationIssue[] = [];
  const nodeIds = new Set(graph.nodes.map((node) => node.id));
  const inputs = graph.nodes.filter((node) => node.data.kind === 'input');
  const outputs = graph.nodes.filter((node) => node.data.kind === 'output');

  if (inputs.length !== 1) issues.push({ code: 'input_count' });
  if (outputs.length === 0) issues.push({ code: 'output_missing' });

  for (const item of graph.edges) {
    if (!nodeIds.has(item.source)) issues.push({ code: 'missing_source' });
    if (!nodeIds.has(item.target)) issues.push({ code: 'missing_target' });
  }

  for (const node of graph.nodes) {
    const incoming = graph.edges.filter((item) => item.target === node.id);
    const outgoing = graph.edges.filter((item) => item.source === node.id);
    if (node.data.kind !== 'input' && incoming.length === 0) issues.push({ code: 'missing_incoming', nodeId: node.id });
    if (node.data.kind !== 'output' && outgoing.length === 0) issues.push({ code: 'missing_outgoing', nodeId: node.id });
    if (node.data.kind === 'branch' && outgoing.length < 2) issues.push({ code: 'branch_routes', nodeId: node.id });
    if (!configurationIsValid(node)) issues.push({ code: 'configuration', nodeId: node.id });
  }

  if (topologicalNodes(graph).length !== graph.nodes.length) issues.push({ code: 'cycle' });
  return issues;
}

export function topologicalNodes(graph: WorkflowGraph): WorkflowNode[] {
  const nodesById = new Map(graph.nodes.map((node) => [node.id, node]));
  const indegree = new Map(graph.nodes.map((node) => [node.id, 0]));
  const outgoing = new Map(graph.nodes.map((node) => [node.id, [] as string[]]));

  for (const item of graph.edges) {
    if (!nodesById.has(item.source) || !nodesById.has(item.target)) continue;
    indegree.set(item.target, (indegree.get(item.target) ?? 0) + 1);
    outgoing.get(item.source)?.push(item.target);
  }

  const queue = graph.nodes.filter((node) => indegree.get(node.id) === 0);
  const ordered: WorkflowNode[] = [];
  while (queue.length > 0) {
    const node = queue.shift();
    if (!node) break;
    ordered.push(node);
    for (const target of outgoing.get(node.id) ?? []) {
      const next = (indegree.get(target) ?? 0) - 1;
      indegree.set(target, next);
      if (next === 0) {
        const targetNode = nodesById.get(target);
        if (targetNode) queue.push(targetNode);
      }
    }
  }
  return ordered;
}

export function insertNodeOnEdge(graph: WorkflowGraph, edgeId: string, node: WorkflowNode): WorkflowGraph {
  const targetEdge = graph.edges.find((item) => item.id === edgeId);
  if (!targetEdge) return { nodes: [...graph.nodes, node], edges: [...graph.edges] };

  return {
    nodes: [...graph.nodes, node],
    edges: [
      ...graph.edges.filter((item) => item.id !== edgeId),
      edge(`${targetEdge.source}-${node.id}`, targetEdge.source, node.id, targetEdge.data?.route),
      edge(`${node.id}-${targetEdge.target}`, node.id, targetEdge.target),
    ],
  };
}

export function simulateWorkflowStep(
  node: WorkflowNode,
  workflowInput: Record<string, unknown>,
  priorOutputs: Record<string, Record<string, unknown>>,
  lang: PlaygroundLang = 'en',
): WorkflowRunStep {
  const previous = Object.values(priorOutputs).at(-1) ?? workflowInput;
  let output: Record<string, unknown>;

  switch (node.data.kind) {
    case 'input':
      output = { ...workflowInput };
      break;
    case 'model':
      output = { intent: lang === 'cn' ? '账单' : 'billing', priority: workflowInput.priority ?? 'normal', confidence: 0.94 };
      break;
    case 'branch':
      output = {
        selectedHandle: node.data.configuration.routes.find(
          (route) => route.equals === String(workflowInput.priority ?? ''),
        )?.handle ?? node.data.configuration.defaultHandle,
      };
      break;
    case 'human_decision':
      output = {
        decision: lang === 'cn' ? '已批准' : 'approved',
        source: lang === 'cn' ? 'Playground 本地模拟' : 'playground-simulation',
      };
      break;
    case 'agent':
      output = {
        answer: lang === 'cn'
          ? '账单请求已准备好进入授权响应。'
          : 'The billing request is ready for an authorized response.',
        context: previous,
      };
      break;
    case 'output':
      output = priorOutputs.compose ?? previous;
      break;
    case 'transform':
      output = { result: previous };
      break;
    default:
      output = { accepted: true, capability: node.data.configuration.capability, input: previous };
      break;
  }

  return {
    nodeId: node.id,
    label: node.data.label,
    kind: node.data.kind,
    status: 'succeeded',
    durationMs: node.data.kind === 'human_decision' ? 480 : 240,
    input: previous,
    output,
  };
}

export function simulateWorkflow(
  graph: WorkflowGraph,
  workflowInput: Record<string, unknown>,
  lang: PlaygroundLang = 'en',
): WorkflowRunStep[] {
  const outputs: Record<string, Record<string, unknown>> = {};
  const activeNodeIds = new Set(
    graph.nodes.filter((node) => node.data.kind === 'input').map((node) => node.id),
  );
  const steps: WorkflowRunStep[] = [];

  for (const node of topologicalNodes(graph)) {
    if (!activeNodeIds.has(node.id)) continue;
    const result = simulateWorkflowStep(node, workflowInput, outputs, lang);
    outputs[node.id] = result.output;
    steps.push(result);

    const outgoing = graph.edges.filter((edge) => edge.source === node.id);
    const activeEdges = node.data.kind === 'branch'
      ? outgoing.filter((edge) => edge.data?.route === result.output.selectedHandle)
      : outgoing;
    for (const edge of activeEdges) activeNodeIds.add(edge.target);
  }

  return steps;
}
