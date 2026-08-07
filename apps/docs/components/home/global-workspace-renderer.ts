import { organizationNodes } from './global-workspace-scene-data';

interface Point2D {
  x: number;
  y: number;
}

interface Point3D extends Point2D {
  z: number;
}

interface ProjectedNode extends Point3D {
  code: string;
  phase: number;
}

export interface WorkspaceSceneLabels {
  cloud: string;
  edge: string;
  sync: string;
}

export interface WorkspaceSceneFrame {
  height: number;
  labels: WorkspaceSceneLabels;
  pointerX: number;
  pointerY: number;
  reducedMotion: boolean;
  time: number;
  width: number;
}

const TAU = Math.PI * 2;
const STATIC_TIME = 10.4;

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(Math.max(value, minimum), maximum);
}

function fractional(value: number) {
  return value - Math.floor(value);
}

function quadraticPoint(start: Point2D, control: Point2D, end: Point2D, progress: number): Point2D {
  const inverse = 1 - progress;
  return {
    x: (inverse * inverse * start.x) + (2 * inverse * progress * control.x) + (progress * progress * end.x),
    y: (inverse * inverse * start.y) + (2 * inverse * progress * control.y) + (progress * progress * end.y),
  };
}

function geoPoint(latitude: number, longitude: number, rotation: number, pitch: number): Point3D {
  const latitudeRadians = latitude * Math.PI / 180;
  const longitudeRadians = (longitude * Math.PI / 180) + rotation;
  const latitudeCosine = Math.cos(latitudeRadians);
  const x = latitudeCosine * Math.sin(longitudeRadians);
  const y = -Math.sin(latitudeRadians);
  const z = latitudeCosine * Math.cos(longitudeRadians);
  const pitchCosine = Math.cos(pitch);
  const pitchSine = Math.sin(pitch);

  return {
    x,
    y: (y * pitchCosine) - (z * pitchSine),
    z: (y * pitchSine) + (z * pitchCosine),
  };
}

function project(point: Point3D, center: Point2D, radius: number): Point3D {
  return {
    x: center.x + (point.x * radius),
    y: center.y + (point.y * radius),
    z: point.z,
  };
}

function strokeDepthCurve(
  context: CanvasRenderingContext2D,
  points: Point3D[],
  visible: (point: Point3D) => boolean,
  strokeStyle: string,
  lineWidth: number,
) {
  let drawing = false;
  context.beginPath();

  for (const point of points) {
    if (!visible(point)) {
      drawing = false;
      continue;
    }

    if (drawing) context.lineTo(point.x, point.y);
    else context.moveTo(point.x, point.y);
    drawing = true;
  }

  context.lineWidth = lineWidth;
  context.strokeStyle = strokeStyle;
  context.stroke();
}

function drawBackdrop(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  center: Point2D,
  radius: number,
) {
  context.clearRect(0, 0, width, height);

  const halo = context.createRadialGradient(
    center.x,
    center.y,
    radius * 0.12,
    center.x,
    center.y,
    radius * 1.72,
  );
  halo.addColorStop(0, 'rgba(18, 100, 255, 0.12)');
  halo.addColorStop(0.42, 'rgba(112, 87, 201, 0.04)');
  halo.addColorStop(1, 'rgba(18, 100, 255, 0)');
  context.fillStyle = halo;
  context.fillRect(0, 0, width, height);

  context.save();
  context.translate(center.x, center.y + (radius * 1.05));
  context.scale(1, 0.24);
  const shadow = context.createRadialGradient(0, 0, radius * 0.1, 0, 0, radius * 0.95);
  shadow.addColorStop(0, 'rgba(36, 76, 137, 0.12)');
  shadow.addColorStop(0.58, 'rgba(18, 100, 255, 0.06)');
  shadow.addColorStop(1, 'rgba(18, 100, 255, 0)');
  context.fillStyle = shadow;
  context.beginPath();
  context.arc(0, 0, radius, 0, TAU);
  context.fill();
  context.restore();
}

