import { type MouseEventHandler, type PointerEventHandler, type RefObject, useEffect, useRef, useState } from 'react';

export interface WorkFileMarqueeRectangle {
  left: number;
  top: number;
  right: number;
  bottom: number;
  width: number;
  height: number;
}

interface WorkFileMarqueePoint {
  x: number;
  y: number;
}

type WorkFileMarqueeMode = 'replace' | 'add' | 'toggle';

interface ActiveMarquee {
  pointerId: number;
  startClient: WorkFileMarqueePoint;
  startContent: WorkFileMarqueePoint;
  currentClient: WorkFileMarqueePoint;
  initialPaths: Set<string>;
  mode: WorkFileMarqueeMode;
  selecting: boolean;
}

const DRAG_THRESHOLD = 4;
const AUTO_SCROLL_EDGE = 44;
const AUTO_SCROLL_MAX_SPEED = 18;

export function useWorkFileMarquee({
  containerRef,
  visiblePaths,
  selectedPaths,
  onSelectionChange,
}: {
  containerRef: RefObject<HTMLDivElement | null>;
  visiblePaths: readonly string[];
  selectedPaths: ReadonlySet<string>;
  onSelectionChange: (paths: readonly string[]) => void;
}) {
  const [rectangle, setRectangle] = useState<WorkFileMarqueeRectangle | null>(null);
  const [deferSelectionToolbar, setDeferSelectionToolbar] = useState(false);
  const activeRef = useRef<ActiveMarquee | null>(null);
  const visiblePathsRef = useRef(visiblePaths);
  const selectedPathsRef = useRef(selectedPaths);
  const onSelectionChangeRef = useRef(onSelectionChange);
  const suppressClickRef = useRef(false);
  const suppressClickTimerRef = useRef<number | null>(null);
  const autoScrollFrameRef = useRef<number | null>(null);
  visiblePathsRef.current = visiblePaths;
  selectedPathsRef.current = selectedPaths;
  onSelectionChangeRef.current = onSelectionChange;

  useEffect(
    () => () => {
      if (suppressClickTimerRef.current !== null) window.clearTimeout(suppressClickTimerRef.current);
      if (autoScrollFrameRef.current !== null && typeof window.cancelAnimationFrame === 'function') {
        window.cancelAnimationFrame(autoScrollFrameRef.current);
      }
    },
    []
  );

  const updateSelection = (container: HTMLDivElement, active: ActiveMarquee) => {
    const currentContent = clientPointToContent(container, active.currentClient);
    const nextRectangle = rectangleBetweenPoints(active.startContent, currentContent);
    const hitPaths = intersectingFilePaths(container, nextRectangle, visiblePathsRef.current);
    const nextPaths = marqueeSelectionPaths(visiblePathsRef.current, active.initialPaths, hitPaths, active.mode);
    setRectangle(nextRectangle);
    onSelectionChangeRef.current(nextPaths);
  };

  const runAutoScroll = () => {
    autoScrollFrameRef.current = null;
    const active = activeRef.current;
    const container = containerRef.current;
    if (!active?.selecting || !container) return;
    const bounds = container.getBoundingClientRect();
    const horizontalVelocity = marqueeAutoScrollVelocity(active.currentClient.x, bounds.left, bounds.right);
    const verticalVelocity = marqueeAutoScrollVelocity(active.currentClient.y, bounds.top, bounds.bottom);
    const maxScrollLeft = Math.max(0, container.scrollWidth - container.clientWidth);
    const maxScrollTop = Math.max(0, container.scrollHeight - container.clientHeight);
    const nextScrollLeft = Math.max(0, Math.min(maxScrollLeft, container.scrollLeft + horizontalVelocity));
    const nextScrollTop = Math.max(0, Math.min(maxScrollTop, container.scrollTop + verticalVelocity));
    if (nextScrollLeft === container.scrollLeft && nextScrollTop === container.scrollTop) return;
    container.scrollLeft = nextScrollLeft;
    container.scrollTop = nextScrollTop;
    updateSelection(container, active);
    if (typeof window.requestAnimationFrame === 'function') {
      autoScrollFrameRef.current = window.requestAnimationFrame(runAutoScroll);
    }
  };

  const scheduleAutoScroll = () => {
    if (autoScrollFrameRef.current !== null || typeof window.requestAnimationFrame !== 'function') return;
    autoScrollFrameRef.current = window.requestAnimationFrame(runAutoScroll);
  };

  const stopAutoScroll = () => {
    if (autoScrollFrameRef.current !== null && typeof window.cancelAnimationFrame === 'function') {
      window.cancelAnimationFrame(autoScrollFrameRef.current);
    }
    autoScrollFrameRef.current = null;
  };

  const onPointerDown: PointerEventHandler<HTMLDivElement> = (event) => {
    if (event.button !== 0 || event.isPrimary === false || isMarqueeExcludedTarget(event.target)) return;
    const container = containerRef.current ?? event.currentTarget;
    const clientPoint = { x: event.clientX, y: event.clientY };
    activeRef.current = {
      pointerId: event.pointerId,
      startClient: clientPoint,
      startContent: clientPointToContent(container, clientPoint),
      currentClient: clientPoint,
      initialPaths: new Set(selectedPathsRef.current),
      mode: event.metaKey || event.ctrlKey ? 'toggle' : event.shiftKey ? 'add' : 'replace',
      selecting: false,
    };
    setDeferSelectionToolbar(selectedPathsRef.current.size === 0);
    container.focus({ preventScroll: true });
    if (typeof container.setPointerCapture === 'function') container.setPointerCapture(event.pointerId);
    event.preventDefault();
  };

  const onPointerMove: PointerEventHandler<HTMLDivElement> = (event) => {
    const active = activeRef.current;
    if (!active || active.pointerId !== event.pointerId) return;
    active.currentClient = { x: event.clientX, y: event.clientY };
    if (
      !active.selecting &&
      Math.hypot(active.currentClient.x - active.startClient.x, active.currentClient.y - active.startClient.y) <
        DRAG_THRESHOLD
    ) {
      return;
    }
    active.selecting = true;
    updateSelection(containerRef.current ?? event.currentTarget, active);
    scheduleAutoScroll();
    event.preventDefault();
  };

  const finishPointer = (event: Parameters<PointerEventHandler<HTMLDivElement>>[0], cancelled: boolean) => {
    const active = activeRef.current;
    if (!active || active.pointerId !== event.pointerId) return;
    stopAutoScroll();
    const container = containerRef.current ?? event.currentTarget;
    if (cancelled) {
      if (active.selecting) onSelectionChangeRef.current([...active.initialPaths]);
    } else if (active.selecting) {
      active.currentClient = { x: event.clientX, y: event.clientY };
      updateSelection(container, active);
      suppressNextClick();
    } else if (active.mode === 'replace') {
      onSelectionChangeRef.current([]);
    }
    if (
      typeof container.hasPointerCapture === 'function' &&
      typeof container.releasePointerCapture === 'function' &&
      container.hasPointerCapture(event.pointerId)
    ) {
      container.releasePointerCapture(event.pointerId);
    }
    activeRef.current = null;
    setRectangle(null);
    setDeferSelectionToolbar(false);
  };

  const suppressNextClick = () => {
    suppressClickRef.current = true;
    if (suppressClickTimerRef.current !== null) window.clearTimeout(suppressClickTimerRef.current);
    suppressClickTimerRef.current = window.setTimeout(() => {
      suppressClickRef.current = false;
      suppressClickTimerRef.current = null;
    }, 0);
  };

  const onPointerUp: PointerEventHandler<HTMLDivElement> = (event) => finishPointer(event, false);
  const onPointerCancel: PointerEventHandler<HTMLDivElement> = (event) => finishPointer(event, true);
  const onClick: MouseEventHandler<HTMLDivElement> = (event) => {
    if (!suppressClickRef.current) return;
    suppressClickRef.current = false;
    event.preventDefault();
    event.stopPropagation();
  };

  return {
    rectangle,
    selecting: rectangle !== null,
    deferSelectionToolbar,
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerCancel,
    onClick,
  };
}

