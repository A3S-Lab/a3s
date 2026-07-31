export type CanvasSignalVariant = "hero" | "lifecycle" | "terminal";

export interface SignalPointer {
  active: boolean;
  x: number;
  y: number;
}

export interface SignalFrame {
  activeIndex: number;
  activeSystems: readonly number[];
  animated: boolean;
  context: CanvasRenderingContext2D;
  height: number;
  itemCount: number;
  pointer: SignalPointer;
  time: number;
  variant: CanvasSignalVariant;
  width: number;
}

interface Point {
  x: number;
  y: number;
}

const ACCENT = "117, 199, 195";
const TEXT = "232, 234, 232";
const LINE = "83, 91, 88";
const TAU = Math.PI * 2;

function rgba(color: string, alpha: number) {
  return `rgba(${color}, ${alpha})`;
}

function unit(seed: number, salt: number) {
  const value = Math.sin(seed * 12.9898 + salt * 78.233) * 43758.5453;
  return value - Math.floor(value);
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(maximum, Math.max(minimum, value));
}

function drawCrosshair(
  context: CanvasRenderingContext2D,
  point: Point,
  size: number,
  alpha: number,
) {
  context.strokeStyle = rgba(ACCENT, alpha);
  context.lineWidth = 1;
  context.beginPath();
  context.moveTo(point.x - size, point.y);
  context.lineTo(point.x + size, point.y);
  context.moveTo(point.x, point.y - size);
  context.lineTo(point.x, point.y + size);
  context.stroke();
}

function drawGlow(
  context: CanvasRenderingContext2D,
  point: Point,
  radius: number,
  alpha: number,
) {
  const glow = context.createRadialGradient(
    point.x,
    point.y,
    0,
    point.x,
    point.y,
    radius,
  );
  glow.addColorStop(0, rgba(ACCENT, alpha));
  glow.addColorStop(0.34, rgba(ACCENT, alpha * 0.24));
  glow.addColorStop(1, rgba(ACCENT, 0));
  context.fillStyle = glow;
  context.fillRect(point.x - radius, point.y - radius, radius * 2, radius * 2);
}

function pointOnQuadratic(
  start: Point,
  control: Point,
  end: Point,
  progress: number,
): Point {
  const inverse = 1 - progress;
  return {
    x:
      inverse * inverse * start.x +
      2 * inverse * progress * control.x +
      progress * progress * end.x,
    y:
      inverse * inverse * start.y +
      2 * inverse * progress * control.y +
      progress * progress * end.y,
  };
}

