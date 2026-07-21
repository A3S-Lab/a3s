import { describe, expect, it } from 'vitest';
import { demoTasks } from '../../data/demo-tasks';
import { createRunSortieSnapshot, type RunSortieSnapshot } from '../../state/lab-state';
import { DEFAULT_HANGAR_ROSTER, type HangarRosterEntry } from '../hangar/hangar-configuration';
import { loadSortieManifest, saveSortieManifest } from './sortie-manifest-store';

const STORAGE_KEY = 'a3s-agent-evaluation.sorties.v1';

class MemoryStorage {
  readonly values = new Map<string, string>();
  setCalls = 0;

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.setCalls += 1;
    this.values.set(key, value);
  }
}

describe('sortie manifest store', () => {
  it('restores the exact frozen map, roster, and executable input by Bench Run ID', () => {
    const storage = new MemoryStorage();
    const sortie = unlockedSortie();

    saveSortieManifest('local-real-run', sortie, storage);
    const restored = loadSortieManifest('local-real-run', storage);

    expect(restored).toEqual(sortie);
    expect(Object.isFrozen(restored)).toBe(true);
    expect(Object.isFrozen(restored?.task)).toBe(true);
    expect(Object.isFrozen(restored?.task.tags)).toBe(true);
    expect(Object.isFrozen(restored?.rosterEntry)).toBe(true);
    expect(Object.isFrozen(restored?.input)).toBe(true);
  });

  it('accepts a locked sortie only with independent lock references and no model input', () => {
    const storage = new MemoryStorage();
    const sortie = createRunSortieSnapshot(demoTasks[0], DEFAULT_HANGAR_ROSTER[0], {
      task: './locks/task.lock.json',
      candidate: './locks/candidate.lock.json',
      locked: true,
    });

    saveSortieManifest('local-locked-run', sortie, storage);

    expect(loadSortieManifest('local-locked-run', storage)).toEqual(sortie);
  });

  it('never overwrites the first valid sortie attributed to an existing Run ID', () => {
    const storage = new MemoryStorage();
    const original = unlockedSortie();
    const conflicting = unlockedSortie(demoTasks[1]);

    saveSortieManifest('local-reused-run-id', original, storage);
    saveSortieManifest(' local-reused-run-id ', conflicting, storage);

    expect(storage.setCalls).toBe(1);
    expect(loadSortieManifest('local-reused-run-id', storage)).toEqual(original);
  });

  it('does not persist an internally inconsistent snapshot or disturb a valid archive', () => {
    const storage = new MemoryStorage();
    const original = unlockedSortie();
    saveSortieManifest('local-original', original, storage);
    const serialized = storage.getItem(STORAGE_KEY);

    const inconsistent = createRunSortieSnapshot(demoTasks[0], DEFAULT_HANGAR_ROSTER[0], {
      task: demoTasks[1].id,
      candidate: DEFAULT_HANGAR_ROSTER[0].candidate,
      model: DEFAULT_HANGAR_ROSTER[0].model,
      locked: false,
    });
    saveSortieManifest('local-inconsistent', inconsistent, storage);

    expect(storage.setCalls).toBe(1);
    expect(storage.getItem(STORAGE_KEY)).toBe(serialized);
    expect(loadSortieManifest('local-original', storage)).toEqual(original);
    expect(loadSortieManifest('local-inconsistent', storage)).toBeUndefined();
  });

  it('rejects unexpected keys at every persisted object boundary', () => {
    const mutations: Array<(archive: MutableArchive) => void> = [
      (archive) => {
        archive.untrusted = true;
      },
      (archive) => {
        archive.manifests[0].untrusted = true;
      },
      (archive) => {
        archive.manifests[0].sortie.untrusted = true;
      },
      (archive) => {
        archive.manifests[0].sortie.task.untrusted = true;
      },
      (archive) => {
        archive.manifests[0].sortie.rosterEntry.untrusted = true;
      },
      (archive) => {
        archive.manifests[0].sortie.input.untrusted = true;
      },
    ];

    for (const mutate of mutations) {
      const storage = storedSortie();
      const archive = readMutableArchive(storage);
      mutate(archive);
      storage.setItem(STORAGE_KEY, JSON.stringify(archive));

      expect(loadSortieManifest('local-real-run', storage)).toBeUndefined();
    }
  });

  it('rejects invalid timestamps, overflows, overlong values, and invalid enumerations', () => {
    const mutations: Array<(archive: MutableArchive) => void> = [
      (archive) => {
        archive.manifests[0].savedAt = 'now';
      },
      (archive) => {
        archive.manifests[0].savedAt = '2026-02-30T00:00:00.000Z';
      },
      (archive) => {
        archive.manifests[0].runId = 'r'.repeat(129);
      },
      (archive) => {
        archive.manifests[0].sortie.task.description = 'd'.repeat(8_193);
      },
      (archive) => {
        archive.manifests[0].sortie.task.tags = Array.from({ length: 65 }, (_, index) => `tag-${index}`);
      },
      (archive) => {
        archive.manifests[0].sortie.rosterEntry.callsign = 'c'.repeat(129);
      },
      (archive) => {
        archive.manifests[0].sortie.input.candidate = 'c'.repeat(1_025);
      },
      (archive) => {
        archive.manifests[0].sortie.task.availability = 'invented';
      },
      (archive) => {
        while (archive.manifests.length < 101) {
          const copy = structuredClone(archive.manifests[0]);
          copy.runId = `local-overflow-${archive.manifests.length}`;
          archive.manifests.push(copy);
        }
      },
    ];

    for (const mutate of mutations) {
      const storage = storedSortie();
      const archive = readMutableArchive(storage);
      mutate(archive);
      storage.setItem(STORAGE_KEY, JSON.stringify(archive));

      expect(loadSortieManifest('local-real-run', storage)).toBeUndefined();
    }
  });

  it('rejects unlocked cross-attribution and every invalid locked-mode combination', () => {
    const mutations: Array<(archive: MutableArchive) => void> = [
      (archive) => {
        archive.manifests[0].sortie.input.task = demoTasks[1].id;
      },
      (archive) => {
        archive.manifests[0].sortie.input.candidate = './another-candidate';
      },
      (archive) => {
        archive.manifests[0].sortie.input.model = 'provider/another-model';
      },
      (archive) => {
        archive.manifests[0].sortie.input.locked = true;
        archive.manifests[0].sortie.input.model = 'provider/model-is-forbidden';
      },
      (archive) => {
        archive.manifests[0].sortie.input.locked = 'yes';
      },
    ];

    for (const mutate of mutations) {
      const storage = storedSortie();
      const archive = readMutableArchive(storage);
      mutate(archive);
      storage.setItem(STORAGE_KEY, JSON.stringify(archive));

      expect(loadSortieManifest('local-real-run', storage)).toBeUndefined();
    }
  });

  it('quarantines ambiguous duplicate Run IDs without hiding unrelated valid sorties', () => {
    const storage = new MemoryStorage();
    saveSortieManifest('local-duplicate', unlockedSortie(), storage);
    saveSortieManifest('local-unrelated', unlockedSortie(demoTasks[1]), storage);
    const archive = readMutableArchive(storage);
    const duplicate = structuredClone(archive.manifests.find((entry) => entry.runId === 'local-duplicate'));
    expect(duplicate).toBeDefined();
    if (!duplicate) return;
    duplicate.sortie.task = JSON.parse(JSON.stringify(demoTasks[1])) as MutableTask;
    duplicate.sortie.input.task = demoTasks[1].id;
    archive.manifests.push(duplicate);
    storage.setItem(STORAGE_KEY, JSON.stringify(archive));

    expect(loadSortieManifest('local-duplicate', storage)).toBeUndefined();
    expect(loadSortieManifest('local-unrelated', storage)).toEqual(unlockedSortie(demoTasks[1]));
  });

  it('ignores malformed records instead of misattributing a Bench result', () => {
    const storage = new MemoryStorage();
    storage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        version: 1,
        manifests: [{ runId: 'local-bad', savedAt: 'now', sortie: { task: {} } }, validStoredManifest('local-good')],
      })
    );

    expect(loadSortieManifest('local-bad', storage)).toBeUndefined();
    expect(loadSortieManifest('local-good', storage)).toEqual(unlockedSortie());
  });
});

