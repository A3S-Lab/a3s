import { describe, expect, it } from 'vitest';
import { calculateTelemetry } from './aerodynamics';

describe('calculateTelemetry', () => {
  it('derives stable positive forces for a nominal test point', () => {
    const result = calculateTelemetry({
      mach: 0.82,
      angleOfAttack: 4,
      airDensity: 1.225,
      turbulence: 0.12,
      smokeVisible: true,
      paused: false,
    });

    expect(result.velocity).toBeCloseTo(279.046, 2);
    expect(result.dynamicPressure).toBeGreaterThan(40_000);
    expect(result.lift).toBeGreaterThan(result.drag);
    expect(result.flowState).toBe('laminar');
  });

  it('reduces lift after the configured stall onset', () => {
    const common = {
      mach: 0.6,
      airDensity: 1.225,
      turbulence: 0.2,
      smokeVisible: true,
      paused: false,
    };
    const preStall = calculateTelemetry({ ...common, angleOfAttack: 13 });
    const postStall = calculateTelemetry({ ...common, angleOfAttack: 21 });

    expect(postStall.liftCoefficient).toBeLessThan(preStall.liftCoefficient);
  });
});