function drawNeuralLattice(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  time: number,
  pointerX: number,
  pointerY: number,
) {
  const points = Array.from({ length: 26 }, (_, index) => {
    const seedX = fractional(Math.sin((index + 3) * 81.719) * 37_914.117);
    const seedY = fractional(Math.sin((index + 7) * 53.331) * 21_437.713);
    const depth = 0.24 + (fractional(Math.sin((index + 11) * 29.177) * 8_319.337) * 0.76);
    return {
      depth,
      x: (seedX * width)
        + (Math.sin((time * (0.045 + depth * 0.035)) + index) * (4 + depth * 7))
        + (pointerX * (1 - depth) * 18),
      y: (seedY * height)
        + (Math.cos((time * (0.04 + depth * 0.03)) + index) * (3 + depth * 5))
        + (pointerY * (1 - depth) * 13),
    };
  });
  const connectionLimit = Math.min(width * 0.21, 156);

  for (let leftIndex = 0; leftIndex < points.length; leftIndex += 1) {
    const left = points[leftIndex];
    for (let rightIndex = leftIndex + 1; rightIndex < points.length; rightIndex += 1) {
      if ((leftIndex + rightIndex) % 3 !== 0) continue;
      const right = points[rightIndex];
      const distance = Math.hypot(right.x - left.x, right.y - left.y);
      if (distance > connectionLimit) continue;
      const strength = 1 - (distance / connectionLimit);
      const alpha = strength * Math.min(left.depth, right.depth) * 0.16;
      const gradient = context.createLinearGradient(left.x, left.y, right.x, right.y);
      gradient.addColorStop(0, `rgba(18, 100, 255, ${alpha * 0.38})`);
      gradient.addColorStop(1, `rgba(112, 87, 201, ${alpha * 0.72})`);
      context.lineWidth = 0.7;
      context.strokeStyle = gradient;
      context.beginPath();
      context.moveTo(left.x, left.y);
      context.lineTo(right.x, right.y);
      context.stroke();

      if ((leftIndex * 7 + rightIndex) % 9 === 0) {
        const progress = fractional((time * 0.08) + (leftIndex * 0.17));
        const pulseX = left.x + ((right.x - left.x) * progress);
        const pulseY = left.y + ((right.y - left.y) * progress);
        context.fillStyle = `rgba(18, 100, 255, ${alpha * 3.4})`;
        context.beginPath();
        context.arc(pulseX, pulseY, 1.35, 0, TAU);
        context.fill();
      }
    }
  }

  for (const [index, point] of points.entries()) {
    const flicker = 0.72 + (Math.sin((time * 0.75) + index) * 0.28);
    const alpha = (0.1 + (point.depth * 0.31)) * flicker;
    context.save();
    context.shadowBlur = point.depth > 0.72 ? 8 : 0;
    context.shadowColor = 'rgba(18, 100, 255, 0.28)';
    context.fillStyle = `rgba(71, 126, 211, ${alpha * 0.72})`;
    context.beginPath();
    context.arc(point.x, point.y, 0.65 + (point.depth * 1.1), 0, TAU);
    context.fill();
    context.restore();
  }
}

function drawStateField(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  time: number,
) {
  for (let index = 0; index < 52; index += 1) {
    const seedX = fractional(Math.sin((index + 1) * 91.731) * 43_758.5453);
    const seedY = fractional(Math.sin((index + 1) * 47.193) * 24_634.6345);
    const depth = fractional(Math.sin((index + 1) * 17.117) * 9_173.127);
    const x = (seedX * width) + (Math.sin(time * (0.08 + depth * 0.04) + index) * (3 + depth * 6));
    const y = (seedY * height) + (Math.cos(time * (0.07 + depth * 0.03) + index) * (2 + depth * 4));
    const twinkle = 0.72 + (Math.sin((time * (0.55 + depth * 0.3)) + index) * 0.28);
    const alpha = (0.06 + (depth * 0.2)) * twinkle;
    const size = 0.45 + (depth * 1.15);

    context.fillStyle = `rgba(88, 122, 174, ${alpha * 0.62})`;
    context.beginPath();
    context.arc(x, y, size, 0, TAU);
    context.fill();

    if (index % 11 === 0) {
      context.strokeStyle = `rgba(18, 100, 255, ${alpha * 0.3})`;
      context.lineWidth = 0.6;
      context.beginPath();
      context.moveTo(x - (5 + depth * 5), y);
      context.lineTo(x + (5 + depth * 5), y);
      context.stroke();
    }
  }
}