function unlockedSortie(
  task = demoTasks[0],
  rosterEntry: Readonly<HangarRosterEntry> = DEFAULT_HANGAR_ROSTER[0]
): RunSortieSnapshot {
  return createRunSortieSnapshot(task, rosterEntry, {
    task: task.id,
    candidate: rosterEntry.candidate.trim(),
    model: rosterEntry.model.trim() || undefined,
    locked: false,
  });
}

function storedSortie(): MemoryStorage {
  const storage = new MemoryStorage();
  saveSortieManifest('local-real-run', unlockedSortie(), storage);
  return storage;
}

function validStoredManifest(runId: string): MutableManifest {
  const storage = new MemoryStorage();
  saveSortieManifest(runId, unlockedSortie(), storage);
  return readMutableArchive(storage).manifests[0];
}

function readMutableArchive(storage: MemoryStorage): MutableArchive {
  return JSON.parse(storage.getItem(STORAGE_KEY) as string) as MutableArchive;
}

interface MutableArchive extends Record<string, unknown> {
  version: number;
  manifests: MutableManifest[];
}

interface MutableManifest extends Record<string, unknown> {
  runId: string;
  savedAt: string;
  sortie: MutableSortie;
}

interface MutableSortie extends Record<string, unknown> {
  task: MutableTask;
  rosterEntry: Record<string, unknown> & HangarRosterEntry;
  input: Record<string, unknown> & {
    task: string;
    candidate: string;
    model?: string;
    locked: boolean | string;
  };
}

type MutableTask = Record<string, unknown> & {
  description?: string;
  tags?: string[];
  availability: string;
};
