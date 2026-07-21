import { forwardRef, useCallback, useEffect, useImperativeHandle, useMemo, useRef, useState } from 'react';
import ForceGraph3D, { type ForceGraphMethods } from 'react-force-graph-3d';
import * as THREE from 'three';
import type { MemoryGraphProjection, MemoryVisualNode } from '../memory-projection';

const LABEL_LIMIT = 28;

const LIGHT_PALETTE = {
  background: '#ffffff',
  event: '#5865d9',
  source: '#7957c5',
  tool: '#c97816',
  file: '#347eaa',
  tag: '#238a73',
  other: '#7f8792',
  selected: '#2864e8',
  dimNode: '#dfe2e7',
  edge: 'rgba(139, 146, 158, 0.38)',
  activeEdge: '#5578cc',
  dimEdge: 'rgba(139, 146, 158, 0.09)',
  label: '#24272d',
  labelStroke: '#ffffff',
} as const;

const DARK_PALETTE = {
  background: '#171820',
  event: '#8790f2',
  source: '#aa8be1',
  tool: '#e5a45f',
  file: '#6ba8ce',
  tag: '#65bda7',
  other: '#949aa5',
  selected: '#6ca3ff',
  dimNode: '#333641',
  edge: 'rgba(151, 157, 172, 0.32)',
  activeEdge: '#86a9ef',
  dimEdge: 'rgba(114, 119, 131, 0.1)',
  label: '#f2f3f5',
  labelStroke: '#171820',
} as const;

type GraphPalette = typeof LIGHT_PALETTE | typeof DARK_PALETTE;

interface Graph3DNode extends Omit<MemoryVisualNode, 'x' | 'y'> {
  name: string;
  val: number;
  degree: number;
  x?: number;
  y?: number;
  z?: number;
}

interface Graph3DLink {
  source: string | Graph3DNode;
  target: string | Graph3DNode;
  kind: string;
  weight: number;
  memoryId: string;
}

type GraphInstance = ForceGraphMethods<Graph3DNode, Graph3DLink>;

export interface MemoryGraph3DHandle {
  focusNode: (id: string) => void;
  resetView: () => void;
  zoomIn: () => void;
  zoomOut: () => void;
}

export interface MemoryGraph3DProps {
  graph: MemoryGraphProjection;
  selectedNodeId?: string;
  onSelectNode: (node: MemoryVisualNode) => void;
  onClearSelection: () => void;
}