function drawOrbitSystem(
  context: CanvasRenderingContext2D,
  center: Point2D,
  radius: number,
  time: number,
) {
  context.save();
  context.translate(center.x, center.y);
  context.rotate(-0.16);
  context.setLineDash([4, 8]);
  context.lineWidth = 1;
  context.strokeStyle = 'rgba(95, 124, 168, 0.14)';
  context.beginPath();
  context.ellipse(0, 0, radius * 1.28, radius * 0.39, 0, Math.PI, TAU);
  context.stroke();
  context.setLineDash([]);
  context.lineWidth = 1.35;
  context.strokeStyle = 'rgba(18, 100, 255, 0.34)';
  context.beginPath();
  context.ellipse(0, 0, radius * 1.28, radius * 0.39, 0, 0, Math.PI);
  context.stroke();

  for (const offset of [0, 0.48]) {
    const progress = (time * 0.075 + offset) % 1;
    const angle = progress * TAU;
    const particleAlpha = clamp(0.42 + (Math.sin(angle) * 0.3), 0.16, 0.76);

    context.lineWidth = 1.6;
    context.strokeStyle = `rgba(18, 100, 255, ${particleAlpha * 0.42})`;
    context.beginPath();
    context.ellipse(
      0,
      0,
      radius * 1.28,
      radius * 0.39,
      0,
      angle - 0.24,
      angle + 0.02,
    );
    context.stroke();

    context.save();
    context.shadowBlur = 13;
    context.shadowColor = 'rgba(18, 100, 255, 0.36)';
    for (let trail = 4; trail >= 0; trail -= 1) {
      const trailAngle = angle - (trail * 0.026);
      const x = Math.cos(trailAngle) * radius * 1.28;
      const y = Math.sin(trailAngle) * radius * 0.39;
      const trailAlpha = particleAlpha * (1 - (trail * 0.17));
      context.fillStyle = `rgba(18, 100, 255, ${trailAlpha * 0.86})`;
      context.beginPath();
      context.arc(x, y, trail === 0 ? 2.7 : 1.2, 0, TAU);
      context.fill();
    }
    context.restore();
  }
  context.restore();
}

