import { beforeEach, describe, expect, it } from 'vitest';
import { labState } from '../../state/lab-state';
import { createHangarDraft, DEFAULT_HANGAR_ROSTER } from './hangar-configuration';
import { restoreHangarState, startHangarPersistence } from './hangar-persistence';

const STORAGE_KEY = 'a3s-agent-evaluation.hangar.v1';

const LEGACY_A3S_DEFAULT = {
  id: 'a3s-j-35',
  airframeId: 'j-35' as const,
  pilotId: 'a3s' as const,
  candidate: 'a3s-code',
  model: 'zai/glm-5.2',
  effort: 'high' as const,
  callsign: 'A3S-01',
};

class MemoryStorage {
  readonly values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }
}

beforeEach(() => {
  labState.hangar = {
    draft: createHangarDraft('a3s'),
    roster: DEFAULT_HANGAR_ROSTER.map((entry) => ({ ...entry })),
    activeEntryId: DEFAULT_HANGAR_ROSTER[0].id,
  };
});

describe('hangar persistence', () => {
  it('restores the complete roster and its active combination across reloads', async () => {
    const storage = new MemoryStorage();
    const stop = startHangarPersistence(storage);
    const codex = labState.hangar.roster[1];
    codex.candidate = './agents/codex-adapter';
    codex.model = 'openai/gpt-5.6';
    labState.hangar.activeEntryId = codex.id;
    await Promise.resolve();
    stop();

    labState.hangar = {
      draft: createHangarDraft('generic'),
      roster: [{ id: 'temporary', ...createHangarDraft('generic') }],
      activeEntryId: 'temporary',
    };

    expect(restoreHangarState(storage)).toBe(true);
    expect(labState.hangar.activeEntryId).toBe(codex.id);
    expect(labState.hangar.draft).toMatchObject({
      pilotId: 'codex',
      candidate: './agents/codex-adapter',
      model: 'openai/gpt-5.6',
    });
    expect(labState.hangar.roster).toHaveLength(DEFAULT_HANGAR_ROSTER.length);
  });

  it('ignores malformed or oversized persisted data', () => {
    const storage = new MemoryStorage();
    storage.setItem(
      STORAGE_KEY,
      JSON.stringify({ version: 1, activeEntryId: 'bad', roster: [{ id: 'bad', candidate: 42 }] })
    );

    expect(restoreHangarState(storage)).toBe(false);
    expect(labState.hangar.activeEntryId).toBe(DEFAULT_HANGAR_ROSTER[0].id);
  });

  it('migrates the legacy built-in A3S default while preserving user callsign and effort changes', () => {
    const storage = new MemoryStorage();
    storage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        version: 1,
        activeEntryId: LEGACY_A3S_DEFAULT.id,
        roster: [{ ...LEGACY_A3S_DEFAULT, effort: 'xhigh', callsign: 'VIPER-07' }],
      })
    );

    expect(restoreHangarState(storage)).toBe(true);
    expect(labState.hangar.activeEntryId).toBe('a3s-j-50');
    expect(labState.hangar.roster).toEqual([
      {
        ...LEGACY_A3S_DEFAULT,
        id: 'a3s-j-50',
        airframeId: 'j-50',
        model: 'anthropic/glm-5.2',
        effort: 'xhigh',
        callsign: 'VIPER-07',
      },
    ]);
    expect(labState.hangar.draft).toMatchObject({
      airframeId: 'j-50',
      model: 'anthropic/glm-5.2',
      effort: 'xhigh',
      callsign: 'VIPER-07',
    });
    expect(JSON.parse(storage.getItem(STORAGE_KEY) ?? '')).toMatchObject({
      activeEntryId: 'a3s-j-50',
      roster: [{ id: 'a3s-j-50', airframeId: 'j-50', model: 'anthropic/glm-5.2' }],
    });
  });

  it('allocates a collision-free id and follows it when the migrated entry is active', () => {
    const storage = new MemoryStorage();
    const existingJ50 = {
      id: 'a3s-j-50',
      ...createHangarDraft('generic', 'prototype'),
      candidate: './agents/custom-adapter',
      model: 'custom/model',
      callsign: 'CUSTOM-01',
    };
    storage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        version: 1,
        activeEntryId: LEGACY_A3S_DEFAULT.id,
        roster: [LEGACY_A3S_DEFAULT, existingJ50],
      })
    );

    expect(restoreHangarState(storage)).toBe(true);
    expect(labState.hangar.roster.map((entry) => entry.id)).toEqual(['a3s-j-50-2', 'a3s-j-50']);
    expect(labState.hangar.activeEntryId).toBe('a3s-j-50-2');
    expect(labState.hangar.draft).toMatchObject({ airframeId: 'j-50', model: 'anthropic/glm-5.2' });
    expect(labState.hangar.roster[1]).toEqual(existingJ50);
  });

  it.each([
    ['model', { model: 'custom/glm-5.2' }],
    ['airframe', { airframeId: 'prototype' as const }],
    ['candidate', { candidate: './agents/a3s-adapter' }],
  ])('does not overwrite a legacy-shaped entry with a user-customized %s', (_field, override) => {
    const storage = new MemoryStorage();
    const customized = { ...LEGACY_A3S_DEFAULT, ...override };
    const serialized = JSON.stringify({ version: 1, activeEntryId: customized.id, roster: [customized] });
    storage.setItem(STORAGE_KEY, serialized);

    expect(restoreHangarState(storage)).toBe(true);
    expect(labState.hangar.roster).toEqual([customized]);
    expect(labState.hangar.activeEntryId).toBe(customized.id);
    expect(storage.getItem(STORAGE_KEY)).toBe(serialized);
  });

  it('keeps a valid user-created copy of the old combination unchanged', () => {
    const storage = new MemoryStorage();
    const userCreated = { ...LEGACY_A3S_DEFAULT, id: 'custom-a3s-flight' };
    const serialized = JSON.stringify({ version: 1, activeEntryId: userCreated.id, roster: [userCreated] });
    storage.setItem(STORAGE_KEY, serialized);

    expect(restoreHangarState(storage)).toBe(true);
    expect(labState.hangar.roster).toEqual([userCreated]);
    expect(labState.hangar.activeEntryId).toBe(userCreated.id);
    expect(storage.getItem(STORAGE_KEY)).toBe(serialized);
  });
});