const MemoryGraph3D = forwardRef<MemoryGraph3DHandle, MemoryGraph3DProps>(function MemoryGraph3D(
  { graph, selectedNodeId, onSelectNode, onClearSelection },
  ref
) {
  const graphRef = useRef<GraphInstance | undefined>(undefined);
  const resizeCleanupRef = useRef<(() => void) | null>(null);
  const fitFrameRef = useRef(0);
  const initializedRef = useRef(false);
  const autoFittedRef = useRef(false);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [dark, setDark] = useState(detectDarkMode);
  const [webglAvailable] = useState(
    () => typeof window === 'undefined' || typeof window.WebGLRenderingContext !== 'undefined'
  );
  const palette = dark ? DARK_PALETTE : LIGHT_PALETTE;

  useEffect(() => {
    const root = document.documentElement;
    const syncTheme = () => setDark(root.classList.contains('dark'));
    syncTheme();
    const observer = new MutationObserver(syncTheme);
    observer.observe(root, { attributes: true, attributeFilter: ['class'] });
    return () => observer.disconnect();
  }, []);

  const attachContainer = useCallback((element: HTMLDivElement | null) => {
    resizeCleanupRef.current?.();
    resizeCleanupRef.current = null;
    if (!element) return;

    const measure = () => {
      const bounds = element.getBoundingClientRect();
      setSize({ width: Math.round(bounds.width), height: Math.round(bounds.height) });
    };
    measure();

    if (typeof ResizeObserver !== 'undefined') {
      const observer = new ResizeObserver(measure);
      observer.observe(element);
      resizeCleanupRef.current = () => observer.disconnect();
      return;
    }

    window.addEventListener('resize', measure);
    resizeCleanupRef.current = () => window.removeEventListener('resize', measure);
  }, []);

  useEffect(
    () => () => {
      resizeCleanupRef.current?.();
      window.cancelAnimationFrame(fitFrameRef.current);
      graphRef.current?.pauseAnimation();
    },
    []
  );

  const graphData = useMemo(() => {
    const degreeById = new Map(graph.nodes.map((node) => [node.id, 0]));
    for (const edge of graph.edges) {
      degreeById.set(edge.from, (degreeById.get(edge.from) ?? 0) + 1);
      degreeById.set(edge.to, (degreeById.get(edge.to) ?? 0) + 1);
    }
    const nodes: Graph3DNode[] = graph.nodes.map((node) => {
      const degree = degreeById.get(node.id) ?? 0;
      return {
        id: node.id,
        nodeType: node.nodeType,
        label: node.label,
        size: node.size,
        tone: node.tone,
        memoryId: node.memoryId,
        entityId: node.entityId,
        relatedMemoryIds: [...node.relatedMemoryIds],
        name: node.label,
        degree,
        val: Math.max(1.2, node.size / 3.4 + Math.log2(degree + 1) * 0.75),
      };
    });
    const links: Graph3DLink[] = graph.edges.map((edge) => ({
      source: edge.from,
      target: edge.to,
      kind: edge.kind,
      weight: edge.weight,
      memoryId: edge.memoryId,
    }));
    return { nodes, links };
  }, [graph.edges, graph.nodes]);

  const dataSignature = useMemo(
    () =>
      `${graphData.nodes
        .map((node) => node.id)
        .sort()
        .join('|')}::${graphData.links
        .map((link) => `${linkEndId(link.source)}>${linkEndId(link.target)}:${link.kind}`)
        .sort()
        .join('|')}`,
    [graphData]
  );

  useEffect(() => {
    autoFittedRef.current = false;
  }, [dataSignature]);

  const nodeById = useMemo(() => new Map(graphData.nodes.map((node) => [node.id, node])), [graphData.nodes]);
  const connectedNodeIds = useMemo(() => {
    const connected = new Set<string>();
    if (!selectedNodeId) return connected;
    connected.add(selectedNodeId);
    for (const link of graphData.links) {
      const source = linkEndId(link.source);
      const target = linkEndId(link.target);
      if (source === selectedNodeId) connected.add(target);
      if (target === selectedNodeId) connected.add(source);
    }
    return connected;
  }, [graphData.links, selectedNodeId]);

  const labelledNodeIds = useMemo(
    () =>
      new Set(
        [...graphData.nodes]
          .sort(
            (left, right) =>
              Number(right.nodeType === 'event') - Number(left.nodeType === 'event') ||
              right.degree - left.degree ||
              right.val - left.val ||
              left.name.localeCompare(right.name)
          )
          .slice(0, LABEL_LIMIT)
          .map((node) => node.id)
      ),
    [graphData.nodes]
  );

  const nodeVal = useCallback(
    (node: Graph3DNode) => (node.id === selectedNodeId ? node.val * 1.9 : node.val),
    [selectedNodeId]
  );
  const nodeColor = useCallback(
    (node: Graph3DNode) => {
      if (node.id === selectedNodeId) return palette.selected;
      if (selectedNodeId && !connectedNodeIds.has(node.id)) return palette.dimNode;
      return toneColor(node, palette);
    },
    [connectedNodeIds, palette, selectedNodeId]
  );
  const isSelectedLink = useCallback(
    (link: Graph3DLink) => {
      if (!selectedNodeId) return false;
      return linkEndId(link.source) === selectedNodeId || linkEndId(link.target) === selectedNodeId;
    },
    [selectedNodeId]
  );
  const linkColor = useCallback(
    (link: Graph3DLink) => {
      if (!selectedNodeId) return palette.edge;
      return isSelectedLink(link) ? palette.activeEdge : palette.dimEdge;
    },
    [isSelectedLink, palette, selectedNodeId]
  );
  const linkWidth = useCallback(
    (link: Graph3DLink) => (isSelectedLink(link) ? 1.5 : Math.max(0.25, link.weight * 0.42)),
    [isSelectedLink]
  );
  const linkParticles = useCallback((link: Graph3DLink) => (isSelectedLink(link) ? 1 : 0), [isSelectedLink]);
  const linkArrows = useCallback((link: Graph3DLink) => (isSelectedLink(link) ? 2.2 : 0), [isSelectedLink]);

  const nodeThreeObject = useCallback(
    (node: Graph3DNode): THREE.Object3D => {
      if (!labelledNodeIds.has(node.id)) return new THREE.Object3D();
      const sprite = makeLabelSprite(truncateLabel(node.name), palette.label, palette.labelStroke);
      const radius = Math.cbrt(node.val) * 4;
      sprite.position.set(0, -(radius + 3.5), 0);
      return sprite;
    },
    [labelledNodeIds, palette]
  );

  const setGraphInstance = useCallback((instance: GraphInstance | null) => {
    graphRef.current = instance ?? undefined;
    if (!instance || initializedRef.current) return;
    initializedRef.current = true;
    const charge = instance.d3Force('charge') as { strength?: (value: number) => unknown } | undefined;
    charge?.strength?.(-105);
    const link = instance.d3Force('link') as { distance?: (value: number) => unknown } | undefined;
    link?.distance?.(38);
  }, []);

  const focusNode = useCallback(
    (id: string) => {
      let attempts = 0;
      let frame = 0;
      const tryFocus = () => {
        const node = nodeById.get(id);
        const instance = graphRef.current;
        if (node && instance && node.x !== undefined && node.y !== undefined && node.z !== undefined) {
          const length = Math.hypot(node.x, node.y, node.z || 1);
          const ratio = 1 + 105 / Math.max(length, 1);
          instance.cameraPosition(
            { x: node.x * ratio, y: node.y * ratio, z: (node.z || 1) * ratio },
            { x: node.x, y: node.y, z: node.z },
            650
          );
          return;
        }
        attempts += 1;
        if (attempts < 120) frame = window.requestAnimationFrame(tryFocus);
      };
      tryFocus();
      return () => window.cancelAnimationFrame(frame);
    },
    [nodeById]
  );

  useEffect(() => {
    if (!selectedNodeId) return;
    return focusNode(selectedNodeId);
  }, [focusNode, selectedNodeId]);

  const fitView = useCallback(() => {
    const instance = graphRef.current;
    if (!instance) return;
    autoFittedRef.current = true;
    window.cancelAnimationFrame(fitFrameRef.current);
    instance.zoomToFit(0, 54);
    const fitScale = graphData.nodes.length > 300 ? 0.74 : 0.66;
    fitFrameRef.current = window.requestAnimationFrame(() => scaleCameraDistance(instance, fitScale, 420));
  }, [graphData.nodes.length]);

  const changeZoom = useCallback((scale: number) => {
    const instance = graphRef.current;
    if (!instance) return;
    scaleCameraDistance(instance, scale, 240);
  }, []);

  useImperativeHandle(
    ref,
    () => ({
      focusNode: (id) => {
        focusNode(id);
      },
      resetView: fitView,
      zoomIn: () => changeZoom(0.78),
      zoomOut: () => changeZoom(1.28),
    }),
    [changeZoom, fitView, focusNode]
  );

  return (
    <div ref={attachContainer} className='memory-graph-scene' data-testid='memory-graph-3d-scene'>
      {!webglAvailable ? (
        <output className='memory-graph-webgl-fallback'>
          当前浏览器无法启用 WebGL。你仍可通过“浏览节点”查看和选择记忆。
        </output>
      ) : size.width > 0 && size.height > 0 ? (
        <ForceGraph3D<Graph3DNode, Graph3DLink>
          ref={setGraphInstance as never}
          width={size.width}
          height={size.height}
          graphData={graphData}
          backgroundColor={palette.background}
          showNavInfo={false}
          nodeId='id'
          nodeRelSize={4}
          nodeLabel={(node) => nodeTooltip(node)}
          nodeVal={nodeVal}
          nodeColor={nodeColor}
          nodeOpacity={0.94}
          nodeResolution={12}
          nodeThreeObject={nodeThreeObject}
          nodeThreeObjectExtend
          linkColor={linkColor}
          linkWidth={linkWidth}
          linkOpacity={0.52}
          linkDirectionalArrowLength={linkArrows}
          linkDirectionalArrowRelPos={1}
          linkDirectionalParticles={linkParticles}
          linkDirectionalParticleColor={() => palette.activeEdge}
          linkDirectionalParticleSpeed={0.006}
          linkDirectionalParticleWidth={1.2}
          warmupTicks={28}
          cooldownTicks={170}
          enableNodeDrag
          onNodeClick={(node) => {
            const sourceNode = graph.nodes.find((item) => item.id === node.id);
            if (sourceNode) onSelectNode(sourceNode);
            focusNode(node.id);
          }}
          onBackgroundClick={onClearSelection}
          onEngineStop={() => {
            if (autoFittedRef.current) return;
            fitView();
          }}
        />
      ) : null}
    </div>
  );
});