function drawGlobe(
  context: CanvasRenderingContext2D,
  center: Point2D,
  radius: number,
  rotation: number,
  pitch: number,
  time: number,
) {
  const atmosphere = context.createRadialGradient(
    center.x,
    center.y,
    radius * 0.74,
    center.x,
    center.y,
    radius * 1.18,
  );
  atmosphere.addColorStop(0, 'rgba(18, 100, 255, 0)');
  atmosphere.addColorStop(0.68, 'rgba(18, 100, 255, 0.025)');
  atmosphere.addColorStop(0.86, 'rgba(18, 100, 255, 0.14)');
  atmosphere.addColorStop(1, 'rgba(18, 100, 255, 0)');
  context.fillStyle = atmosphere;
  context.beginPath();
  context.arc(center.x, center.y, radius * 1.18, 0, TAU);
  context.fill();

  const sphere = context.createRadialGradient(
    center.x - (radius * 0.34),
    center.y - (radius * 0.42),
    radius * 0.06,
    center.x,
    center.y,
    radius * 1.08,
  );
  sphere.addColorStop(0, 'rgba(255, 255, 255, 0.98)');
  sphere.addColorStop(0.34, 'rgba(248, 251, 255, 0.97)');
  sphere.addColorStop(0.72, 'rgba(235, 243, 253, 0.95)');
  sphere.addColorStop(1, 'rgba(213, 227, 246, 0.92)');
  context.fillStyle = sphere;
  context.beginPath();
  context.arc(center.x, center.y, radius, 0, TAU);
  context.fill();

  context.save();
  context.beginPath();
  context.arc(center.x, center.y, radius - 0.5, 0, TAU);
  context.clip();

  const gridLines: Point3D[][] = [];
  for (let latitude = -60; latitude <= 60; latitude += 30) {
    const points: Point3D[] = [];
    for (let longitude = -180; longitude <= 180; longitude += 4) {
      points.push(project(geoPoint(latitude, longitude, rotation, pitch), center, radius));
    }
    gridLines.push(points);
  }

  for (let longitude = -150; longitude <= 180; longitude += 30) {
    const points: Point3D[] = [];
    for (let latitude = -90; latitude <= 90; latitude += 3) {
      points.push(project(geoPoint(latitude, longitude, rotation, pitch), center, radius));
    }
    gridLines.push(points);
  }

  for (const points of gridLines) {
    strokeDepthCurve(
      context,
      points,
      (point) => point.z <= 0,
      'rgba(64, 93, 134, 0.055)',
      0.7,
    );
  }
  for (const points of gridLines) {
    strokeDepthCurve(
      context,
      points,
      (point) => point.z > 0,
      'rgba(18, 100, 255, 0.18)',
      0.85,
    );
  }

  for (let latitude = -75; latitude <= 75; latitude += 15) {
    for (let longitude = -180; longitude < 180; longitude += 15) {
      const point = project(geoPoint(latitude, longitude, rotation, pitch), center, radius);
      if (point.z <= 0.04) continue;
      const flicker = 0.76 + (Math.sin((time * 0.62) + latitude + longitude) * 0.24);
      const alpha = (0.08 + (point.z * 0.25)) * flicker;
      context.fillStyle = `rgba(112, 87, 201, ${alpha * 0.72})`;
      context.beginPath();
      context.arc(point.x, point.y, 0.4 + (point.z * 0.72), 0, TAU);
      context.fill();
    }
  }

  const sweep = context.createConicGradient((time * 0.24) % TAU, center.x, center.y);
  sweep.addColorStop(0, 'rgba(18, 100, 255, 0)');
  sweep.addColorStop(0.035, 'rgba(18, 100, 255, 0.08)');
  sweep.addColorStop(0.08, 'rgba(18, 100, 255, 0)');
  sweep.addColorStop(1, 'rgba(18, 100, 255, 0)');
  context.fillStyle = sweep;
  context.fillRect(center.x - radius, center.y - radius, radius * 2, radius * 2);

  const shade = context.createLinearGradient(
    center.x - radius,
    center.y,
    center.x + radius,
    center.y,
  );
  shade.addColorStop(0, 'rgba(42, 72, 111, 0.06)');
  shade.addColorStop(0.44, 'rgba(255, 255, 255, 0)');
  shade.addColorStop(1, 'rgba(18, 100, 255, 0.06)');
  context.fillStyle = shade;
  context.fillRect(center.x - radius, center.y - radius, radius * 2, radius * 2);
  context.restore();

  const rim = context.createLinearGradient(
    center.x - radius,
    center.y - radius,
    center.x + radius,
    center.y + radius,
  );
  rim.addColorStop(0, 'rgba(255, 255, 255, 0.9)');
  rim.addColorStop(0.5, 'rgba(111, 151, 211, 0.24)');
  rim.addColorStop(1, 'rgba(18, 100, 255, 0.45)');
  context.lineWidth = 1.5;
  context.strokeStyle = rim;
  context.beginPath();
  context.arc(center.x, center.y, radius, 0, TAU);
  context.stroke();

  context.lineWidth = 1;
  context.strokeStyle = 'rgba(255, 255, 255, 0.92)';
  context.beginPath();
  context.arc(center.x - 1, center.y - 1, radius - 1.5, Math.PI * 1.02, Math.PI * 1.58);
  context.stroke();
}

function drawCoreEnergy(
  context: CanvasRenderingContext2D,
  center: Point2D,
  radius: number,
  time: number,
) {
  context.save();
  context.translate(center.x, center.y);

  const coreGlow = context.createRadialGradient(0, 0, 0, 0, 0, radius * 0.46);
  coreGlow.addColorStop(0, 'rgba(18, 100, 255, 0.16)');
  coreGlow.addColorStop(0.48, 'rgba(18, 100, 255, 0.06)');
  coreGlow.addColorStop(1, 'rgba(18, 100, 255, 0)');
  context.fillStyle = coreGlow;
  context.beginPath();
  context.arc(0, 0, radius * 0.46, 0, TAU);
  context.fill();

  for (const offset of [0, 0.34, 0.68]) {
    const progress = fractional((time * 0.13) + offset);
    const waveRadius = radius * (0.28 + (progress * 0.34));
    context.lineWidth = 1.15;
    context.strokeStyle = `rgba(18, 100, 255, ${(1 - progress) * 0.16})`;
    context.beginPath();
    context.arc(0, 0, waveRadius, 0, TAU);
    context.stroke();
  }

  context.rotate((time * 0.18) - 0.22);
  context.lineWidth = 1;
  context.strokeStyle = 'rgba(18, 100, 255, 0.2)';
  context.beginPath();
  context.ellipse(0, 0, radius * 0.57, radius * 0.22, 0, 0, TAU);
  context.stroke();
  context.rotate(-(time * 0.31));
  context.setLineDash([3, 6]);
  context.strokeStyle = 'rgba(112, 87, 201, 0.17)';
  context.beginPath();
  context.ellipse(0, 0, radius * 0.48, radius * 0.3, 0, 0, TAU);
  context.stroke();
  context.setLineDash([]);
  context.restore();
}

