import type * as THREE from 'three';
import {
  createProceduralFighter,
  type ProceduralFighterOptions,
  type ProceduralFighterVariant,
} from './procedural-fighter';

export type CandidateFamily = 'a3s-code' | 'codex' | 'claude-code' | 'unknown';
export type AircraftId = ProceduralFighterVariant;

export interface AircraftCreateOptions extends ProceduralFighterOptions {
  airframeId?: AircraftId;
  /** Visual identity is owned by the selected pilot, not by the executable Adapter reference. */
  candidateFamily?: CandidateFamily;
}

export interface AircraftAssetSource {
  kind: 'procedural';
  attribution: string;
  bundled: true;
}

export interface ExternalAircraftReference {
  author: string;
  license: 'CC-BY-4.0';
  status: 'not-bundled';
  url: string;
}

export interface AircraftProfile {
  id: AircraftId;
  candidateFamily: CandidateFamily;
  agentLabel: string;
  displayName: string;
  manufacturer: string;
  aliases: readonly string[];
  accentColor: THREE.ColorRepresentation;
  source: AircraftAssetSource;
  externalReference?: ExternalAircraftReference;
  create: (options?: ProceduralFighterOptions) => THREE.Group;
}

const PROCEDURAL_SOURCE = Object.freeze({
  kind: 'procedural' as const,
  attribution: 'A3S Agent Evaluation procedural aircraft generator',
  bundled: true as const,
});

export const AIRCRAFT_PROFILES = Object.freeze({
  a3sCode: Object.freeze({
    id: 'j-35',
    candidateFamily: 'a3s-code',
    agentLabel: 'A3S Code',
    displayName: 'J-35',
    manufacturer: 'Shenyang Aircraft Corporation',
    aliases: Object.freeze(['a3s-code', 'a3s code', 'a3scode']),
    accentColor: 0x6ca3ff,
    source: PROCEDURAL_SOURCE,
    externalReference: Object.freeze({
      author: 'SB-129',
      license: 'CC-BY-4.0',
      status: 'not-bundled',
      url: 'https://sketchfab.com/3d-models/shenyang-j-35-37085f458d7943cd8314372af76e0f67',
    }),
    create: (options: ProceduralFighterOptions = {}) =>
      createProceduralFighter('j-35', {
        accentColor: 0x6ca3ff,
        baseColor: 0x667482,
        livery: 'a3s',
        secondaryColor: 0x746cf0,
        ...options,
      }),
  } satisfies AircraftProfile),
  codex: Object.freeze({
    id: 'f-35',
    candidateFamily: 'codex',
    agentLabel: 'Codex',
    displayName: 'F-35 Lightning II',
    manufacturer: 'Lockheed Martin',
    aliases: Object.freeze(['codex', 'openai']),
    accentColor: 0x10a37f,
    source: PROCEDURAL_SOURCE,
    create: (options: ProceduralFighterOptions = {}) =>
      createProceduralFighter('f-35', {
        accentColor: 0x10a37f,
        baseColor: 0x4c5658,
        livery: 'codex',
        secondaryColor: 0xdcebed,
        ...options,
      }),
  } satisfies AircraftProfile),
  claude: Object.freeze({
    id: 'f-22',
    candidateFamily: 'claude-code',
    agentLabel: 'Claude Code',
    displayName: 'F-22 Raptor',
    manufacturer: 'Lockheed Martin / Boeing',
    aliases: Object.freeze(['claude-code', 'claude code', 'claude', 'anthropic']),
    accentColor: 0xd97757,
    source: PROCEDURAL_SOURCE,
    create: (options: ProceduralFighterOptions = {}) =>
      createProceduralFighter('f-22', {
        accentColor: 0xd97757,
        baseColor: 0x716d69,
        livery: 'claude',
        secondaryColor: 0xf0c69d,
        ...options,
      }),
  } satisfies AircraftProfile),
  prototype: Object.freeze({
    id: 'prototype',
    candidateFamily: 'unknown',
    agentLabel: 'Unknown Candidate',
    displayName: 'Generic Test Prototype',
    manufacturer: 'A3S Agent Evaluation',
    aliases: Object.freeze(['unknown']),
    accentColor: 0xa88cf2,
    source: PROCEDURAL_SOURCE,
    create: (options: ProceduralFighterOptions = {}) =>
      createProceduralFighter('prototype', {
        accentColor: 0xa88cf2,
        livery: 'generic',
        secondaryColor: 0xd8c9ff,
        ...options,
      }),
  } satisfies AircraftProfile),
});

const IDENTIFIED_PROFILES: readonly AircraftProfile[] = [
  AIRCRAFT_PROFILES.a3sCode,
  AIRCRAFT_PROFILES.codex,
  AIRCRAFT_PROFILES.claude,
];

export function resolveAircraft(candidate: string): AircraftProfile {
  const normalizedCandidate = normalizeIdentity(candidate);
  if (!normalizedCandidate) return AIRCRAFT_PROFILES.prototype;

  return (
    IDENTIFIED_PROFILES.find((profile) =>
      profile.aliases.some((alias) => containsIdentity(normalizedCandidate, normalizeIdentity(alias)))
    ) ?? AIRCRAFT_PROFILES.prototype
  );
}

export function aircraftProfileForFamily(candidateFamily: CandidateFamily): AircraftProfile {
  return (
    IDENTIFIED_PROFILES.find((profile) => profile.candidateFamily === candidateFamily) ?? AIRCRAFT_PROFILES.prototype
  );
}

export function createAircraft(candidate: string, options: AircraftCreateOptions = {}): THREE.Group {
  const { candidateFamily, ...themedOptions } = options;
  const profile = candidateFamily ? aircraftProfileForFamily(candidateFamily) : resolveAircraft(candidate);
  const { airframeId = profile.id, ...fighterOptions } = themedOptions;
  const aircraft = createProceduralFighter(airframeId, {
    ...agentTheme(profile.candidateFamily),
    ...fighterOptions,
  });
  aircraft.userData.aircraft = {
    candidateFamily: profile.candidateFamily,
    agentLabel: profile.agentLabel,
    modelId: airframeId,
    displayName: aircraft.name.replace('aircraft:', '').toUpperCase(),
    accentColor: profile.accentColor,
    forwardAxis: '-x',
    upAxis: '+y',
    spanAxis: '+z',
  };
  return aircraft;
}

function agentTheme(candidateFamily: CandidateFamily): ProceduralFighterOptions {
  if (candidateFamily === 'a3s-code') {
    return { accentColor: 0x6ca3ff, baseColor: 0x667482, livery: 'a3s', secondaryColor: 0x746cf0 };
  }
  if (candidateFamily === 'codex') {
    return { accentColor: 0x10a37f, baseColor: 0x4c5658, livery: 'codex', secondaryColor: 0xdcebed };
  }
  if (candidateFamily === 'claude-code') {
    return { accentColor: 0xd97757, baseColor: 0x716d69, livery: 'claude', secondaryColor: 0xf0c69d };
  }
  return { accentColor: 0xa88cf2, livery: 'generic', secondaryColor: 0xd8c9ff };
}

function normalizeIdentity(value: string): string {
  return value
    .normalize('NFKC')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

function containsIdentity(candidate: string, alias: string): boolean {
  return candidate === alias || `-${candidate}-`.includes(`-${alias}-`);
}