export default MemoryGraph3D;

function detectDarkMode(): boolean {
  return typeof document !== 'undefined' && document.documentElement.classList.contains('dark');
}

function linkEndId(end: string | Graph3DNode): string {
  return typeof end === 'object' ? end.id : end;
}

function scaleCameraDistance(instance: GraphInstance, scale: number, transitionMs: number): void {
  const camera = instance.camera();
  const controls = instance.controls() as { target?: THREE.Vector3 };
  const target = controls.target?.clone() ?? new THREE.Vector3();
  const next = camera.position.clone().sub(target).multiplyScalar(scale).add(target);
  instance.cameraPosition(next, target, transitionMs);
}

function toneColor(node: Graph3DNode, palette: GraphPalette): string {
  if (node.nodeType === 'event') return palette.event;
  if (['source', 'provider', 'session', 'ctx-event'].includes(node.tone)) return palette.source;
  if (['tool', 'command'].includes(node.tone)) return palette.tool;
  if (['file', 'url'].includes(node.tone)) return palette.file;
  if (['tag', 'topic', 'outcome'].includes(node.tone)) return palette.tag;
  return palette.other;
}

function makeLabelSprite(text: string, color: string, stroke: string): THREE.Sprite {
  const fontSize = 40;
  const padding = 12;
  const canvas = document.createElement('canvas');
  const context = canvas.getContext('2d');
  if (!context) return new THREE.Sprite(new THREE.SpriteMaterial({ transparent: true, opacity: 0 }));
  const font = `600 ${fontSize}px PingFang SC, system-ui, sans-serif`;
  context.font = font;
  const textWidth = Math.max(1, Math.ceil(context.measureText(text).width));
  canvas.width = textWidth + padding * 2;
  canvas.height = fontSize + padding * 2;
  context.font = font;
  context.textAlign = 'center';
  context.textBaseline = 'middle';
  context.lineJoin = 'round';
  context.lineWidth = 6;
  context.strokeStyle = stroke;
  context.strokeText(text, canvas.width / 2, canvas.height / 2);
  context.fillStyle = color;
  context.fillText(text, canvas.width / 2, canvas.height / 2);
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.minFilter = THREE.LinearFilter;
  const sprite = new THREE.Sprite(new THREE.SpriteMaterial({ map: texture, transparent: true, depthWrite: false }));
  const scale = 0.14;
  sprite.scale.set((canvas.width / canvas.height) * fontSize * scale, fontSize * scale, 1);
  return sprite;
}

function nodeTooltip(node: Graph3DNode): string {
  const kind = node.nodeType === 'event' ? '记忆' : '实体';
  return `${kind} · ${node.name}\n${node.degree} 条关联`;
}

function truncateLabel(value: string): string {
  const characters = [...value];
  return characters.length > 22 ? `${characters.slice(0, 21).join('')}…` : value;
}
