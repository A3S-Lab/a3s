const SYNC_STATE_KEY = 'a3s-work.server-sync.v1';

export interface WorkSyncState {
  migrated: boolean;
  artifacts: string[];
  folders: string[];
  artifactPurges: string[];
  folderPurges: string[];
}

const EMPTY_SYNC_STATE: WorkSyncState = {
  migrated: false,
  artifacts: [],
  folders: [],
  artifactPurges: [],
  folderPurges: [],
};

export function readWorkSyncState(): WorkSyncState {
  try {
    const value = JSON.parse(localStorage.getItem(SYNC_STATE_KEY) ?? 'null') as Partial<WorkSyncState> | null;
    if (!value || typeof value !== 'object') return { ...EMPTY_SYNC_STATE };
    return {
      migrated: value.migrated === true,
      artifacts: stringArray(value.artifacts),
      folders: stringArray(value.folders),
      artifactPurges: stringArray(value.artifactPurges),
      folderPurges: stringArray(value.folderPurges),
    };
  } catch {
    return { ...EMPTY_SYNC_STATE };
  }
}

export function updateWorkSyncState(update: (state: WorkSyncState) => WorkSyncState): void {
  const next = update(readWorkSyncState());
  localStorage.setItem(SYNC_STATE_KEY, JSON.stringify(next));
}

export function markArtifactPending(id: string): void {
  updateWorkSyncState((state) => ({ ...state, artifacts: addUnique(state.artifacts, id) }));
}

export function markFolderPending(id: string): void {
  updateWorkSyncState((state) => ({ ...state, folders: addUnique(state.folders, id) }));
}

export function markArtifactPurgePending(id: string): void {
  updateWorkSyncState((state) => ({
    ...state,
    artifacts: remove(state.artifacts, id),
    artifactPurges: addUnique(state.artifactPurges, id),
  }));
}

export function markFolderPurgePending(id: string): void {
  updateWorkSyncState((state) => ({
    ...state,
    folders: remove(state.folders, id),
    folderPurges: addUnique(state.folderPurges, id),
  }));
}

export function clearArtifactPending(id: string): void {
  updateWorkSyncState((state) => ({
    ...state,
    artifacts: remove(state.artifacts, id),
    artifactPurges: remove(state.artifactPurges, id),
  }));
}

export function clearFolderPending(id: string): void {
  updateWorkSyncState((state) => ({
    ...state,
    folders: remove(state.folders, id),
    folderPurges: remove(state.folderPurges, id),
  }));
}

function stringArray(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return [...new Set(value.filter((item): item is string => typeof item === 'string' && item.length > 0))];
}

function addUnique(items: string[], id: string): string[] {
  return items.includes(id) ? items : [...items, id];
}

function remove(items: string[], id: string): string[] {
  return items.filter((item) => item !== id);
}
