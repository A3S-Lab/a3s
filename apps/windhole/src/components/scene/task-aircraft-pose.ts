export interface AircraftTaskPose {
  rollX: number;
  yawY: number;
  pitchZ: number;
}

export function taskAircraftPose(taskId: string, aircraftIndex: number): AircraftTaskPose {
  if (!taskId) return { rollX: 0, yawY: 0, pitchZ: 0 };

  const phase = (stableHash(taskId) / 0xffffffff) * Math.PI * 2;
  const lanePhase = aircraftIndex * 1.83;
  return {
    rollX: degrees(Math.sin(phase + lanePhase) * 11),
    yawY: degrees(Math.cos(phase * 0.79 + lanePhase * 1.17) * 8),
    pitchZ: degrees(Math.sin(phase * 1.23 - lanePhase * 0.61) * 2.8),
  };
}

function stableHash(value: string): number {
  let hash = 2166136261;
  for (const character of value) {
    hash ^= character.charCodeAt(0);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function degrees(value: number): number {
  return (value * Math.PI) / 180;
}