export function rectangleBetweenPoints(
  start: WorkFileMarqueePoint,
  end: WorkFileMarqueePoint
): WorkFileMarqueeRectangle {
  const left = Math.min(start.x, end.x);
  const top = Math.min(start.y, end.y);
  const right = Math.max(start.x, end.x);
  const bottom = Math.max(start.y, end.y);
  return {
    left,
    top,
    right,
    bottom,
    width: right - left,
    height: bottom - top,
  };
}

export function rectanglesIntersect(first: WorkFileMarqueeRectangle, second: WorkFileMarqueeRectangle): boolean {
  return (
    first.left < second.right && first.right > second.left && first.top < second.bottom && first.bottom > second.top
  );
}

export function marqueeSelectionPaths(
  visiblePaths: readonly string[],
  initialPaths: ReadonlySet<string>,
  hitPaths: ReadonlySet<string>,
  mode: WorkFileMarqueeMode
): string[] {
  return visiblePaths.filter((path) => {
    if (mode === 'replace') return hitPaths.has(path);
    if (mode === 'add') return initialPaths.has(path) || hitPaths.has(path);
    return hitPaths.has(path) ? !initialPaths.has(path) : initialPaths.has(path);
  });
}

export function marqueeAutoScrollVelocity(position: number, start: number, end: number): number {
  if (end <= start) return 0;
  const leadingDistance = position - start;
  if (leadingDistance < AUTO_SCROLL_EDGE) {
    const intensity = Math.min(1, Math.max(0, (AUTO_SCROLL_EDGE - leadingDistance) / AUTO_SCROLL_EDGE));
    return -AUTO_SCROLL_MAX_SPEED * intensity;
  }
  const trailingDistance = end - position;
  if (trailingDistance < AUTO_SCROLL_EDGE) {
    const intensity = Math.min(1, Math.max(0, (AUTO_SCROLL_EDGE - trailingDistance) / AUTO_SCROLL_EDGE));
    return AUTO_SCROLL_MAX_SPEED * intensity;
  }
  return 0;
}

