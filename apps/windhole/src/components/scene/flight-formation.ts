import { type AircraftConfiguration, resolveAircraftConfiguration } from './aircraft-configuration';
import {
  aircraftProfileForFamily,
  AIRCRAFT_PROFILES,
  type AircraftProfile,
  resolveAircraft,
} from './aircraft-registry';
import type { AirframeId } from './airframe-selection';
import { type PilotIdentity, type PilotProfile, pilotProfileForId, resolvePilotProfile } from './pilot-profile';
import type { ReasoningEffort } from './weapon-loadout';

export interface FormationAircraft {
  airframeId?: AirframeId;
  instanceId: string;
  candidate: string;
  candidateLabel: string;
  configuration: AircraftConfiguration;
  effort: ReasoningEffort;
  model: string;
  pilot: PilotProfile;
  pilotId?: PilotIdentity;
  profile: AircraftProfile;
  position: readonly [x: number, y: number, z: number];
  scale: number;
  phase: number;
}

export interface FormationRosterEntry {
  id: string;
  airframeId: AirframeId;
  pilotId: PilotIdentity;
  candidate: string;
  callsign: string;
  model: string;
  effort: ReasoningEffort;
}

type FormationSeed = Omit<FormationAircraft, 'configuration' | 'pilot' | 'profile'>;

const FORMATION_SLOTS = [
  { position: [-3.1, 1.45, -4.2], scale: 0.66, phase: 0.2 },
  { position: [0, -1.45, -1.35], scale: 0.6, phase: 1.8 },
  { position: [3.1, 1.35, 1.5], scale: 0.51, phase: 3.4 },
  { position: [-1.35, -1.5, 4.35], scale: 0.44, phase: 4.8 },
  { position: [3.35, -1.55, 4.75], scale: 0.43, phase: 5.7 },
] as const;

/** Builds the live scene exclusively from the serializable hangar roster. */
export function buildRosterFormation(roster: readonly FormationRosterEntry[]): FormationAircraft[] {
  return roster.slice(0, FORMATION_SLOTS.length).map((entry, index) =>
    createFormationAircraft({
      instanceId: entry.id,
      candidate: entry.candidate,
      candidateLabel: entry.callsign,
      effort: entry.effort,
      model: entry.model,
      airframeId: entry.airframeId,
      pilotId: entry.pilotId,
      ...FORMATION_SLOTS[index],
    })
  );
}

export function selectedFormationId(
  formation: readonly FormationAircraft[],
  candidate: string,
  preferredEntryId?: string
): string {
  if (preferredEntryId && formation.some((entry) => entry.instanceId === preferredEntryId)) return preferredEntryId;
  const normalizedCandidate = candidate.trim();
  const profile = resolveAircraft(normalizedCandidate);
  const matched = formation.find((entry) =>
    profile === AIRCRAFT_PROFILES.prototype
      ? Boolean(normalizedCandidate) && entry.candidate === normalizedCandidate
      : entry.profile.candidateFamily === profile.candidateFamily
  );
  return matched?.instanceId ?? formation[0]?.instanceId ?? '';
}

function createFormationAircraft(seed: FormationSeed): FormationAircraft {
  const pilot = seed.pilotId ? pilotProfileForId(seed.pilotId) : resolvePilotProfile(seed.candidate);
  return {
    ...seed,
    configuration: resolveAircraftConfiguration({
      airframeId: seed.airframeId,
      candidate: seed.candidate,
      model: seed.model,
      effort: seed.effort,
    }),
    pilot,
    profile: seed.pilotId ? aircraftProfileForFamily(pilot.candidateFamily) : resolveAircraft(seed.candidate),
  };
}
