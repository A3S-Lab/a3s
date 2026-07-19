import { AIRCRAFT_PROFILES, type AircraftId, resolveAircraft } from './aircraft-registry';

export type AirframeId = AircraftId | 'j-50';
export type AirframeResolutionStrategy = 'manual' | 'model-exact' | 'model-family' | 'candidate-fallback' | 'default';

export interface AirframeDefinition {
  id: AirframeId;
  displayName: string;
  manufacturer: string;
}

export interface ModelAirframeRule {
  id: 'glm' | 'gpt' | 'claude';
  airframeId: AirframeId;
  exactModels: readonly string[];
  familyPrefixes: readonly string[];
}

export interface AirframeResolution {
  airframe: AirframeDefinition;
  strategy: AirframeResolutionStrategy;
  ruleId?: ModelAirframeRule['id'];
}

export const AIRFRAMES = Object.freeze({
  'j-50': Object.freeze({
    id: 'j-50',
    displayName: 'J-50',
    manufacturer: 'Shenyang Aircraft Corporation',
  } satisfies AirframeDefinition),
  'j-35': Object.freeze({
    id: 'j-35',
    displayName: 'J-35',
    manufacturer: 'Shenyang Aircraft Corporation',
  } satisfies AirframeDefinition),
  'f-35': Object.freeze({
    id: 'f-35',
    displayName: 'F-35 Lightning II',
    manufacturer: 'Lockheed Martin',
  } satisfies AirframeDefinition),
  'f-22': Object.freeze({
    id: 'f-22',
    displayName: 'F-22 Raptor',
    manufacturer: 'Lockheed Martin / Boeing',
  } satisfies AirframeDefinition),
  prototype: Object.freeze({
    id: 'prototype',
    displayName: 'Generic Test Prototype',
    manufacturer: 'A3S Agent Evaluation',
  } satisfies AirframeDefinition),
});

export const MODEL_AIRFRAME_RULES: readonly ModelAirframeRule[] = Object.freeze([
  Object.freeze({
    id: 'glm',
    airframeId: 'j-50',
    exactModels: Object.freeze(['glm-5.2']),
    familyPrefixes: Object.freeze(['glm']),
  }),
  Object.freeze({
    id: 'gpt',
    airframeId: 'f-35',
    exactModels: Object.freeze(['gpt-5.6']),
    familyPrefixes: Object.freeze(['gpt', 'codex', 'o1', 'o3', 'o4']),
  }),
  Object.freeze({
    id: 'claude',
    airframeId: 'f-22',
    exactModels: Object.freeze([]),
    familyPrefixes: Object.freeze(['claude']),
  }),
]);

export function resolveAirframe(
  model?: string | null,
  candidate?: string | null,
  preferredAirframeId?: AirframeId | null
): AirframeResolution {
  if (preferredAirframeId) {
    return { airframe: AIRFRAMES[preferredAirframeId], strategy: 'manual' };
  }

  const modelId = normalizeModelIdentifier(model ?? '');
  if (modelId) {
    const exactRule = MODEL_AIRFRAME_RULES.find((rule) =>
      rule.exactModels.some((exactModel) => normalizeModelIdentifier(exactModel) === modelId)
    );
    if (exactRule) return resolutionForRule(exactRule, 'model-exact');

    const familyRule = MODEL_AIRFRAME_RULES.find((rule) =>
      rule.familyPrefixes.some((prefix) => matchesFamily(modelId, normalizeModelIdentifier(prefix)))
    );
    if (familyRule) return resolutionForRule(familyRule, 'model-family');
  }

  const candidateProfile = resolveAircraft(candidate ?? '');
  if (candidateProfile !== AIRCRAFT_PROFILES.prototype) {
    return {
      airframe: AIRFRAMES[candidateProfile.id],
      strategy: 'candidate-fallback',
    };
  }

  return { airframe: AIRFRAMES.prototype, strategy: 'default' };
}

function resolutionForRule(rule: ModelAirframeRule, strategy: AirframeResolutionStrategy): AirframeResolution {
  return {
    airframe: AIRFRAMES[rule.airframeId],
    strategy,
    ruleId: rule.id,
  };
}

function normalizeModelIdentifier(value: string): string {
  const identifier = value.trim().toLowerCase().split(/[/:]/).filter(Boolean).at(-1) ?? '';
  return identifier
    .normalize('NFKC')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

function matchesFamily(modelId: string, familyPrefix: string): boolean {
  if (modelId === familyPrefix || modelId.startsWith(`${familyPrefix}-`)) return true;
  const nextCharacter = modelId.at(familyPrefix.length);
  return modelId.startsWith(familyPrefix) && nextCharacter !== undefined && /[0-9]/.test(nextCharacter);
}
