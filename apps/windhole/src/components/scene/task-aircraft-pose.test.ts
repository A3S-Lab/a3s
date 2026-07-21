import { describe, expect, it } from 'vitest';
import { taskAircraftPose } from './task-aircraft-pose';

describe('task aircraft pose', () => {
  it('returns a neutral pose when no task is selected', () => {
    expect(taskAircraftPose('', 0)).toEqual({ rollX: 0, yawY: 0, pitchZ: 0 });
  });

  it('gives different tasks deterministic formation attitudes', () => {
    const first = [0, 1, 2].map((index) => taskAircraftPose('quick_file_edit', index));
    const second = [0, 1, 2].map((index) => taskAircraftPose('rust_multicrate_reconstruction', index));

    expect(first).not.toEqual(second);
    expect([0, 1, 2].map((index) => taskAircraftPose('quick_file_edit', index))).toEqual(first);
  });

  it('keeps automatic attitudes within restrained inspection limits', () => {
    for (let index = 0; index < 4; index += 1) {
      const pose = taskAircraftPose('warehouse_forklift_routing', index);
      expect(Math.abs(pose.rollX)).toBeLessThanOrEqual((11 * Math.PI) / 180);
      expect(Math.abs(pose.yawY)).toBeLessThanOrEqual((8 * Math.PI) / 180);
      expect(Math.abs(pose.pitchZ)).toBeLessThanOrEqual((2.8 * Math.PI) / 180);
    }
  });
});