function drawConnection(
  context: CanvasRenderingContext2D,
  start: Point2D,
  end: Point2D,
  bend: number,
  alpha: number,
  progress: number,
) {
  const deltaX = end.x - start.x;
  const deltaY = end.y - start.y;
  const distance = Math.max(Math.hypot(deltaX, deltaY), 1);
  const control = {
    x: ((start.x + end.x) * 0.5) - ((deltaY / distance) * bend),
    y: ((start.y + end.y) * 0.5) + ((deltaX / distance) * bend),
  };
  const gradient = context.createLinearGradient(start.x, start.y, end.x, end.y);
  gradient.addColorStop(0, `rgba(18, 100, 255, ${alpha * 0.16})`);
  gradient.addColorStop(1, `rgba(18, 100, 255, ${alpha * 0.58})`);

  context.save();
  context.shadowBlur = 7;
  context.shadowColor = `rgba(18, 100, 255, ${alpha * 0.2})`;
  context.lineWidth = 1.05;
  context.strokeStyle = gradient;
  context.beginPath();
  context.moveTo(start.x, start.y);
  context.quadraticCurveTo(control.x, control.y, end.x, end.y);
  context.stroke();
  context.restore();

  const particle = quadraticPoint(start, control, end, progress);
  context.fillStyle = `rgba(18, 100, 255, ${alpha * 0.14})`;
  context.beginPath();
  context.arc(particle.x, particle.y, 5.2, 0, TAU);
  context.fill();
  context.fillStyle = `rgba(18, 100, 255, ${alpha * 0.86})`;
  context.beginPath();
  context.arc(particle.x, particle.y, 2.25, 0, TAU);
  context.fill();
  context.lineWidth = 1;
  context.strokeStyle = `rgba(255, 255, 255, ${alpha * 0.94})`;
  context.stroke();
}

function drawPeerMesh(
  context: CanvasRenderingContext2D,
  nodes: ProjectedNode[],
  radius: number,
  time: number,
) {
  const visible = nodes.filter((node) => node.z > 0.04);
  const pairs: Array<[ProjectedNode, ProjectedNode]> = [];

  for (const [index, node] of visible.entries()) {
    const nearest = visible
      .slice(index + 1)
      .map((candidate) => ({
        candidate,
        distance: Math.hypot(candidate.x - node.x, candidate.y - node.y),
      }))
      .filter(({ distance }) => distance < radius * 1.18)
      .sort((left, right) => left.distance - right.distance)[0];
    if (nearest) pairs.push([node, nearest.candidate]);
  }

  for (const [index, [start, end]] of pairs.entries()) {
    const depth = Math.min(start.z, end.z);
    drawConnection(
      context,
      start,
      end,
      radius * (index % 2 === 0 ? 0.045 : -0.045),
      clamp(depth * 0.42, 0.12, 0.34),
      fractional((time * 0.11) + (index * 0.23)),
    );
  }
}

function drawHumanGlyph(
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  scale: number,
  alpha: number,
) {
  context.fillStyle = `rgba(42, 65, 97, ${alpha})`;
  context.beginPath();
  context.arc(x, y - (3.2 * scale), 2.1 * scale, 0, TAU);
  context.fill();
  context.beginPath();
  context.arc(x, y + (3.1 * scale), 4.1 * scale, Math.PI, TAU);
  context.fill();
}

function drawAgentGlyph(
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  scale: number,
  alpha: number,
) {
  context.save();
  context.translate(x, y);
  context.rotate(Math.PI / 4);
  context.fillStyle = `rgba(112, 87, 201, ${alpha})`;
  context.fillRect(-3.7 * scale, -3.7 * scale, 7.4 * scale, 7.4 * scale);
  context.fillStyle = `rgba(255, 255, 255, ${alpha})`;
  context.beginPath();
  context.arc(0, 0, 1.25 * scale, 0, TAU);
  context.fill();
  context.restore();
}