function drawHero(frame: SignalFrame) {
  const { animated, context, height, pointer, time, width } = frame;
  const motionTime = animated ? time : 2400;
  const compact = width < 720;
  const nodeCount = compact ? 18 : 34;
  const nodes: Point[] = [];

  const washCenter = pointer.active
    ? pointer
    : {
        x: width * (0.72 + Math.sin(motionTime * 0.00018) * 0.08),
        y: height * (0.46 + Math.cos(motionTime * 0.00015) * 0.1),
      };
  const washRadius = Math.max(220, Math.min(width, height) * 0.66);
  drawGlow(context, washCenter, washRadius, compact ? 0.06 : 0.085);

  for (let index = 0; index < nodeCount; index += 1) {
    const spreadStart = compact ? 0.05 : 0.34;
    const baseX =
      width * (spreadStart + unit(index + 1, 4) * (1 - spreadStart));
    const baseY = height * (0.06 + unit(index + 1, 8) * 0.88);
    const drift = animated ? 1 : 0;
    nodes.push({
      x: baseX + Math.sin(motionTime * 0.00024 + index * 1.7) * 8 * drift,
      y: baseY + Math.cos(motionTime * 0.0002 + index * 1.3) * 6 * drift,
    });
  }

  const connectionDistance = compact ? 118 : 172;
  context.lineWidth = 0.75;
  for (let startIndex = 0; startIndex < nodes.length; startIndex += 1) {
    const start = nodes[startIndex];
    if (!start) continue;
    for (
      let endIndex = startIndex + 1;
      endIndex < nodes.length;
      endIndex += 1
    ) {
      const end = nodes[endIndex];
      if (!end) continue;
      const distance = Math.hypot(end.x - start.x, end.y - start.y);
      if (distance >= connectionDistance) continue;

      const proximity = 1 - distance / connectionDistance;
      const pointerDistance = pointer.active
        ? Math.min(
            Math.hypot(start.x - pointer.x, start.y - pointer.y),
            Math.hypot(end.x - pointer.x, end.y - pointer.y),
          )
        : Number.POSITIVE_INFINITY;
      const pointerBoost = pointerDistance < 180 ? 0.13 : 0;
      context.strokeStyle = rgba(
        pointerBoost > 0 ? ACCENT : LINE,
        0.035 + proximity * 0.12 + pointerBoost,
      );
      context.beginPath();
      context.moveTo(start.x, start.y);
      context.lineTo(end.x, end.y);
      context.stroke();
    }
  }

  context.save();
  context.globalCompositeOperation = "lighter";
  for (let index = 0; index < nodes.length; index += 1) {
    const node = nodes[index];
    if (!node) continue;
    const pulse = 0.5 + Math.sin(motionTime * 0.002 + index) * 0.5;
    context.fillStyle = rgba(
      index % 4 === 0 ? ACCENT : TEXT,
      0.24 + pulse * 0.38,
    );
    context.beginPath();
    context.arc(node.x, node.y, index % 6 === 0 ? 1.8 : 1.05, 0, TAU);
    context.fill();
    if (index % 9 === 0) drawGlow(context, node, 24, 0.12 + pulse * 0.05);
  }

  const packetCount = compact ? 3 : 7;
  for (let index = 0; index < packetCount; index += 1) {
    const start = nodes[(index * 5 + 1) % nodes.length];
    const end = nodes[(index * 9 + 7) % nodes.length];
    if (!start || !end) continue;
    const progress = animated
      ? (motionTime * (0.000045 + index * 0.000004) + unit(index, 2)) % 1
      : (index + 1) / (packetCount + 1);
    const point = {
      x: start.x + (end.x - start.x) * progress,
      y: start.y + (end.y - start.y) * progress,
    };
    context.fillStyle = rgba(ACCENT, 0.82);
    context.beginPath();
    context.arc(point.x, point.y, 1.7, 0, TAU);
    context.fill();
    drawGlow(context, point, 15, 0.18);
  }
  context.restore();

  if (pointer.active) {
    drawGlow(context, pointer, 84, 0.12);
    context.strokeStyle = rgba(ACCENT, 0.34);
    context.setLineDash([3, 5]);
    context.beginPath();
    context.arc(pointer.x, pointer.y, 29, 0, TAU);
    context.stroke();
    context.setLineDash([]);
    drawCrosshair(context, pointer, 7, 0.72);
  }
}

function drawLifecycle(frame: SignalFrame) {
  const { activeIndex, animated, context, height, itemCount, time, width } =
    frame;
  const count = Math.max(1, itemCount);
  const motionTime = animated ? time : 1800 + activeIndex * 310;
  const padding = width < 720 ? 18 : 24;
  const usableWidth = Math.max(1, width - padding * 2);
  const nodeY = Math.min(30, height * 0.24);
  const active = clamp(activeIndex, 0, count - 1);
  const points = Array.from({ length: count }, (_, index) => ({
    x: padding + (usableWidth / count) * (index + 0.5),
    y: nodeY,
  }));

  context.strokeStyle = rgba(LINE, 0.22);
  context.lineWidth = 1;
  context.beginPath();
  context.moveTo(padding, nodeY);
  context.lineTo(width - padding, nodeY);
  context.stroke();

  for (let index = 0; index < points.length; index += 1) {
    const point = points[index];
    if (!point) continue;
    const selected = index === active;
    context.strokeStyle = rgba(ACCENT, selected ? 0.58 : 0.18);
    context.beginPath();
    context.moveTo(point.x, nodeY - 8);
    context.lineTo(point.x, height - 10);
    context.stroke();

    context.fillStyle = rgba(ACCENT, selected ? 0.95 : 0.38);
    context.beginPath();
    context.arc(point.x, point.y, selected ? 2.4 : 1.2, 0, TAU);
    context.fill();

    if (selected) {
      const pulse = 12 + (Math.sin(motionTime * 0.003) + 1) * 5;
      context.strokeStyle = rgba(ACCENT, 0.28);
      context.beginPath();
      context.arc(point.x, point.y, pulse, 0, TAU);
      context.stroke();
      drawGlow(context, point, 46, 0.18);
    }
  }

  context.save();
  context.globalCompositeOperation = "lighter";
  const packetCount = Math.min(6, Math.max(3, count - 1));
  for (let index = 0; index < packetCount; index += 1) {
    const progress = animated
      ? (motionTime * (0.00011 + index * 0.000009) + index / packetCount) % 1
      : (index + 1) / (packetCount + 1);
    const x = padding + usableWidth * progress;
    const amplitude = 5 + (index % 2) * 3;
    const y = nodeY + Math.sin(progress * TAU * 2 + index) * amplitude;
    context.fillStyle = rgba(ACCENT, 0.82);
    context.fillRect(x - 2.5, y - 1, 5, 2);
    drawGlow(context, { x, y }, 18, 0.14);
  }
  context.restore();

  const sweep = animated ? (motionTime * 0.00009) % 1 : (active + 0.5) / count;
  const sweepX = padding + usableWidth * sweep;
  const sweepGradient = context.createLinearGradient(
    sweepX - 70,
    0,
    sweepX + 12,
    0,
  );
  sweepGradient.addColorStop(0, rgba(ACCENT, 0));
  sweepGradient.addColorStop(1, rgba(ACCENT, 0.08));
  context.fillStyle = sweepGradient;
  context.fillRect(sweepX - 70, 0, 82, height);
}

