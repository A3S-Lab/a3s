'use client';

import { useEffect, useId, useRef } from 'react';
import {
  ArrowsClockwise,
  Check,
  CloudArrowUp,
  FolderOpen,
  GlobeHemisphereWest,
  MagnifyingGlass,
  Robot,
  User,
  UsersThree,
} from '@phosphor-icons/react';
import { A3SMark } from '@/components/home/a3s-mark';
import {
  drawGlobalWorkspaceScene,
  type WorkspaceSceneLabels,
} from '@/components/home/global-workspace-renderer';
import { workspaceSceneCopy } from '@/components/home/global-workspace-scene-data';
import type { Lang } from '@/components/home/home-content';

const maximumPixelRatio = 2;

export function GlobalWorkspaceScene({ lang }: { lang: Lang }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const figureRef = useRef<HTMLElement>(null);
  const titleId = useId();
  const descriptionId = useId();
  const copy = workspaceSceneCopy[lang];

  useEffect(() => {
    const canvas = canvasRef.current;
    const figure = figureRef.current;
    const context = canvas?.getContext('2d');
    if (!canvas || !figure || !context) return undefined;

    const reducedMotionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
    const labels: WorkspaceSceneLabels = {
      cloud: copy.cloud,
      edge: copy.edge,
      sync: copy.sync,
    };
    let animationFrame: number | undefined;
    let elapsed = 0;
    let height = 0;
    let intersecting = false;
    let lastFrame = performance.now();
    let pointerX = 0;
    let pointerY = 0;
    let reducedMotion = reducedMotionQuery.matches;
    let targetPointerX = 0;
    let targetPointerY = 0;
    let width = 0;

    const renderFrame = () => {
      if (width === 0 || height === 0) return;
      const pixelRatio = Math.min(window.devicePixelRatio || 1, maximumPixelRatio);
      context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
      drawGlobalWorkspaceScene(context, {
        height,
        labels,
        pointerX,
        pointerY,
        reducedMotion,
        time: elapsed,
        width,
      });
    };

    const resize = () => {
      const bounds = canvas.getBoundingClientRect();
      const nextWidth = Math.max(Math.round(bounds.width), 1);
      const nextHeight = Math.max(Math.round(bounds.height), 1);
      const pixelRatio = Math.min(window.devicePixelRatio || 1, maximumPixelRatio);

      if (
        nextWidth !== width
        || nextHeight !== height
        || canvas.width !== Math.round(nextWidth * pixelRatio)
        || canvas.height !== Math.round(nextHeight * pixelRatio)
      ) {
        width = nextWidth;
        height = nextHeight;
        canvas.width = Math.round(width * pixelRatio);
        canvas.height = Math.round(height * pixelRatio);
      }
      renderFrame();
    };

    const tick = (now: number) => {
      const delta = Math.min((now - lastFrame) / 1_000, 0.05);
      lastFrame = now;
      elapsed += delta;
      pointerX += (targetPointerX - pointerX) * 0.055;
      pointerY += (targetPointerY - pointerY) * 0.055;
      renderFrame();
      animationFrame = window.requestAnimationFrame(tick);
    };

    const syncPlayback = () => {
      const shouldAnimate = intersecting
        && document.visibilityState === 'visible'
        && !reducedMotion;

      figure.classList.toggle('is-motion-active', shouldAnimate);

      if (shouldAnimate && animationFrame === undefined) {
        lastFrame = performance.now();
        animationFrame = window.requestAnimationFrame(tick);
      } else if (!shouldAnimate && animationFrame !== undefined) {
        window.cancelAnimationFrame(animationFrame);
        animationFrame = undefined;
        renderFrame();
      } else if (!shouldAnimate) {
        renderFrame();
      }
    };

    const handlePointerMove = (event: PointerEvent) => {
      if (reducedMotion) return;
      const bounds = figure.getBoundingClientRect();
      targetPointerX = ((event.clientX - bounds.left) / bounds.width * 2) - 1;
      targetPointerY = ((event.clientY - bounds.top) / bounds.height * 2) - 1;
    };
    const handlePointerLeave = () => {
      targetPointerX = 0;
      targetPointerY = 0;
    };
    const handleMotionPreference = () => {
      reducedMotion = reducedMotionQuery.matches;
      if (reducedMotion) {
        pointerX = 0;
        pointerY = 0;
        targetPointerX = 0;
        targetPointerY = 0;
      }
      syncPlayback();
    };
    const handleVisibility = () => syncPlayback();
    const intersectionObserver = new IntersectionObserver(([entry]) => {
      intersecting = entry.isIntersecting;
      syncPlayback();
    }, { threshold: 0.12 });
    const resizeObserver = new ResizeObserver(resize);

    resize();
    intersectionObserver.observe(figure);
    resizeObserver.observe(canvas);
    figure.addEventListener('pointermove', handlePointerMove, { passive: true });
    figure.addEventListener('pointerleave', handlePointerLeave);
    document.addEventListener('visibilitychange', handleVisibility);
    reducedMotionQuery.addEventListener('change', handleMotionPreference);

    return () => {
      if (animationFrame !== undefined) window.cancelAnimationFrame(animationFrame);
      figure.classList.remove('is-motion-active');
      intersectionObserver.disconnect();
      resizeObserver.disconnect();
      figure.removeEventListener('pointermove', handlePointerMove);
      figure.removeEventListener('pointerleave', handlePointerLeave);
      document.removeEventListener('visibilitychange', handleVisibility);
      reducedMotionQuery.removeEventListener('change', handleMotionPreference);
    };
  }, [copy.cloud, copy.edge, copy.sync]);

  return (
    <figure
      aria-describedby={descriptionId}
      aria-labelledby={titleId}
      className="a3s-global-workspace"
      ref={figureRef}
      role="img"
    >
      <figcaption className="a3s-visually-hidden" id={titleId}>{copy.accessibleLabel}</figcaption>
      <p className="a3s-visually-hidden" id={descriptionId}>{copy.accessibleDescription}</p>
      <div aria-hidden="true" className="a3s-global-workspace__visual">
        <div className="a3s-global-workspace__app">
          <header className="a3s-global-workspace__appbar">
            <span className="a3s-global-workspace__brand">
              <A3SMark />
              <strong>{copy.title}</strong>
            </span>
            <span className="a3s-global-workspace__search">
              <MagnifyingGlass />
              {copy.search}
              <kbd>⌘ K</kbd>
            </span>
            <span className="a3s-global-workspace__online">
              <i /> {copy.mode}
            </span>
          </header>

          <div className="a3s-global-workspace__layout">
            <aside className="a3s-global-workspace__sidebar">
              <nav>
                <span className="is-active"><GlobeHemisphereWest />{copy.global}</span>
                <span><UsersThree />{copy.teams}</span>
                <span><FolderOpen />{copy.workspaceNav}</span>
                <span><ArrowsClockwise />{copy.syncNav}</span>
              </nav>
              <div className="a3s-global-workspace__regions">
                <small>{copy.regions}</small>
                <span><i />SFO</span>
                <span><i />LDN</span>
                <span><i />SHA</span>
              </div>
              <footer><User /><span>Human</span><Robot /><span>Agent</span></footer>
            </aside>

            <section className="a3s-global-workspace__stage">
              <canvas className="a3s-global-workspace__canvas" ref={canvasRef} />
              <div className="a3s-global-workspace__core">
                <div className="a3s-global-workspace__core-card">
                  <A3SMark className="a3s-global-workspace__mark" />
                  <strong>A3S OS</strong>
                  <small>{copy.workspace}</small>
                </div>
              </div>

              <article className="a3s-global-workspace__handoff">
                <header>
                  <span><Robot /><b>{copy.handoffTitle}</b><small>{copy.handoffMeta}</small></span>
                  <em><Check />{copy.handoffStatus}</em>
                </header>
                <ol>
                  {copy.handoffSteps.map((step, index) => (
                    <li key={step}>
                      <i>{index === 0 ? <User /> : index === 1 ? <Robot /> : <CloudArrowUp />}</i>
                      <span>{step}</span>
                      <Check />
                    </li>
                  ))}
                </ol>
              </article>

              <ul className="a3s-global-workspace__legend">
                <li>
                  <span><User weight="duotone" /><Robot weight="duotone" /></span>
                  <b>{copy.human} + {copy.agent}</b>
                </li>
                <li>
                  <FolderOpen weight="duotone" />
                  <b>{copy.workspace}</b>
                </li>
                <li>
                  <CloudArrowUp weight="duotone" />
                  <b>{copy.edgeCloud}</b>
                </li>
              </ul>
            </section>
          </div>
        </div>
      </div>
    </figure>
  );
}