function drawNodePanel(
  context: CanvasRenderingContext2D,
  node: ProjectedNode,
  center: Point2D,
  radius: number,
  compact: boolean,
  alpha: number,
) {
  const directionX = (node.x - center.x) / radius;
  const directionY = (node.y - center.y) / radius;
  const panelWidth = compact ? 47 : 58;
  const panelHeight = compact ? 22 : 26;
  const x = node.x + (directionX * radius * 0.085);
  const y = node.y + (directionY * radius * 0.085);
  const left = x - (panelWidth * 0.5);
  const top = y - (panelHeight * 0.5);

  context.save();
  context.shadowBlur = 11;
  context.shadowColor = `rgba(36, 76, 137, ${alpha * 0.14})`;
  context.lineWidth = 0.9;
  context.fillStyle = `rgba(255, 255, 255, ${0.7 + (alpha * 0.28)})`;
  context.strokeStyle = `rgba(18, 100, 255, ${alpha * 0.48})`;
  context.beginPath();
  context.roundRect(left, top, panelWidth, panelHeight, compact ? 6 : 7);
  context.fill();
  context.stroke();
  context.restore();

  const panelLight = context.createLinearGradient(left, top, left, top + panelHeight);
  panelLight.addColorStop(0, `rgba(255, 255, 255, ${alpha * 0.44})`);
  panelLight.addColorStop(1, 'rgba(18, 100, 255, 0.025)');
  context.fillStyle = panelLight;
  context.beginPath();
  context.roundRect(left + 1, top + 1, panelWidth - 2, panelHeight - 2, compact ? 5 : 6);
  context.fill();

  context.lineWidth = 0.8;
  context.strokeStyle = `rgba(18, 100, 255, ${alpha * 0.46})`;
  context.beginPath();
  context.moveTo(node.x, node.y);
  context.lineTo(
    x - (directionX * panelWidth * 0.34),
    y - (directionY * panelHeight * 0.34),
  );
  context.stroke();

  const glyphScale = compact ? 0.72 : 0.86;
  drawHumanGlyph(context, left + (compact ? 10 : 12), y, glyphScale, alpha);
  drawAgentGlyph(context, left + (compact ? 21 : 25), y, glyphScale, alpha);

  context.fillStyle = `rgba(55, 77, 110, ${alpha})`;
  context.font = `${compact ? 7 : 8}px "Geist Mono Variable", monospace`;
  context.textAlign = 'left';
  context.textBaseline = 'middle';
  context.fillText(node.code, left + (compact ? 29 : 35), y + 0.5);
}

function drawLayerNode(
  context: CanvasRenderingContext2D,
  point: Point2D,
  label: string,
  kind: 'cloud' | 'edge',
  compact: boolean,
) {
  const width = compact ? 58 : 70;
  const height = compact ? 24 : 28;
  context.save();
  context.shadowBlur = 14;
  context.shadowColor = 'rgba(36, 76, 137, 0.16)';
  context.fillStyle = 'rgba(255, 255, 255, 0.95)';
  context.strokeStyle = 'rgba(18, 100, 255, 0.52)';
  context.lineWidth = 0.9;
  context.beginPath();
  context.roundRect(point.x - (width * 0.5), point.y - (height * 0.5), width, height, compact ? 7 : 8);
  context.fill();
  context.stroke();
  context.restore();

  context.fillStyle = kind === 'cloud' ? 'rgba(18, 100, 255, 0.12)' : 'rgba(12, 155, 112, 0.12)';
  context.beginPath();
  if (kind === 'cloud') context.arc(point.x - (width * 0.31), point.y, compact ? 5 : 6, 0, TAU);
  else context.roundRect(point.x - (width * 0.39), point.y - (compact ? 5 : 6), compact ? 10 : 12, compact ? 10 : 12, 2);
  context.fill();

  context.fillStyle = '#3d567a';
  context.font = `650 ${compact ? 7 : 8}px "Geist Mono Variable", monospace`;
  context.textAlign = 'center';
  context.textBaseline = 'middle';
  context.fillText(label.toUpperCase(), point.x + (compact ? 7 : 8), point.y + 0.5);
}

