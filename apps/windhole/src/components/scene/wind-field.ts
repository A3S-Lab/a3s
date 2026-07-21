import * as THREE from 'three';
import type { WindTunnelParameters } from '../../types/bench';

interface Streamline {
  baseY: number;
  baseZ: number;
  phase: number;
  line: THREE.Line<THREE.BufferGeometry, THREE.LineBasicMaterial>;
  positions: Float32Array;
  attribute: THREE.BufferAttribute;
}

interface ParticleField {
  points: THREE.Points<THREE.BufferGeometry, THREE.PointsMaterial>;
  positions: Float32Array;
  baseY: Float32Array;
  baseZ: Float32Array;
  phase: Float32Array;
  attribute: THREE.BufferAttribute;
}

export interface WindField {
  streamlines: Streamline[];
  particles: ParticleField;
}

export function createWindField(scene: THREE.Scene): WindField {
  const lineMaterial = new THREE.LineBasicMaterial({
    color: 0x62dce5,
    transparent: true,
    opacity: 0.26,
    depthWrite: false,
  });
  const streamlines: Streamline[] = [];
  const rows = [-2.35, -1.55, -0.75, 0.05, 0.85, 1.65, 2.45];
  const depths = [-3, -1.5, 0, 1.5, 3];
  let index = 0;
  for (const baseZ of depths) {
    for (const baseY of rows) {
      if ((index + Math.round(baseY * 10)) % 2 !== 0) {
        index += 1;
        continue;
      }
      const positions = new Float32Array(88 * 3);
      const attribute = new THREE.BufferAttribute(positions, 3);
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute('position', attribute);
      const line = new THREE.Line(geometry, lineMaterial);
      line.frustumCulled = false;
      scene.add(line);
      streamlines.push({ baseY, baseZ, phase: index * 0.57, line, positions, attribute });
      index += 1;
    }
  }

  const count = 1_050;
  const positions = new Float32Array(count * 3);
  const baseY = new Float32Array(count);
  const baseZ = new Float32Array(count);
  const phase = new Float32Array(count);
  for (let particle = 0; particle < count; particle += 1) {
    const offset = particle * 3;
    positions[offset] = -9.5 + Math.random() * 19;
    baseY[particle] = -2.75 + Math.random() * 5.6;
    baseZ[particle] = -3.6 + Math.random() * 7.2;
    positions[offset + 1] = baseY[particle];
    positions[offset + 2] = baseZ[particle];
    phase[particle] = Math.random() * Math.PI * 2;
  }
  const attribute = new THREE.BufferAttribute(positions, 3);
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', attribute);
  const material = new THREE.PointsMaterial({
    color: 0x8eeaf0,
    size: 0.038,
    transparent: true,
    opacity: 0.58,
    depthWrite: false,
  });
  const points = new THREE.Points(geometry, material);
  scene.add(points);

  return { streamlines, particles: { points, positions, baseY, baseZ, phase, attribute } };
}

export function updateWindField(
  field: WindField,
  parameters: WindTunnelParameters,
  aircraftCenters: readonly THREE.Vector3[],
  elapsed: number,
  delta: number
): void {
  const speed = 1.25 + parameters.mach * 3.65;
  const particles = field.particles;
  for (let index = 0; index < particles.baseY.length; index += 1) {
    const offset = index * 3;
    let x = particles.positions[offset] + delta * speed;
    if (x > 9.55) x = -9.55;
    particles.positions[offset] = x;
    particles.positions[offset + 1] = airflowY(
      x,
      particles.baseY[index],
      particles.baseZ[index],
      parameters,
      aircraftCenters,
      elapsed,
      particles.phase[index]
    );
    particles.positions[offset + 2] =
      particles.baseZ[index] +
      parameters.turbulence * 0.045 * Math.cos(x * 1.7 + elapsed * 3.4 + particles.phase[index]);
  }
  particles.attribute.needsUpdate = true;

  for (const streamline of field.streamlines) {
    for (let point = 0; point < 88; point += 1) {
      const x = -9.6 + point * (19.2 / 87);
      const offset = point * 3;
      streamline.positions[offset] = x;
      streamline.positions[offset + 1] = airflowY(
        x,
        streamline.baseY,
        streamline.baseZ,
        parameters,
        aircraftCenters,
        elapsed,
        streamline.phase
      );
      streamline.positions[offset + 2] =
        streamline.baseZ + parameters.turbulence * 0.025 * Math.cos(x * 1.45 + elapsed * 2.8 + streamline.phase);
    }
    streamline.attribute.needsUpdate = true;
    streamline.line.visible = parameters.smokeVisible;
  }
  particles.points.visible = parameters.smokeVisible;
}

function airflowY(
  x: number,
  baseY: number,
  baseZ: number,
  parameters: WindTunnelParameters,
  aircraftCenters: readonly THREE.Vector3[],
  elapsed: number,
  phase: number
): number {
  const angle = THREE.MathUtils.degToRad(parameters.angleOfAttack);
  let displacement = 0;
  let wake = 0;
  let localInfluence = 0;

  for (const center of aircraftCenters) {
    const relativeX = x - center.x;
    const relativeY = baseY - center.y;
    const relativeZ = baseZ - center.z;
    const depthFalloff = Math.exp(-(relativeZ * relativeZ) / 1.3);
    const bodyFalloff = Math.exp(-((relativeX / 2.25) ** 2)) * Math.exp(-Math.abs(relativeY) * 1.4) * depthFalloff;
    displacement += Math.sign(relativeY || 1) * bodyFalloff * 0.31;
    localInfluence = Math.max(localInfluence, bodyFalloff);
    if (relativeX > 1.7) {
      wake += -Math.sin(angle) * Math.exp(-(relativeX - 1.7) / 3.7) * depthFalloff * 0.74;
    }
  }

  const noise =
    parameters.turbulence *
    0.105 *
    Math.sin(x * 2.05 + elapsed * 4.2 + phase) *
    (0.22 + Math.max(localInfluence, x > 2 ? 0.55 : 0));
  return baseY + displacement + wake + noise;
}
