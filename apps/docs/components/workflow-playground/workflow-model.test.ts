import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  createInitialWorkflow,
  createWorkflowNode,
  insertNodeOnEdge,
  simulateWorkflow,
  topologicalNodes,
  validateWorkflow,
  workflowCatalog,
} from './workflow-model';

describe('Workflow Designer Playground model', () => {
  test('publishes the complete closed Cloud step-kind catalog', () => {
    assert.deepEqual(workflowCatalog.map((item) => item.kind), [
      'input',
      'output',
      'transform',
      'branch',
      'subworkflow',
      'agent',
      'model',
      'memory',
      'mcp',
      'tool',
      'service',
      'execution',
      'human_decision',
    ]);
    assert.equal(new Set(workflowCatalog.map((item) => item.kind)).size, 13);
  });

  test('ships a valid bilingual example graph', () => {
    const english = createInitialWorkflow('en');
    const chinese = createInitialWorkflow('cn');

    assert.deepEqual(validateWorkflow(english), []);
    assert.deepEqual(validateWorkflow(chinese), []);
    assert.deepEqual(english.nodes.map((node) => node.id), chinese.nodes.map((node) => node.id));
    assert.equal(english.nodes[0].data.label, 'Receive request');
    assert.equal(chinese.nodes[0].data.label, '接收请求');
    assert.equal(chinese.nodes.find((node) => node.id === 'review')?.data.configuration.message, '批准高优先级响应');
  });

  test('inserts a node by splitting the selected edge', () => {
    const graph = createInitialWorkflow('en');
    const node = createWorkflowNode('transform', { x: 220, y: 300 }, 'en', 'normalize');
    const inserted = insertNodeOnEdge(graph, 'input-classify', node);

    assert.equal(inserted.edges.some((edge) => edge.id === 'input-classify'), false);
    assert.equal(inserted.edges.some((edge) => edge.source === 'input' && edge.target === 'normalize'), true);
    assert.equal(inserted.edges.some((edge) => edge.source === 'normalize' && edge.target === 'classify'), true);
    assert.deepEqual(validateWorkflow(inserted), []);
  });

  test('rejects cycles and disconnected nodes', () => {
    const graph = createInitialWorkflow('en');
    graph.edges.push({
      id: 'cycle',
      source: 'output',
      target: 'classify',
      type: 'workflow',
      data: {},
    });
    graph.nodes.push(createWorkflowNode('tool', { x: 20, y: 20 }, 'en', 'orphan'));

    const codes = validateWorkflow(graph).map((issue) => issue.code);
    assert.ok(codes.includes('cycle'));
    assert.ok(codes.includes('missing_incoming'));
    assert.ok(codes.includes('missing_outgoing'));
    assert.ok(topologicalNodes(graph).length < graph.nodes.length);
  });

  test('simulates a deterministic topological trace', () => {
    const graph = createInitialWorkflow('en');
    const first = simulateWorkflow(graph, { customer_message: 'Invoice issue', priority: 'high' });
    const second = simulateWorkflow(graph, { customer_message: 'Invoice issue', priority: 'high' });

    assert.deepEqual(first, second);
    assert.deepEqual(first.map((step) => step.nodeId), [
      'input',
      'classify',
      'priority',
      'review',
      'compose',
      'output',
    ]);
    assert.deepEqual(first.at(-1)?.output, first.find((step) => step.nodeId === 'compose')?.output);
  });

  test('executes only the route selected by a Branch node', () => {
    const graph = createInitialWorkflow('en');
    const priority = simulateWorkflow(graph, { customer_message: 'Invoice issue', priority: 'high' });
    const standard = simulateWorkflow(graph, { customer_message: 'Invoice issue', priority: 'normal' });

    assert.deepEqual(priority.map((step) => step.nodeId), [
      'input',
      'classify',
      'priority',
      'review',
      'compose',
      'output',
    ]);
    assert.deepEqual(standard.map((step) => step.nodeId), [
      'input',
      'classify',
      'priority',
      'compose',
      'output',
    ]);
  });

  test('localizes synthetic run values for the Chinese Playground', () => {
    const trace = simulateWorkflow(
      createInitialWorkflow('cn'),
      { customer_message: '账单问题', priority: 'high' },
      'cn',
    );

    assert.equal(trace.find((step) => step.nodeId === 'classify')?.output.intent, '账单');
    assert.equal(trace.find((step) => step.nodeId === 'review')?.output.decision, '已批准');
    assert.equal(trace.find((step) => step.nodeId === 'compose')?.output.answer, '账单请求已准备好进入授权响应。');
  });
});
