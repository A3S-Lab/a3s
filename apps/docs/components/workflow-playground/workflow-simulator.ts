import {
  isRoutingProfile,
  isTriggerProfile,
  topologicalNodes,
  type PlaygroundLang,
  type WorkflowGraph,
  type WorkflowNode,
  type WorkflowRunStep,
} from './workflow-model';

function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, unknown>
    : { value };
}

function inputForNode(
  graph: WorkflowGraph,
  node: WorkflowNode,
  workflowInput: Record<string, unknown>,
  outputs: Record<string, Record<string, unknown>>,
): Record<string, unknown> {
  const values = graph.edges
    .filter((item) => item.target === node.id)
    .map((item) => outputs[item.source])
    .filter((value): value is Record<string, unknown> => Boolean(value));
  return values.at(-1) ?? workflowInput;
}

export function simulateWorkflowStep(
  node: WorkflowNode,
  workflowInput: Record<string, unknown>,
  priorOutputs: Record<string, Record<string, unknown>>,
  lang: PlaygroundLang = 'en',
  effectiveInput?: Record<string, unknown>,
): WorkflowRunStep {
  const previous = effectiveInput ?? Object.values(priorOutputs).at(-1) ?? workflowInput;
  const config = node.data.configuration;
  let output: Record<string, unknown>;

  switch (node.data.profile) {
    case 'user-input':
    case 'schedule-trigger':
    case 'webhook-trigger':
    case 'integration-trigger':
      output = { ...workflowInput, trigger: node.data.profile };
      break;
    case 'llm':
      output = {
        text: lang === 'cn' ? '已生成类型化模型结果。' : 'Generated a typed model result.',
        intent: lang === 'cn' ? '账单' : 'billing',
        priority: workflowInput.priority ?? 'normal',
        confidence: 0.94,
        model: config.model,
        usage: { inputTokens: 86, outputTokens: 24 },
      };
      break;
    case 'question-classifier':
      output = {
        intent: lang === 'cn' ? '账单' : 'billing',
        priority: workflowInput.priority ?? 'normal',
        confidence: 0.94,
        selectedHandle: config.routes.find((route) => route.equals === String(workflowInput.priority ?? ''))?.handle
          ?? config.defaultHandle,
      };
      break;
    case 'if-else':
      output = { selectedHandle: config.routes.find((route) => route.equals === String(workflowInput.priority ?? ''))?.handle ?? config.defaultHandle };
      break;
    case 'human-input':
      output = { decision: lang === 'cn' ? '已批准' : 'approved', source: lang === 'cn' ? 'Playground 本地模拟' : 'playground-simulation' };
      break;
    case 'agent':
      output = { answer: lang === 'cn' ? '账单请求已准备好进入授权响应。' : 'The billing request is ready for an authorized response.', context: previous };
      break;
    case 'output':
    case 'answer':
      output = previous;
      break;
    case 'template':
    case 'code':
      output = { result: previous };
      break;
    case 'document-extractor':
      output = { text: String(previous.text ?? workflowInput.customer_message ?? ''), pages: 1 };
      break;
    case 'http-request':
      output = { status: 200, body: previous, method: config.method, url: config.url };
      break;
    case 'knowledge-retrieval':
      output = { records: [{ id: 'knowledge-001', score: 0.93, content: String(workflowInput.customer_message ?? '') }], query: config.query };
      break;
    case 'list-operator': {
      const values = Array.isArray(previous.items) ? previous.items : Array.isArray(workflowInput.items) ? workflowInput.items : [];
      output = {
        items: values.filter((value) => {
          const record = asRecord(value);
          return config.listFilterValue.length === 0 || String(record[config.listFilterField] ?? '') === config.listFilterValue;
        }),
      };
      break;
    }
    case 'variable-aggregator':
      output = { value: previous.items ?? previous.value ?? previous.answer ?? previous.result ?? previous };
      break;
    case 'variable-assigner':
      output = { value: previous, mode: config.assignmentMode };
      break;
    case 'parameter-extractor':
      output = { parameters: { customer_id: 'customer-001', request_type: 'billing' }, model: config.model };
      break;
    case 'iteration': {
      const values = Array.isArray(previous.items) ? previous.items : Array.isArray(workflowInput.items) ? workflowInput.items : [];
      output = { items: values.slice(0, config.maxIterations), iterations: Math.min(values.length, config.maxIterations) };
      break;
    }
    case 'loop':
      output = { result: previous, iterations: Math.min(3, config.maxIterations), condition: config.loopCondition };
      break;
    case 'tool':
      output = { result: previous, capability: config.capability };
      break;
  }

  return {
    nodeId: node.id,
    label: node.data.label,
    profile: node.data.profile,
    kind: node.data.kind,
    status: 'succeeded',
    durationMs: node.data.profile === 'human-input' ? 480 : 240,
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
  const activeNodeIds = new Set(graph.nodes.filter((node) => isTriggerProfile(node.data.profile)).map((node) => node.id));
  const steps: WorkflowRunStep[] = [];

  for (const node of topologicalNodes(graph)) {
    if (!activeNodeIds.has(node.id)) continue;
    const effectiveInput = inputForNode(graph, node, workflowInput, outputs);
    const result = simulateWorkflowStep(node, workflowInput, outputs, lang, effectiveInput);
    outputs[node.id] = result.output;
    steps.push(result);

    const outgoing = graph.edges.filter((edge) => edge.source === node.id);
    const activeEdges = isRoutingProfile(node.data.profile)
      ? outgoing.filter((edge) => edge.data?.route === result.output.selectedHandle)
      : outgoing;
    for (const item of activeEdges) activeNodeIds.add(item.target);
  }

  return steps;
}
