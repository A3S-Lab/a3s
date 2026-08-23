import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { withBase } from '@rspress/core/runtime';
import {
  applyEdgeChanges,
  applyNodeChanges,
  Background,
  BackgroundVariant,
  Controls,
  MarkerType,
  MiniMap,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type Connection,
  type EdgeChange,
  type NodeChange,
} from '@xyflow/react';
import { DebugPanel, type DebugTab, type PlaygroundRunRecord } from './debug-panel';
import { NodeInspector } from './node-inspector';
import { NodeLibrary } from './node-library';
import { RunDrawer } from './run-drawer';
import { useWorkflowDocument } from './use-workflow-document';
import { WorkflowCanvasDock, WorkflowHeader, WorkflowRail } from './workflow-chrome';
import { workflowCopy } from './workflow-copy';
import { WorkflowEdgeView } from './workflow-edge';
import {
  cloneWorkflowGraph,
  createInitialWorkflow,
  createWorkflowNode,
  insertNodeOnEdge,
  normalizePersistedWorkflow,
  relatedNodeIds,
  validateWorkflow,
  workflowCatalog,
  type PlaygroundLang,
  type WorkflowCanvasMode,
  type WorkflowEdge,
  type WorkflowGraph,
  type WorkflowNode,
  type WorkflowNodeData,
  type WorkflowNodeProfile,
  type WorkflowNodeStatus,
} from './workflow-model';
import { simulateWorkflow, simulateWorkflowStep } from './workflow-simulator';
import { WorkflowNodeView } from './workflow-node';
import { WorkflowOverlays } from './workflow-overlays';
import {
  defaultWorkflowInputs,
  isWorkflowEditableTarget,
  stripWorkflowRuntimeData,
  waitForWorkflowStep,
} from './workflow-runtime';

const nodeTypes = { workflow: WorkflowNodeView };
const edgeTypes = { workflow: WorkflowEdgeView };

function WorkflowMarkdown({ lang }: { lang: PlaygroundLang }) {
  const copy = workflowCopy[lang];
  return (
    <main>
      <h1>{copy.pageTitle}</h1>
      <p>{copy.simulatedNotice}</p>
      <h2>{copy.nodeLibrary}</h2>
      <ul>{workflowCatalog.map((item) => <li key={item.profile}>{item.name[lang]}: {item.description[lang]}</li>)}</ul>
    </main>
  );
}