function drawLayerConnection(
  context: CanvasRenderingContext2D,
  start: Point2D,
  end: Point2D,
  time: number,
  offset: number,
) {
  const control = {
    x: ((start.x + end.x) * 0.5) + ((end.y - start.y) * 0.12),
    y: ((start.y + end.y) * 0.5) - ((end.x - start.x) * 0.12),
  };
  context.setLineDash([3, 5]);
  context.lineWidth = 1;
  context.strokeStyle = 'rgba(18, 100, 255, 0.34)';
  context.beginPath();
  context.moveTo(start.x, start.y);
  context.quadraticCurveTo(control.x, control.y, end.x, end.y);
  context.stroke();
  context.setLineDash([]);

  for (const direction of [0, 0.52]) {
    const rawProgress = (time * 0.12 + offset + direction) % 1;
    const progress = direction === 0 ? rawProgress : 1 - rawProgress;
    const particle = quadraticPoint(start, control, end, progress);
    context.save();
    context.shadowBlur = 10;
    context.shadowColor = 'rgba(18, 100, 255, 0.28)';
    context.fillStyle = direction === 0 ? '#1264ff' : '#0c9b70';
    context.beginPath();
    context.arc(particle.x, particle.y, 2.2, 0, TAU);
    context.fill();
    context.restore();
  }
}

export function drawGlobalWorkspaceScene(
  context: CanvasRenderingContext2D,
  frame: WorkspaceSceneFrame,
) {
  const time = frame.reducedMotion ? STATIC_TIME : frame.time;
  const compact = frame.width < 520;
  const center = {
    x: frame.width * 0.5,
    y: frame.height * 0.47,
  };
  const availableHeight = Math.max(frame.height - (compact ? 100 : 116), 180);
  const radius = Math.min(frame.width * (compact ? 0.315 : 0.31), availableHeight * 0.45);
  const rotation = (time * 0.18) + (frame.pointerX * 0.2);
  const pitch = -0.15 + (frame.pointerY * 0.1);
  const projectedNodes: ProjectedNode[] = organizationNodes.map((node) => ({
    ...project(geoPoint(node.latitude, node.longitude, rotation, pitch), center, radius),
    code: node.code,
    phase: node.phase,
  }));

  drawBackdrop(context, frame.width, frame.height, center, radius);
  drawStateField(context, frame.width, frame.height, time);
  drawNeuralLattice(
    context,
    frame.width,
    frame.height,
    time,
    frame.pointerX,
    frame.pointerY,
  );
  drawOrbitSystem(context, center, radius, time);
  drawGlobe(context, center, radius, rotation, pitch, time);
  drawCoreEnergy(context, center, radius, time);

  const cloud = {
    x: center.x + (radius * 0.88),
    y: center.y - (radius * 0.78),
  };
  const edge = {
    x: center.x - (radius * 0.92),
    y: center.y + (radius * 0.78),
  };

  drawLayerConnection(context, center, cloud, time, 0.13);
  drawLayerConnection(context, center, edge, time, 0.61);
  drawPeerMesh(context, projectedNodes, radius, time);

  for (const [index, node] of projectedNodes.entries()) {
    if (node.z < -0.16) continue;
    const alpha = clamp((node.z + 0.22) / 1.22, 0.16, 0.88);
    drawConnection(
      context,
      node,
      center,
      radius * (index % 2 === 0 ? 0.11 : -0.11),
      alpha,
      (time * 0.16 + node.phase) % 1,
    );
  }

  for (const node of projectedNodes.sort((left, right) => left.z - right.z)) {
    if (node.z < -0.05) continue;
    const alpha = clamp((node.z + 0.15) / 1.15, 0.2, 1);
    drawNodePanel(context, node, center, radius, compact, alpha);
  }

  drawLayerNode(context, cloud, frame.labels.cloud, 'cloud', compact);
  drawLayerNode(context, edge, frame.labels.edge, 'edge', compact);

  const syncX = compact ? center.x : center.x + (radius * 0.52);
  const syncY = compact ? center.y + (radius * 1.17) : center.y + (radius * 1.12);
  context.fillStyle = 'rgba(83, 101, 126, 0.72)';
  context.font = `600 ${compact ? 7 : 8}px "Geist Mono Variable", monospace`;
  context.textAlign = 'center';
  context.textBaseline = 'middle';
  context.fillText(frame.labels.sync.toUpperCase(), syncX, syncY);
}
