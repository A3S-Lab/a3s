import type { AerodynamicTelemetry, WindTunnelParameters } from '../types/bench';

const speedOfSound = 340.3;
const referenceArea = 27.9;
const referenceLength = 15.1;
const dynamicViscosity = 1.81e-5;

export function calculateTelemetry(parameters: WindTunnelParameters): AerodynamicTelemetry {
  const velocity = parameters.mach * speedOfSound;
  const dynamicPressure = 0.5 * parameters.airDensity * velocity ** 2;
  const angleRadians = (parameters.angleOfAttack * Math.PI) / 180;
  const linearLift = 0.18 + 5.35 * angleRadians;
  const stallOnset = Math.max(0, Math.abs(parameters.angleOfAttack) - 13) / 8;
  const stallFactor = 1 - Math.min(0.58, stallOnset * 0.58);
  const liftCoefficient = clamp(linearLift * stallFactor, -1.25, 1.58);
  const dragCoefficient = 0.024 + 0.052 * liftCoefficient ** 2 + parameters.turbulence * 0.038;
  const lift = dynamicPressure * referenceArea * liftCoefficient;
  const drag = dynamicPressure * referenceArea * dragCoefficient;
  const reynolds = (parameters.airDensity * velocity * referenceLength) / dynamicViscosity;
  const flowState =
    parameters.turbulence < 0.22 ? 'laminar' : parameters.turbulence < 0.55 ? 'transitional' : 'turbulent';

  return {
    velocity,
    dynamicPressure,
    liftCoefficient,
    dragCoefficient,
    lift,
    drag,
    reynolds,
    flowState,
  };
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}