function WorkflowDesignerSurface({ lang }: { lang: PlaygroundLang }) {
  const copy = workflowCopy[lang];
  const homeHref = withBase('/');
  const rootHref = lang === 'en' ? homeHref.replace(/en\/$/, '') : homeHref;
  const languageHref = lang === 'cn' ? `${rootHref}en/playground/workflow-designer` : `${rootHref}playground/workflow-designer`;
  const storageKey = `a3s-workflow-playground:${lang}`;
  const {
    graph,
    canUndo,
    canRedo,
    commit,
    updateTransient,
    undo,
    redo,
    restore,
    beginDrag,
    endDrag,
  } = useWorkflowDocument(() => createInitialWorkflow(lang));
  const { fitView, screenToFlowPosition } = useReactFlow<WorkflowNode, WorkflowEdge>();
  const [selectedNodeId, setSelectedNodeId] = useState<string>();
  const [canvasMode, setCanvasMode] = useState<WorkflowCanvasMode>('pan');
  const [relationshipFocus, setRelationshipFocus] = useState(false);
  const [nodeLibraryOpen, setNodeLibraryOpen] = useState(false);
  const [insertEdgeId, setInsertEdgeId] = useState<string>();
  const [pendingNodePosition, setPendingNodePosition] = useState<{ x: number; y: number }>();
  const [runDrawerOpen, setRunDrawerOpen] = useState(false);
  const [debugOpen, setDebugOpen] = useState(false);
  const [debugTab, setDebugTab] = useState<DebugTab>('trace');
  const [validationOpen, setValidationOpen] = useState(false);
  const [resetOpen, setResetOpen] = useState(false);
  const [storageReady, setStorageReady] = useState(false);
  const [saveState, setSaveState] = useState<'saved' | 'saving'>('saved');
  const [toast, setToast] = useState<string>();
  const [inputs, setInputs] = useState<Record<string, string>>(defaultWorkflowInputs[lang]);
  const [trace, setTrace] = useState<ReturnType<typeof simulateWorkflow>>([]);
  const [history, setHistory] = useState<PlaygroundRunRecord[]>([]);
  const [statuses, setStatuses] = useState<Record<string, WorkflowNodeStatus>>({});
  const [running, setRunning] = useState(false);
  const [runningNodeId, setRunningNodeId] = useState<string>();
  const nodeCounter = useRef(1);
  const edgeCounter = useRef(1);
  const runCounter = useRef(1);
  const runAbort = useRef<AbortController | undefined>(undefined);
  const issues = useMemo(() => validateWorkflow(graph), [graph]);
  const selectedNode = graph.nodes.find((node) => node.id === selectedNodeId);

  useEffect(() => {
    try {
      const persisted = window.localStorage.getItem(storageKey);
      if (persisted) {
        const parsed = JSON.parse(persisted) as WorkflowGraph;
        if (Array.isArray(parsed.nodes) && Array.isArray(parsed.edges)) restore(normalizePersistedWorkflow(parsed));
      }
    } catch {
      window.localStorage.removeItem(storageKey);
    } finally {
      setStorageReady(true);
    }
  }, [restore, storageKey]);

  useEffect(() => {
    if (!storageReady) return;
    setSaveState('saving');
    const timeout = window.setTimeout(() => {
      window.localStorage.setItem(storageKey, JSON.stringify(stripWorkflowRuntimeData(graph)));
      setSaveState('saved');
    }, 260);
    return () => window.clearTimeout(timeout);
  }, [graph, storageKey, storageReady]);

  useEffect(() => {
    if (!toast) return;
    const timeout = window.setTimeout(() => setToast(undefined), 2400);
    return () => window.clearTimeout(timeout);
  }, [toast]);

  useEffect(() => () => runAbort.current?.abort(), []);

  const deleteNode = useCallback((nodeId: string) => {
    if (running) return;
    commit((current) => ({
      nodes: current.nodes.filter((node) => node.id !== nodeId),
      edges: current.edges.filter((edge) => edge.source !== nodeId && edge.target !== nodeId),
    }));
    setSelectedNodeId((current) => current === nodeId ? undefined : current);
  }, [commit, running]);

  const duplicateNode = useCallback((nodeId: string) => {
    if (running) return;
    const source = graph.nodes.find((node) => node.id === nodeId);
    if (!source) return;
    let id = `${source.data.kind}-${nodeCounter.current++}`;
    while (graph.nodes.some((node) => node.id === id)) id = `${source.data.kind}-${nodeCounter.current++}`;
    const duplicate: WorkflowNode = {
      ...source,
      id,
      selected: false,
      position: { x: source.position.x + 48, y: source.position.y + 48 },
      data: cloneWorkflowGraph({ nodes: [source], edges: [] }).nodes[0].data,
    };
    commit((current) => ({ ...current, nodes: [...current.nodes, duplicate] }));
    setSelectedNodeId(id);
  }, [commit, graph.nodes, running]);

  const stopRun = useCallback(() => {
    runAbort.current?.abort();
    setRunning(false);
    setRunningNodeId(undefined);
    setStatuses({});
  }, []);

  const runSingleStep = useCallback(async (nodeId: string) => {
    const node = graph.nodes.find((candidate) => candidate.id === nodeId);
    if (!node || running) return;
    const controller = new AbortController();
    runAbort.current?.abort();
    runAbort.current = controller;
    setRunning(true);
    setDebugOpen(true);
    setDebugTab('trace');
    setTrace([]);
    setRunningNodeId(nodeId);
    setStatuses({ [nodeId]: 'running' });
    const completed = await waitForWorkflowStep(420, controller.signal);
    if (!completed) return;
    const result = simulateWorkflowStep(node, inputs, {}, lang);
    setTrace([result]);
    setStatuses({ [nodeId]: 'succeeded' });
    setRunningNodeId(undefined);
    setRunning(false);
  }, [graph.nodes, inputs, lang, running]);

  const runWorkflow = useCallback(async () => {
    if (running) return;
    const currentIssues = validateWorkflow(graph);
    if (currentIssues.length > 0) {
      setValidationOpen(true);
      return;
    }
    const controller = new AbortController();
    runAbort.current?.abort();
    runAbort.current = controller;
    const steps = simulateWorkflow(graph, inputs, lang);
    setRunning(true);
    setDebugOpen(true);
    setDebugTab('trace');
    setTrace([]);
    setStatuses(Object.fromEntries(steps.map((step) => [step.nodeId, 'queued'])));

    for (const step of steps) {
      if (controller.signal.aborted) return;
      setRunningNodeId(step.nodeId);
      setStatuses((current) => ({ ...current, [step.nodeId]: 'running' }));
      if (!await waitForWorkflowStep(360, controller.signal)) return;
      setTrace((current) => [...current, step]);
      setStatuses((current) => ({ ...current, [step.nodeId]: 'succeeded' }));
    }

    const now = new Date();
    setHistory((current) => [{
      id: `run-${String(runCounter.current++).padStart(3, '0')}`,
      status: 'succeeded' as const,
      startedAt: now.toLocaleTimeString(lang === 'cn' ? 'zh-CN' : 'en-US', { hour: '2-digit', minute: '2-digit', second: '2-digit' }),
      durationMs: steps.reduce((total, step) => total + step.durationMs, 0),
      steps,
    }, ...current].slice(0, 12));
    setRunningNodeId(undefined);
    setRunning(false);
    setRunDrawerOpen(false);
  }, [graph, inputs, lang, running]);

  const updateNodeData = useCallback((nodeId: string, data: WorkflowNodeData) => {
    commit((current) => ({
      ...current,
      nodes: current.nodes.map((node) => node.id === nodeId ? { ...node, data } : node),
    }));
  }, [commit]);

  const openNodeLibrary = useCallback((edgeId?: string, position?: { x: number; y: number }) => {
    if (running) return;
    setInsertEdgeId(edgeId);
    setPendingNodePosition(position);
    setNodeLibraryOpen(true);
  }, [running]);

  const addNode = useCallback((profile: WorkflowNodeProfile) => {
    let id = `${profile}-${nodeCounter.current++}`;
    while (graph.nodes.some((node) => node.id === id)) id = `${profile}-${nodeCounter.current++}`;
    const targetEdge = insertEdgeId ? graph.edges.find((edge) => edge.id === insertEdgeId) : undefined;
    const source = targetEdge ? graph.nodes.find((node) => node.id === targetEdge.source) : undefined;
    const target = targetEdge ? graph.nodes.find((node) => node.id === targetEdge.target) : undefined;
    const position = source && target
      ? { x: (source.position.x + target.position.x) / 2, y: (source.position.y + target.position.y) / 2 + 90 }
      : pendingNodePosition ?? screenToFlowPosition({ x: window.innerWidth / 2, y: window.innerHeight / 2 });
    const node = createWorkflowNode(profile, position, lang, id);
    commit((current) => insertEdgeId ? insertNodeOnEdge(current, insertEdgeId, node) : ({ ...current, nodes: [...current.nodes, node] }));
    setSelectedNodeId(id);
    setNodeLibraryOpen(false);
    setInsertEdgeId(undefined);
    setPendingNodePosition(undefined);
  }, [commit, graph.edges, graph.nodes, insertEdgeId, lang, pendingNodePosition, screenToFlowPosition]);

  const connectNodes = useCallback((connection: Connection) => {
    if (!connection.source || !connection.target || connection.source === connection.target || running) return;
    if (graph.edges.some((edge) => edge.source === connection.source && edge.target === connection.target)) return;
    const nextEdge: WorkflowEdge = {
      id: `edge-${edgeCounter.current++}`,
      source: connection.source,
      target: connection.target,
      sourceHandle: connection.sourceHandle,
      targetHandle: connection.targetHandle,
      type: 'workflow',
      data: {},
    };
    const candidate = { nodes: graph.nodes, edges: [...graph.edges, nextEdge] };
    if (validateWorkflow(candidate).some((issue) => issue.code === 'cycle')) {
      setToast(copy.validation.cycle);
      return;
    }
    commit(candidate);
  }, [commit, copy.validation.cycle, graph.edges, graph.nodes, running]);

  const onNodesChange = useCallback((changes: NodeChange<WorkflowNode>[]) => {
    updateTransient((current) => ({ ...current, nodes: applyNodeChanges(changes, current.nodes) }));
  }, [updateTransient]);

  const onEdgesChange = useCallback((changes: EdgeChange<WorkflowEdge>[]) => {
    updateTransient((current) => ({ ...current, edges: applyEdgeChanges(changes, current.edges) }));
  }, [updateTransient]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (isWorkflowEditableTarget(event.target)) return;
      const command = event.ctrlKey || event.metaKey;
      if (command && event.key.toLocaleLowerCase() === 'z') {
        event.preventDefault();
        event.shiftKey ? redo() : undo();
      } else if (command && event.key.toLocaleLowerCase() === 'd' && selectedNodeId) {
        event.preventDefault();
        duplicateNode(selectedNodeId);
      } else if ((event.key === 'Delete' || event.key === 'Backspace') && selectedNodeId) {
        event.preventDefault();
        deleteNode(selectedNodeId);
      } else if (event.key === 'Escape') {
        setNodeLibraryOpen(false);
        setRunDrawerOpen(false);
        setValidationOpen(false);
      } else if (event.key === 'Shift' && selectedNodeId) {
        setRelationshipFocus(true);
      }
    };
    const handleKeyUp = (event: KeyboardEvent) => {
      if (event.key === 'Shift') setRelationshipFocus(false);
    };
    const handleBlur = () => setRelationshipFocus(false);
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    window.addEventListener('blur', handleBlur);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
      window.removeEventListener('blur', handleBlur);
    };
  }, [deleteNode, duplicateNode, redo, selectedNodeId, undo]);

  const relatedNodes = useMemo(
    () => relationshipFocus && selectedNodeId ? relatedNodeIds(graph, selectedNodeId) : undefined,
    [graph, relationshipFocus, selectedNodeId],
  );

  const displayNodes = useMemo(() => graph.nodes.map((node) => ({
    ...node,
    selected: node.id === selectedNodeId,
    data: {
      ...node.data,
      lang,
      runtimeStatus: statuses[node.id] ?? 'idle',
      relationState: relatedNodes ? (relatedNodes.has(node.id) ? 'active' as const : 'dimmed' as const) : undefined,
      lastRun: [...trace].reverse().find((step) => step.nodeId === node.id),
      onRun: runSingleStep,
      onDuplicate: duplicateNode,
      onDelete: deleteNode,
    },
  })), [deleteNode, duplicateNode, graph.nodes, lang, relatedNodes, runSingleStep, selectedNodeId, statuses, trace]);

  const displayEdges = useMemo(() => graph.edges.map((edge) => ({
    ...edge,
    markerEnd: { type: MarkerType.ArrowClosed, width: 18, height: 18 },
    animated: running && (statuses[edge.source] === 'running' || statuses[edge.target] === 'running'),
    data: {
      ...edge.data,
      insertLabel: copy.addNode,
      relationState: relatedNodes ? (relatedNodes.has(edge.source) && relatedNodes.has(edge.target) ? 'active' as const : 'dimmed' as const) : undefined,
      onInsert: openNodeLibrary,
    },
  })), [copy.addNode, graph.edges, openNodeLibrary, relatedNodes, running, statuses]);

  const exportGraph = () => {
    const blob = new Blob([JSON.stringify(stripWorkflowRuntimeData(graph), null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = 'a3s-workflow-playground.json';
    anchor.click();
    URL.revokeObjectURL(url);
    setToast(copy.graphExported);
  };

  const resetWorkflow = () => {
    stopRun();
    restore(createInitialWorkflow(lang));
    setSelectedNodeId(undefined);
    setTrace([]);
    setHistory([]);
    setResetOpen(false);
    setToast(copy.restored);
    window.setTimeout(() => void fitView({ duration: 280, padding: 0.18 }), 0);
  };

  const rightPanelOpen = Boolean((selectedNode && !runDrawerOpen) || runDrawerOpen);
  const shellClass = [
    'a3s-workflow-playground',
    rightPanelOpen ? 'has-right-panel' : '',
    debugOpen ? 'has-debug-panel' : '',
    debugOpen && debugTab === 'history' ? 'is-history-view' : '',
    relationshipFocus ? 'is-relationship-focus' : '',
  ].filter(Boolean).join(' ');

  return (
    <main className={shellClass} data-language={lang} data-canvas-mode={canvasMode} data-testid="workflow-playground">
      <a className="a3s-workflow-skip" href="#workflow-canvas">{copy.canvasLabel}</a>
      <WorkflowHeader
        copy={copy}
        lang={lang}
        homeHref={homeHref}
        languageHref={languageHref}
        saveState={saveState}
        running={running}
        issueCount={issues.length}
        onReset={() => setResetOpen(true)}
        onExport={exportGraph}
        onValidate={() => setValidationOpen(true)}
        onRunToggle={() => running ? stopRun() : setRunDrawerOpen(true)}
      />

      <section className="a3s-workflow-stage">
        <WorkflowRail
          copy={copy}
          onAdd={() => openNodeLibrary()}
          mode={canvasMode}
          onModeChange={setCanvasMode}
        />

        <WorkflowCanvasDock
          copy={copy}
          canUndo={canUndo}
          canRedo={canRedo}
          running={running}
          onUndo={undo}
          onRedo={redo}
          onDebugTab={(tab) => { setDebugOpen(true); setDebugTab(tab); }}
        />

        <div className="a3s-workflow-canvas" id="workflow-canvas" aria-label={copy.canvasLabel}>
          <ReactFlow<WorkflowNode, WorkflowEdge>
            nodes={displayNodes}
            edges={displayEdges}
            nodeTypes={nodeTypes}
            edgeTypes={edgeTypes}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={connectNodes}
            onNodeClick={(_, node) => { setSelectedNodeId(node.id); setRunDrawerOpen(false); }}
            onPaneClick={() => setSelectedNodeId(undefined)}
            onPaneContextMenu={(event) => {
              event.preventDefault();
              openNodeLibrary(undefined, screenToFlowPosition({ x: event.clientX, y: event.clientY }));
            }}
            onNodeDragStart={beginDrag}
            onNodeDragStop={endDrag}
            nodesDraggable={!running}
            nodesConnectable={!running}
            elementsSelectable={!running}
            deleteKeyCode={null}
            panOnDrag={canvasMode === 'pan'}
            panOnScroll
            selectionOnDrag={canvasMode === 'select'}
            fitView
            fitViewOptions={{ padding: 0.16, minZoom: 0.44, maxZoom: 1 }}
            minZoom={0.25}
            maxZoom={1.6}
            proOptions={{ hideAttribution: true }}
          >
            <Background variant={BackgroundVariant.Dots} gap={22} size={1.2} color="#c5cfdd" />
            <MiniMap pannable zoomable nodeStrokeWidth={2} ariaLabel={copy.fitView} />
            <Controls showInteractive={false} position="bottom-right" />
          </ReactFlow>
          <p className="a3s-workflow-hint">{copy.keyboardHint}</p>
        </div>

        <NodeLibrary copy={copy} lang={lang} open={nodeLibraryOpen} onClose={() => { setNodeLibraryOpen(false); setInsertEdgeId(undefined); setPendingNodePosition(undefined); }} onSelect={addNode} />

        {selectedNode && !runDrawerOpen ? (
          <NodeInspector
            node={{
              ...selectedNode,
              data: { ...selectedNode.data, lastRun: [...trace].reverse().find((step) => step.nodeId === selectedNode.id) },
            }}
            lang={lang}
            copy={copy}
            onClose={() => setSelectedNodeId(undefined)}
            onChange={(data) => updateNodeData(selectedNode.id, data)}
            onRun={() => void runSingleStep(selectedNode.id)}
            onDuplicate={() => duplicateNode(selectedNode.id)}
            onDelete={() => deleteNode(selectedNode.id)}
          />
        ) : null}

        <RunDrawer
          copy={copy}
          open={runDrawerOpen}
          running={running}
          values={inputs}
          issues={issues}
          onClose={() => setRunDrawerOpen(false)}
          onChange={(key, value) => setInputs((current) => ({ ...current, [key]: value }))}
          onStart={() => void runWorkflow()}
        />

        <DebugPanel
          copy={copy}
          open={debugOpen}
          activeTab={debugTab}
          onTabChange={setDebugTab}
          onClose={() => setDebugOpen(false)}
          trace={trace}
          runningNodeId={runningNodeId}
          variables={inputs}
          onVariableChange={(key, value) => setInputs((current) => ({ ...current, [key]: value }))}
          history={history}
          onSelectNode={setSelectedNodeId}
        />
      </section>

      <WorkflowOverlays
        copy={copy}
        issues={issues}
        validationOpen={validationOpen}
        resetOpen={resetOpen}
        toast={toast}
        onCloseValidation={() => setValidationOpen(false)}
        onCloseReset={() => setResetOpen(false)}
        onConfirmReset={resetWorkflow}
      />
    </main>
  );
}

export default function WorkflowDesignerPlayground({ lang }: { lang: PlaygroundLang }) {
  if (import.meta.env.SSG_MD) return <WorkflowMarkdown lang={lang} />;
  return <ReactFlowProvider><WorkflowDesignerSurface lang={lang} /></ReactFlowProvider>;
}
