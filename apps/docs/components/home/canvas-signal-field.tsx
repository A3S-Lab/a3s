"use client";

import { useEffect, useRef } from "react";
import {
  drawSignalFrame,
  type CanvasSignalVariant,
  type SignalPointer,
} from "./canvas-signal-renderers";

interface CanvasSignalFieldProps {
  activeIndex?: number;
  activeSystems?: readonly number[];
  className?: string;
  itemCount?: number;
  playing?: boolean;
  variant: CanvasSignalVariant;
}

interface CanvasSettings {
  activeIndex: number;
  activeSystems: readonly number[];
  itemCount: number;
  playing: boolean;
  variant: CanvasSignalVariant;
}

const EMPTY_SYSTEMS: readonly number[] = [];

export function CanvasSignalField({
  activeIndex = 0,
  activeSystems = EMPTY_SYSTEMS,
  className,
  itemCount = 1,
  playing = true,
  variant,
}: CanvasSignalFieldProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const invalidateRef = useRef<(() => void) | null>(null);
  const settingsRef = useRef<CanvasSettings>({
    activeIndex,
    activeSystems,
    itemCount,
    playing,
    variant,
  });
  settingsRef.current = {
    activeIndex,
    activeSystems,
    itemCount,
    playing,
    variant,
  };
  const activeSystemsKey = activeSystems.join(",");

  useEffect(() => {
    invalidateRef.current?.();
  }, [activeIndex, activeSystemsKey, itemCount, playing, variant]);

  useEffect(() => {
    const currentCanvas = canvasRef.current;
    if (!currentCanvas) return;
    const canvas: HTMLCanvasElement = currentCanvas;

    const currentContext = canvas.getContext("2d", { alpha: true });
    if (!currentContext) return;
    const context: CanvasRenderingContext2D = currentContext;

    const motionPreference = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    );
    const compactPreference = window.matchMedia(
      "(max-width: 720px), (pointer: coarse)",
    );
    const pointer: SignalPointer = { active: false, x: 0, y: 0 };
    let width = 0;
    let height = 0;
    let reducedMotion = motionPreference.matches;
    let visible = false;
    let pageVisible = document.visibilityState === "visible";
    let animationFrame = 0;
    let lastFrame = 0;
    const tracksPointer = settingsRef.current.variant === "hero";

    function resizeCanvas() {
      const bounds = canvas.getBoundingClientRect();
      width = Math.max(1, bounds.width);
      height = Math.max(1, bounds.height);
      const pixelRatio = Math.min(window.devicePixelRatio || 1, 1.5);
      const pixelWidth = Math.max(1, Math.round(width * pixelRatio));
      const pixelHeight = Math.max(1, Math.round(height * pixelRatio));
      if (canvas.width !== pixelWidth || canvas.height !== pixelHeight) {
        canvas.width = pixelWidth;
        canvas.height = pixelHeight;
      }
      context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
      draw(performance.now(), false);
      start();
    }

    function draw(time: number, animated: boolean) {
      const settings = settingsRef.current;
      drawSignalFrame({
        ...settings,
        animated,
        context,
        height,
        pointer,
        time,
        width,
      });
      canvas.dataset.canvasMode = animated ? "animated" : "static";
      canvas.dataset.canvasReady = "true";
    }

    function shouldAnimate() {
      return (
        visible && pageVisible && !reducedMotion && settingsRef.current.playing
      );
    }

    function tick(time: number) {
      animationFrame = 0;
      if (!shouldAnimate()) {
        draw(time, false);
        return;
      }

      const frameInterval = compactPreference.matches ? 1000 / 30 : 1000 / 50;
      if (time - lastFrame >= frameInterval) {
        draw(time, true);
        lastFrame = time;
      }
      animationFrame = window.requestAnimationFrame(tick);
    }

    function start() {
      if (!animationFrame && shouldAnimate()) {
        animationFrame = window.requestAnimationFrame(tick);
      }
    }

    function stop() {
      if (!animationFrame) return;
      window.cancelAnimationFrame(animationFrame);
      animationFrame = 0;
    }

    function invalidate() {
      draw(performance.now(), shouldAnimate());
      start();
    }

    function handlePointerMove(event: PointerEvent) {
      if (settingsRef.current.variant !== "hero") return;
      const bounds = canvas.getBoundingClientRect();
      const inside =
        event.clientX >= bounds.left &&
        event.clientX <= bounds.right &&
        event.clientY >= bounds.top &&
        event.clientY <= bounds.bottom;
      pointer.active = inside && event.pointerType !== "touch";
      if (pointer.active) {
        pointer.x = event.clientX - bounds.left;
        pointer.y = event.clientY - bounds.top;
      }
      if (!shouldAnimate()) invalidate();
    }

    function handlePointerExit() {
      if (!pointer.active) return;
      pointer.active = false;
      if (!shouldAnimate()) invalidate();
    }

    function handleVisibilityChange() {
      pageVisible = document.visibilityState === "visible";
      if (pageVisible) start();
      else stop();
    }

    function handleMotionChange() {
      reducedMotion = motionPreference.matches;
      if (reducedMotion) {
        stop();
        draw(performance.now(), false);
      } else {
        start();
      }
    }

    const resizeObserver = new ResizeObserver(resizeCanvas);
    resizeObserver.observe(canvas);

    const intersectionObserver = new IntersectionObserver(
      ([entry]) => {
        visible = entry?.isIntersecting ?? false;
        if (visible) start();
        else {
          stop();
          draw(performance.now(), false);
        }
      },
      { rootMargin: "120px 0px" },
    );
    intersectionObserver.observe(canvas);

    if (tracksPointer) {
      window.addEventListener("pointermove", handlePointerMove, {
        passive: true,
      });
      window.addEventListener("scroll", handlePointerExit, { passive: true });
      window.addEventListener("blur", handlePointerExit);
    }
    document.addEventListener("visibilitychange", handleVisibilityChange);
    motionPreference.addEventListener("change", handleMotionChange);
    invalidateRef.current = invalidate;
    resizeCanvas();

    return () => {
      stop();
      resizeObserver.disconnect();
      intersectionObserver.disconnect();
      if (tracksPointer) {
        window.removeEventListener("pointermove", handlePointerMove);
        window.removeEventListener("scroll", handlePointerExit);
        window.removeEventListener("blur", handlePointerExit);
      }
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      motionPreference.removeEventListener("change", handleMotionChange);
      invalidateRef.current = null;
    };
  }, []);

  const classes = [
    "a3s-canvas-field",
    `a3s-canvas-field--${variant}`,
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <canvas
      aria-hidden="true"
      className={classes}
      data-canvas-field={variant}
      ref={canvasRef}
    />
  );
}