function clientPointToContent(container: HTMLDivElement, point: WorkFileMarqueePoint): WorkFileMarqueePoint {
  const bounds = container.getBoundingClientRect();
  return {
    x: point.x - bounds.left + container.scrollLeft,
    y: point.y - bounds.top + container.scrollTop,
  };
}

function intersectingFilePaths(
  container: HTMLDivElement,
  marquee: WorkFileMarqueeRectangle,
  visiblePaths: readonly string[]
): Set<string> {
  const containerBounds = container.getBoundingClientRect();
  const hits = new Set<string>();
  const items = container.querySelectorAll<HTMLElement>('[data-work-file-index]');
  for (const item of items) {
    const index = Number(item.dataset.workFileIndex);
    const path = visiblePaths[index];
    if (!path || item.hasAttribute('disabled')) continue;
    const bounds = item.getBoundingClientRect();
    const itemRectangle: WorkFileMarqueeRectangle = {
      left: bounds.left - containerBounds.left + container.scrollLeft,
      top: bounds.top - containerBounds.top + container.scrollTop,
      right: bounds.right - containerBounds.left + container.scrollLeft,
      bottom: bounds.bottom - containerBounds.top + container.scrollTop,
      width: bounds.width,
      height: bounds.height,
    };
    if (rectanglesIntersect(marquee, itemRectangle)) hits.add(path);
  }
  return hits;
}

function isMarqueeExcludedTarget(target: EventTarget | null): boolean {
  return (
    target instanceof Element &&
    Boolean(
      target.closest(
        '[data-work-file-index], [data-work-inline-name-editor], .work-files-list-header, button, input, textarea, select, a, [contenteditable="true"]'
      )
    )
  );
}
