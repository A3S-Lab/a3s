import type { EvolutionCandidate, EvolutionOverview } from '../../types/api';

const NOW = '2026-07-21T08:00:00.000Z';

export function evolutionTestData(): EvolutionOverview {
  const skill = candidate({
    id: 'skill-focused-verification',
    kind: 'skill',
    patternKey: 'skill.verification.focused',
    title: 'Focused verification',
    summary: 'Run focused checks before broad validation.',
    instructions: [
      'Identify the smallest relevant test target.',
      'Run focused checks before broad workspace validation.',
    ],
    state: 'materialized',
    occurrences: 4,
    distinctSessions: 3,
    confidence: 0.94,
    importance: 0.88,
    maturity: 0.93,
    assetPath: '.a3s/skills/focused-verification',
    currentVersion: 2,
    versions: [
      {
        version: 1,
        createdAt: '2026-07-20T08:00:00.000Z',
        assetPath: '.a3s/skills/focused-verification',
        snapshotPath: '.a3s/evolution/history/skill-focused-verification/v0001/asset',
        contentHash: 'hash-v1',
        evidenceIds: ['evidence-1', 'evidence-2'],
        automatic: true,
      },
      {
        version: 2,
        createdAt: NOW,
        assetPath: '.a3s/skills/focused-verification',
        snapshotPath: '.a3s/evolution/history/skill-focused-verification/v0002/asset',
        contentHash: 'hash-v2',
        evidenceIds: ['evidence-1', 'evidence-2', 'evidence-3', 'evidence-4'],
        automatic: false,
      },
    ],
    audit: [
      { action: 'ready', at: '2026-07-20T07:00:00.000Z', note: 'matured from recurring evidence' },
      { action: 'materialized', at: '2026-07-20T08:00:00.000Z', version: 1, note: 'materialized locally' },
      { action: 'updated', at: NOW, version: 2, note: 'materialized locally after explicit review' },
      { action: 'activated', at: NOW, version: 2, note: 'loaded by a Code session' },
    ],
  });
  skill.evidence = [
    evidence('evidence-1', 'session-one', 'Run focused cargo tests after changing a Rust crate.', true),
    evidence('evidence-2', 'session-two', 'Prefer the smallest relevant test target before the full suite.', true),
    evidence('evidence-3', 'session-three', 'Validate the affected package before broad checks.', false),
    evidence('evidence-4', 'session-three', 'Keep verification proportional to the changed behavior.', false),
  ];

  const preference = candidate({
    id: 'preference-concise-evidence',
    kind: 'preference',
    patternKey: 'preference.response.concise-evidence',
    title: 'Concise evidence-backed responses',
    summary: 'Lead with outcomes and retain concrete evidence.',
    instructions: ['Lead with the result.', 'Use concrete evidence for completion claims.'],
    state: 'ready',
    occurrences: 2,
    distinctSessions: 2,
    confidence: 0.96,
    importance: 0.9,
    maturity: 0.91,
  });
  preference.evidence = [
    evidence('preference-1', 'session-one', 'Keep answers concise and evidence-backed.', true),
    evidence('preference-2', 'session-two', 'Show the result before implementation details.', true),
  ];

  const rejected = candidate({
    id: 'okf-obsolete-library',
    kind: 'okf',
    patternKey: 'okf.library.obsolete',
    title: 'Obsolete library notes',
    summary: 'A rejected knowledge pattern.',
    instructions: ['Consult the obsolete library notes.'],
    state: 'rejected',
    occurrences: 2,
    distinctSessions: 1,
    confidence: 0.8,
    importance: 0.78,
    maturity: 0.72,
    rejectionReason: 'No longer applies to this workspace.',
  });

  return {
    schema: 'a3s.code.evolution.v1',
    revision: 8,
    root: '/workspace/.a3s/evolution',
    workspaceRoot: '/workspace',
    skillRoot: '/workspace/.a3s/skills',
    okfRoot: '/workspace/okf',
    updatedAt: NOW,
    stats: {
      total: 3,
      observing: 0,
      ready: 1,
      materialized: 1,
      rejected: 1,
      rolledBack: 0,
      updateAvailable: 0,
      activationPending: 0,
      byKind: { preference: 1, skill: 1, okf: 1 },
    },
    candidates: [preference, skill, rejected],
    policy: {
      readyEvidence: 2,
      autoMaterializeEvidence: 3,
      autoMaterializeSessions: 2,
      autoMaterializeConfidence: 0.88,
      localOnly: true,
      reviewSupported: true,
    },
  };
}

function candidate(overrides: Partial<EvolutionCandidate> & Pick<EvolutionCandidate, 'id' | 'kind' | 'title'>) {
  return {
    id: overrides.id,
    kind: overrides.kind,
    patternKey: overrides.patternKey ?? `pattern.${overrides.id}`,
    patternAliases: [],
    title: overrides.title,
    summary: overrides.summary ?? '',
    instructions: overrides.instructions ?? [],
    state: overrides.state ?? 'observing',
    evidence: overrides.evidence ?? [],
    occurrences: overrides.occurrences ?? 1,
    distinctSessions: overrides.distinctSessions ?? 1,
    confidence: overrides.confidence ?? 0.8,
    importance: overrides.importance ?? 0.8,
    maturity: overrides.maturity ?? 0.7,
    hasConflicts: overrides.hasConflicts ?? false,
    updateAvailable: overrides.updateAvailable ?? false,
    activationPending: overrides.activationPending ?? false,
    createdAt: overrides.createdAt ?? '2026-07-19T08:00:00.000Z',
    updatedAt: overrides.updatedAt ?? NOW,
    readyAt: overrides.readyAt,
    materializedAt: overrides.materializedAt,
    rejectedAt: overrides.rejectedAt,
    rolledBackAt: overrides.rolledBackAt,
    rejectionReason: overrides.rejectionReason,
    assetPath: overrides.assetPath,
    currentVersion: overrides.currentVersion,
    versions: overrides.versions ?? [],
    audit: overrides.audit ?? [],
  } satisfies EvolutionCandidate;
}

function evidence(id: string, sessionId: string, content: string, explicitSignal: boolean) {
  return {
    id,
    memoryId: `memory-${id}`,
    sessionId,
    source: 'preference',
    content,
    reason: 'Repeated behavior changed future task execution.',
    timestamp: NOW,
    importance: 0.9,
    confidence: 0.95,
    conflictsWith: [],
    explicitSignal,
  };
}
