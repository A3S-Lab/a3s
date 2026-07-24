import { type RefObject, type UIEventHandler, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';

const BOTTOM_THRESHOLD = 96;
const FOLLOW_SETTLE_THRESHOLD = 0.75;
const FOLLOW_MIN_STEP = 2;
const FOLLOW_MAX_STEP = 140;
const FOLLOW_RESPONSE = 0.32;

interface StreamFollowOptions {
  revision: string;
  sessionId: string | null;
  streaming: boolean;
}

interface StreamFollowController {
  contentRef: RefObject<HTMLDivElement | null>;
  jumpToLatest: () => void;
  onScroll: UIEventHandler<HTMLDivElement>;
  scrollRef: RefObject<HTMLDivElement | null>;
  showLatest: boolean;
}

function nextStreamFollowScrollTop(current: number, target: number): number {
  const distance = target - current;
  if (Math.abs(distance) <= FOLLOW_SETTLE_THRESHOLD) return target;
  const step = Math.min(FOLLOW_MAX_STEP, Math.max(FOLLOW_MIN_STEP, Math.abs(distance) * FOLLOW_RESPONSE));
  return current + Math.sign(distance) * Math.min(Math.abs(distance), step);
}

export function useStreamFollow({ revision, sessionId, streaming }: StreamFollowOptions): StreamFollowController {
  const scrollRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const frameRef = useRef<number | null>(null);
  const followFrameRef = useRef<FrameRequestCallback>(() => undefined);
  const stickToBottomRef = useRef(true);
  const programmaticScrollTopRef = useRef(0);
  const streamingRef = useRef(streaming);
  const previousStreamingRef = useRef(streaming);
  const [showLatest, setShowLatest] = useState(false);
  streamingRef.current = streaming;

  const cancelFollowFrame = useCallback(() => {
    if (frameRef.current === null) return;
    cancelFrame(frameRef.current);
    frameRef.current = null;
  }, []);

  const writeScrollTop = useCallback((element: HTMLDivElement, top: number) => {
    programmaticScrollTopRef.current = top;
    element.scrollTop = top;
  }, []);

  followFrameRef.current = () => {
    frameRef.current = null;
    const element = scrollRef.current;
    if (!element || !stickToBottomRef.current) return;
    const target = bottomScrollTop(element);
    const next = nextStreamFollowScrollTop(element.scrollTop, target);
    writeScrollTop(element, next);
    setShowLatest(false);
    if (next !== target) frameRef.current = requestFrame(followFrameRef.current);
  };

  const followLatest = useCallback(
    (animate: boolean) => {
      const element = scrollRef.current;
      if (!element || !stickToBottomRef.current) return;
      if (!animate || prefersReducedMotion()) {
        cancelFollowFrame();
        writeScrollTop(element, bottomScrollTop(element));
        setShowLatest(false);
        return;
      }
      if (frameRef.current === null) frameRef.current = requestFrame(followFrameRef.current);
    },
    [cancelFollowFrame, writeScrollTop]
  );

  useLayoutEffect(() => {
    stickToBottomRef.current = true;
    previousStreamingRef.current = streamingRef.current;
    cancelFollowFrame();
    setShowLatest(false);
    const element = scrollRef.current;
    if (element) writeScrollTop(element, bottomScrollTop(element));
  }, [cancelFollowFrame, sessionId, writeScrollTop]);

  useLayoutEffect(() => {
    const animate = streaming || previousStreamingRef.current;
    previousStreamingRef.current = streaming;
    followLatest(animate);
  }, [followLatest, revision, streaming]);

  useEffect(() => {
    const content = contentRef.current;
    if (!content || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() => {
      followLatest(streamingRef.current);
    });
    observer.observe(content);
    return () => observer.disconnect();
  }, [followLatest, sessionId]);

  useEffect(() => cancelFollowFrame, [cancelFollowFrame]);

  const onScroll = useCallback<UIEventHandler<HTMLDivElement>>(
    (event) => {
      const element = event.currentTarget;
      const nearBottom = bottomScrollTop(element) - element.scrollTop < BOTTOM_THRESHOLD;
      const followedPosition = Math.abs(element.scrollTop - programmaticScrollTopRef.current) <= 1;
      if (frameRef.current !== null && followedPosition) {
        setShowLatest(false);
        return;
      }
      stickToBottomRef.current = nearBottom;
      if (!nearBottom) cancelFollowFrame();
      setShowLatest(!nearBottom);
    },
    [cancelFollowFrame]
  );

  const jumpToLatest = useCallback(() => {
    stickToBottomRef.current = true;
    setShowLatest(false);
    followLatest(true);
  }, [followLatest]);

  return {
    contentRef,
    jumpToLatest,
    onScroll,
    scrollRef,
    showLatest,
  };
}

function bottomScrollTop(element: HTMLDivElement): number {
  return Math.max(0, element.scrollHeight - element.clientHeight);
}

function prefersReducedMotion(): boolean {
  return typeof window.matchMedia === 'function' && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

function requestFrame(callback: FrameRequestCallback): number {
  if (typeof window.requestAnimationFrame === 'function') return window.requestAnimationFrame(callback);
  return window.setTimeout(() => callback(performance.now()), 16);
}

function cancelFrame(frame: number): void {
  if (typeof window.cancelAnimationFrame === 'function') window.cancelAnimationFrame(frame);
  else window.clearTimeout(frame);
}
