import { type AirframeId, type AirframeResolution, resolveAirframe } from './airframe-selection';
import { resolveWeaponLoadout, type WeaponLoadout } from './weapon-loadout';

export interface AircraftConfigurationInput {
  candidate?: string | null;
  airframeId?: AirframeId | null;
  model?: string | null;
  /** Visual metaphor only. This value is never forwarded to A3S Bench. */
  effort?: string | null;
}

export interface AircraftConfiguration {
  airframe: AirframeResolution;
  loadout: WeaponLoadout;
}

export function resolveAircraftConfiguration(input: AircraftConfigurationInput): AircraftConfiguration {
  return {
    airframe: resolveAirframe(input.model, input.candidate, input.airframeId),
    loadout: resolveWeaponLoadout(input.effort),
  };
}
