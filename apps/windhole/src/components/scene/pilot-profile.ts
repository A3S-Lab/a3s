import type { CandidateFamily } from './aircraft-registry';
import { resolveAircraft } from './aircraft-registry';

export type PilotIdentity = 'a3s' | 'codex' | 'claude' | 'generic';
export type PilotMarkingPattern = 'spectrum-bars' | 'orbit-ring' | 'triple-ray' | 'test-chevron';

export interface PilotAttirePalette {
  readonly flightSuit: number;
  readonly gloves: number;
  readonly harness: number;
  readonly helmet: number;
  readonly visor: number;
}

export interface PilotMarking {
  readonly helmetCode: string;
  readonly label: string;
  readonly pattern: PilotMarkingPattern;
  readonly primaryColor: number;
  readonly secondaryColor: number;
}

export interface PilotProfile {
  readonly id: PilotIdentity;
  readonly candidateFamily: CandidateFamily;
  readonly displayName: string;
  readonly cockpitGlowColor: number;
  readonly attire: PilotAttirePalette;
  readonly marking: PilotMarking;
}

export const PILOT_PROFILES: Readonly<Record<CandidateFamily, PilotProfile>> = Object.freeze({
  'a3s-code': definePilotProfile({
    id: 'a3s',
    candidateFamily: 'a3s-code',
    displayName: 'A3S Flight Lead',
    cockpitGlowColor: 0x6ca3ff,
    attire: {
      flightSuit: 0x3e5f8f,
      gloves: 0x8fb7ff,
      harness: 0x746cf0,
      helmet: 0xdeeaff,
      visor: 0x112a38,
    },
    marking: {
      helmetCode: 'A3S',
      label: 'Blue-violet spectrum bars',
      pattern: 'spectrum-bars',
      primaryColor: 0x6ca3ff,
      secondaryColor: 0x746cf0,
    },
  }),
  codex: definePilotProfile({
    id: 'codex',
    candidateFamily: 'codex',
    displayName: 'Codex Systems Pilot',
    cockpitGlowColor: 0x10a37f,
    attire: {
      flightSuit: 0x1f6d5b,
      gloves: 0x9cd9c8,
      harness: 0x10a37f,
      helmet: 0xe4f0ec,
      visor: 0x0b2523,
    },
    marking: {
      helmetCode: 'CX',
      label: 'Emerald orbit ring',
      pattern: 'orbit-ring',
      primaryColor: 0x10a37f,
      secondaryColor: 0xdcebed,
    },
  }),
  'claude-code': definePilotProfile({
    id: 'claude',
    candidateFamily: 'claude-code',
    displayName: 'Claude Test Pilot',
    cockpitGlowColor: 0xd97757,
    attire: {
      flightSuit: 0x8b4c36,
      gloves: 0xe9a784,
      harness: 0xd97757,
      helmet: 0xf1d5ba,
      visor: 0x2d1c18,
    },
    marking: {
      helmetCode: 'CC',
      label: 'Terracotta triple ray',
      pattern: 'triple-ray',
      primaryColor: 0xd97757,
      secondaryColor: 0xf0c69d,
    },
  }),
  unknown: definePilotProfile({
    id: 'generic',
    candidateFamily: 'unknown',
    displayName: 'Unassigned Test Pilot',
    cockpitGlowColor: 0xa88cf2,
    attire: {
      flightSuit: 0x5c6678,
      gloves: 0xbac4d7,
      harness: 0xa88cf2,
      helmet: 0xd8dde7,
      visor: 0x202635,
    },
    marking: {
      helmetCode: 'TEST',
      label: 'Violet test chevron',
      pattern: 'test-chevron',
      primaryColor: 0xa88cf2,
      secondaryColor: 0xd8c9ff,
    },
  }),
});

export function resolvePilotProfile(candidate: string): PilotProfile {
  return pilotProfileForFamily(resolveAircraft(candidate).candidateFamily);
}

export function pilotProfileForFamily(candidateFamily: CandidateFamily): PilotProfile {
  return PILOT_PROFILES[candidateFamily];
}

export function pilotProfileForId(pilotId: PilotIdentity): PilotProfile {
  const profile = Object.values(PILOT_PROFILES).find((candidate) => candidate.id === pilotId);
  if (!profile) throw new Error(`Unknown pilot profile: ${pilotId}`);
  return profile;
}

function definePilotProfile(profile: PilotProfile): PilotProfile {
  return Object.freeze({
    ...profile,
    attire: Object.freeze({ ...profile.attire }),
    marking: Object.freeze({ ...profile.marking }),
  });
}