function drawTerminal(frame: SignalFrame) {
  const { activeIndex, activeSystems, animated, context, height, time, width } =
    frame;
  const motionTime = animated ? time : 2200 + activeIndex * 430;
  const systemCount = 5;
  const sourceY = height - 26;
  const active = new Set(activeSystems);
  const hub = {
    x: width * (width < 620 ? 0.72 : 0.83),
    y: 58 + (activeIndex % 3) * 26,
  };

  const scanProgress = animated
    ? (motionTime * 0.000075) % 1
    : (activeIndex + 1) / 8;
  const scanX = width * scanProgress;
  const scan = context.createLinearGradient(scanX - 54, 0, scanX + 2, 0);
  scan.addColorStop(0, rgba(ACCENT, 0));
  scan.addColorStop(1, rgba(ACCENT, 0.075));
  context.fillStyle = scan;
  context.fillRect(scanX - 54, 0, 56, height);

  context.save();
  context.globalCompositeOperation = "lighter";
  for (let index = 0; index < systemCount; index += 1) {
    const source = {
      x: ((index + 0.5) / systemCount) * width,
      y: sourceY,
    };
    const enabled = active.has(index);
    const control = {
      x: source.x + (hub.x - source.x) * 0.46,
      y: Math.min(source.y - 52, hub.y + 84 + index * 5),
    };

    context.strokeStyle = rgba(enabled ? ACCENT : LINE, enabled ? 0.19 : 0.055);
    context.lineWidth = enabled ? 1 : 0.7;
    context.beginPath();
    context.moveTo(source.x, source.y);
    context.quadraticCurveTo(control.x, control.y, hub.x, hub.y);
    context.stroke();

    context.fillStyle = rgba(enabled ? ACCENT : LINE, enabled ? 0.7 : 0.22);
    context.beginPath();
    context.arc(source.x, source.y, enabled ? 1.8 : 1, 0, TAU);
    context.fill();

    if (!enabled) continue;
    const progress = animated
      ? (motionTime * (0.000095 + index * 0.00001) + index * 0.17) % 1
      : 0.34 + index * 0.09;
    const packet = pointOnQuadratic(source, control, hub, progress % 1);
    context.fillStyle = rgba(ACCENT, 0.92);
    context.beginPath();
    context.arc(packet.x, packet.y, 1.65, 0, TAU);
    context.fill();
    drawGlow(context, packet, 17, 0.15);
  }

  context.fillStyle = rgba(ACCENT, 0.9);
  context.beginPath();
  context.arc(hub.x, hub.y, 2.2, 0, TAU);
  context.fill();
  drawGlow(context, hub, 34, 0.14);
  context.restore();

  const waveStart = Math.max(24, width * 0.5);
  const waveWidth = Math.max(1, width - waveStart - 20);
  const waveY = Math.min(height - 70, 180 + (activeIndex % 3) * 24);
  context.strokeStyle = rgba(ACCENT, 0.1);
  context.lineWidth = 1;
  context.beginPath();
  for (let offset = 0; offset <= waveWidth; offset += 4) {
    const progress = offset / waveWidth;
    const amplitude = 5 + active.size * 1.4;
    const y =
      waveY +
      Math.sin(progress * TAU * 3 + motionTime * 0.0014) *
        amplitude *
        Math.sin(progress * Math.PI);
    if (offset === 0) context.moveTo(waveStart, y);
    else context.lineTo(waveStart + offset, y);
  }
  context.stroke();

  drawCrosshair(context, hub, 5, 0.34);
}

export function drawSignalFrame(frame: SignalFrame) {
  const { context, height, variant, width } = frame;
  context.clearRect(0, 0, width, height);
  if (width <= 1 || height <= 1) return;

  if (variant === "hero") drawHero(frame);
  else if (variant === "lifecycle") drawLifecycle(frame);
  else drawTerminal(frame);
}
